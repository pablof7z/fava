//! Embed exact immutable compiler-input provenance into pinned canary builds.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

const SOURCE_PATHS: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "apps/canary",
    "crates",
];
const PINNED_MANIFEST: &str = "/attestation/source.manifest";
const MAX_FILES: usize = 4_096;
const MAX_FILE_BYTES: u64 = 8_388_608;
const MAX_TOTAL_BYTES: u64 = 67_108_864;

struct BuildClaim {
    revision: String,
    tree: String,
    source_tree_sha256: String,
    source_manifest_sha256: String,
    source_image_sha256: String,
    rust_base_image_sha256: String,
    clean: bool,
    immutable: bool,
}

struct SourceRow {
    mode: u32,
    sha256: String,
    bytes: u64,
}

fn main() {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").expect("canary manifest directory");
    let repository = Path::new(&manifest_dir)
        .join("../..")
        .canonicalize()
        .expect("Fava repository root");
    let pinned = std::env::var("FAVA_CANARY_PINNED_BUILD").as_deref() == Ok("1");
    let claim = if pinned {
        pinned_claim(&repository)
    } else {
        development_claim(&repository)
    };
    println!("cargo:rustc-env=FAVA_BUILD_REVISION={}", claim.revision);
    println!("cargo:rustc-env=FAVA_BUILD_TREE={}", claim.tree);
    println!(
        "cargo:rustc-env=FAVA_BUILD_SOURCE_TREE_SHA256={}",
        claim.source_tree_sha256
    );
    println!(
        "cargo:rustc-env=FAVA_BUILD_SOURCE_MANIFEST_SHA256={}",
        claim.source_manifest_sha256
    );
    println!(
        "cargo:rustc-env=FAVA_BUILD_SOURCE_IMAGE_SHA256={}",
        claim.source_image_sha256
    );
    println!(
        "cargo:rustc-env=FAVA_BUILD_RUST_BASE_IMAGE_SHA256={}",
        claim.rust_base_image_sha256
    );
    println!("cargo:rustc-env=FAVA_BUILD_SOURCE_CLEAN={}", claim.clean);
    println!(
        "cargo:rustc-env=FAVA_BUILD_SOURCE_IMMUTABLE={}",
        claim.immutable
    );
    for name in [
        "FAVA_CANARY_PINNED_BUILD",
        "FAVA_BUILD_REVISION",
        "FAVA_BUILD_TREE",
        "FAVA_BUILD_SOURCE_TREE_SHA256",
        "FAVA_BUILD_SOURCE_MANIFEST_SHA256",
        "FAVA_BUILD_SOURCE_IMAGE_SHA256",
        "FAVA_BUILD_RUST_BASE_IMAGE_SHA256",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for path in SOURCE_PATHS {
        println!("cargo:rerun-if-changed={}", repository.join(path).display());
    }
}

fn pinned_claim(root: &Path) -> BuildClaim {
    let revision = exact_env("FAVA_BUILD_REVISION", 40);
    let tree = exact_env("FAVA_BUILD_TREE", 40);
    let source_tree_sha256 = exact_env("FAVA_BUILD_SOURCE_TREE_SHA256", 64);
    let source_manifest_sha256 = exact_env("FAVA_BUILD_SOURCE_MANIFEST_SHA256", 64);
    let source_image_sha256 = exact_env("FAVA_BUILD_SOURCE_IMAGE_SHA256", 64);
    let rust_base_image_sha256 = exact_env("FAVA_BUILD_RUST_BASE_IMAGE_SHA256", 64);
    assert!(
        !source_image_sha256.bytes().all(|byte| byte == b'0'),
        "pinned Fava build source image identity was all zero"
    );
    assert!(
        !rust_base_image_sha256.bytes().all(|byte| byte == b'0'),
        "pinned Rust base image identity was all zero"
    );
    let manifest_bytes = fs::read(PINNED_MANIFEST).expect("pinned source manifest");
    assert_eq!(
        hex::encode(Sha256::digest(&manifest_bytes)),
        source_manifest_sha256,
        "pinned source manifest digest disagreed with the engine-derived expectation"
    );
    let rows = parse_manifest(&manifest_bytes, &revision, &tree);
    let actual = actual_source_rows(root);
    assert_eq!(
        rows.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>(),
        "pinned source manifest inventory disagreed with compiler inputs"
    );
    for (path, expected) in rows {
        let observed = actual.get(&path).expect("manifest path");
        assert_eq!(observed.mode, expected.mode, "pinned source mode changed");
        assert_eq!(observed.bytes, expected.bytes, "pinned source size changed");
        assert_eq!(observed.sha256, expected.sha256, "pinned source bytes changed");
        assert!(
            write_open_is_read_only(&root.join(&path)),
            "pinned Fava compiler input was not on a read-only filesystem"
        );
    }
    assert!(
        write_open_is_read_only(Path::new(PINNED_MANIFEST)),
        "pinned Fava source manifest was not on a read-only filesystem"
    );
    BuildClaim {
        revision,
        tree,
        source_tree_sha256,
        source_manifest_sha256,
        source_image_sha256,
        rust_base_image_sha256,
        clean: true,
        immutable: true,
    }
}

fn development_claim(root: &Path) -> BuildClaim {
    let mut status_arguments = vec!["status", "--porcelain=v1", "--untracked-files=all", "--"];
    status_arguments.extend(SOURCE_PATHS);
    let clean = git_bytes(root, &status_arguments).is_empty();
    let revision = git(root, &["rev-parse", "HEAD"]);
    let tree = git(root, &["rev-parse", "HEAD^{tree}"]);
    let mut tree_arguments = vec!["ls-tree", "-r", "--full-tree", "HEAD", "--"];
    tree_arguments.extend(SOURCE_PATHS);
    BuildClaim {
        revision,
        tree,
        source_tree_sha256: hex::encode(Sha256::digest(git_bytes(root, &tree_arguments))),
        source_manifest_sha256: "0".repeat(64),
        source_image_sha256: "0".repeat(64),
        rust_base_image_sha256: "0".repeat(64),
        clean,
        immutable: false,
    }
}

fn parse_manifest(bytes: &[u8], revision: &str, tree: &str) -> BTreeMap<PathBuf, SourceRow> {
    assert!(bytes.ends_with(b"\n"), "pinned source manifest lacked final LF");
    assert!(!bytes.contains(&b'\r'), "pinned source manifest contained CR");
    let text = std::str::from_utf8(bytes).expect("pinned source manifest UTF-8");
    let lines = text.lines().collect::<Vec<_>>();
    assert!(lines.len() >= 5, "pinned source manifest headers were incomplete");
    assert_eq!(lines[0], "format=fava-pinned-source-v1");
    assert_eq!(lines[1], format!("revision={revision}"));
    assert_eq!(lines[2], format!("tree={tree}"));
    let file_count = canonical_u64(
        lines[3]
            .strip_prefix("file_count=")
            .expect("source manifest file count"),
    );
    let total_bytes = canonical_u64(
        lines[4]
            .strip_prefix("total_bytes=")
            .expect("source manifest total bytes"),
    );
    let file_count = usize::try_from(file_count).expect("source manifest file count fits usize");
    assert!(file_count > 0 && file_count <= MAX_FILES);
    assert!(total_bytes <= MAX_TOTAL_BYTES);
    assert_eq!(lines.len(), 5 + file_count);
    let mut rows = BTreeMap::new();
    let mut observed_total = 0_u64;
    for line in &lines[5..] {
        let fields = line
            .strip_prefix("file=")
            .expect("source manifest file row")
            .split('\t')
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 4, "source manifest file row shape");
        let mode = fields[0].parse::<u32>().expect("source mode");
        assert!(matches!(mode, 100_644 | 100_755));
        assert!(is_lower_hex(fields[1], 64));
        let size = canonical_u64(fields[2]);
        assert!(size <= MAX_FILE_BYTES);
        let path = canonical_path(fields[3]);
        assert!(
            rows.insert(
                path,
                SourceRow {
                    mode,
                    sha256: fields[1].to_owned(),
                    bytes: size,
                },
            )
            .is_none(),
            "duplicate source manifest path"
        );
        observed_total = observed_total.checked_add(size).expect("source bytes overflow");
    }
    assert_eq!(observed_total, total_bytes);
    rows
}

