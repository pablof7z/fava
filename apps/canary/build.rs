//! Embed the exact committed Fava source identity into each canary build.

use std::path::Path;
use std::process::Command;

const SOURCE_PATHS: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "apps/canary",
    "crates",
];

fn main() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("canary manifest directory");
    let repository = Path::new(&manifest)
        .join("../..")
        .canonicalize()
        .expect("Fava repository root");
    let mut status_arguments = vec!["status", "--porcelain=v1", "--untracked-files=all", "--"];
    status_arguments.extend(SOURCE_PATHS);
    let clean = git_bytes(&repository, &status_arguments).is_empty();
    if std::env::var("FAVA_CANARY_PINNED_BUILD").as_deref() == Ok("1") {
        assert!(clean, "pinned Fava build source inputs were dirty");
    }
    let revision = git(&repository, &["rev-parse", "HEAD"]);
    let tree = git(&repository, &["rev-parse", "HEAD^{tree}"]);
    println!("cargo:rustc-env=FAVA_BUILD_REVISION={revision}");
    println!("cargo:rustc-env=FAVA_BUILD_TREE={tree}");
    println!("cargo:rustc-env=FAVA_BUILD_SOURCE_CLEAN={clean}");
    println!("cargo:rerun-if-env-changed=FAVA_CANARY_PINNED_BUILD");
    for path in SOURCE_PATHS {
        println!("cargo:rerun-if-changed={}", repository.join(path).display());
    }
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