fn actual_source_rows(root: &Path) -> BTreeMap<PathBuf, SourceRow> {
    let mut files = Vec::new();
    for relative in SOURCE_PATHS {
        let path = root.join(relative);
        if path.exists() {
            collect_files(root, &path, &mut files);
        }
    }
    files.sort();
    files.dedup();
    assert!(!files.is_empty() && files.len() <= MAX_FILES);
    let mut total = 0_u64;
    let mut rows = BTreeMap::new();
    for relative in files {
        let path = root.join(&relative);
        let metadata = path.symlink_metadata().expect("compiler input metadata");
        assert!(metadata.is_file() && metadata.len() <= MAX_FILE_BYTES);
        let permissions = metadata.permissions().mode() & 0o777;
        let mode = match permissions {
            0o644 | 0o444 => 100_644,
            0o755 | 0o555 => 100_755,
            _ => panic!("pinned compiler input had noncanonical permissions"),
        };
        let bytes = fs::read(&path).expect("compiler input bytes");
        assert_eq!(bytes.len() as u64, metadata.len());
        total = total.checked_add(metadata.len()).expect("compiler bytes overflow");
        assert!(total <= MAX_TOTAL_BYTES);
        rows.insert(
            relative,
            SourceRow {
                mode,
                sha256: hex::encode(Sha256::digest(bytes)),
                bytes: metadata.len(),
            },
        );
    }
    rows
}

fn collect_files(root: &Path, path: &Path, files: &mut Vec<PathBuf>) {
    let metadata = path.symlink_metadata().expect("source inventory metadata");
    assert!(!metadata.file_type().is_symlink(), "source inventory symlink");
    if metadata.is_file() {
        files.push(path.strip_prefix(root).expect("source relative path").to_owned());
        return;
    }
    assert!(metadata.is_dir(), "source inventory special file");
    let mut children = fs::read_dir(path)
        .expect("source directory")
        .map(|entry| entry.expect("source entry").path())
        .collect::<Vec<_>>();
    children.sort();
    for child in children {
        collect_files(root, &child, files);
    }
}

fn canonical_path(value: &str) -> PathBuf {
    assert!(!value.is_empty() && value.len() <= 512 && !value.starts_with('/'));
    assert!(
        value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"._/+@=-".contains(&byte)
        }),
        "source manifest path characters"
    );
    let path = PathBuf::from(value);
    assert!(
        path.components().all(|component| {
            matches!(component, std::path::Component::Normal(_))
        }),
        "source manifest path traversal"
    );
    path
}

fn canonical_u64(value: &str) -> u64 {
    let parsed = value.parse::<u64>().expect("canonical decimal");
    assert_eq!(parsed.to_string(), value, "noncanonical decimal");
    parsed
}

fn exact_env(name: &str, length: usize) -> String {
    let value = std::env::var(name).unwrap_or_else(|_| panic!("missing pinned build {name}"));
    assert!(is_lower_hex(&value, length), "invalid pinned build {name}");
    value
}

fn write_open_is_read_only(path: &Path) -> bool {
    matches!(
        OpenOptions::new().write(true).open(path),
        Err(error) if error.kind() == ErrorKind::ReadOnlyFilesystem
    )
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn git(root: &Path, arguments: &[&str]) -> String {
    String::from_utf8(git_bytes(root, arguments))
        .expect("Git output was UTF-8")
        .trim()
        .to_owned()
}

fn git_bytes(root: &Path, arguments: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .expect("Git source identity command launched");
    assert!(output.status.success(), "Git source identity command failed");
    output.stdout
}
