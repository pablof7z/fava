//! Executable dependency, ownership, and universal-owner boundaries.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fava_query::{Kind, Query, RelayUrl};
use fava_simple_groups::{SimpleGroup, SimpleGroupRecords, SimpleGroups};
use fava_write::{EventBuilder, PublicKey, Timestamp};

const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const BAZEL_MANIFEST: &str = include_str!("../BUILD.bazel");
const PUBLIC_ROOT: &str = include_str!("../src/lib.rs");

fn toml_table_keys(manifest: &str, table: &str) -> BTreeSet<String> {
    let header = format!("[{table}]");
    let mut in_table = false;
    let mut keys = BTreeSet::new();
    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_table = line == header;
            continue;
        }
        if in_table && !line.is_empty() {
            let (key, _) = line
                .split_once('=')
                .expect("dependency entries are TOML key/value assignments");
            keys.insert(
                key.trim()
                    .strip_suffix(".workspace")
                    .unwrap_or(key.trim())
                    .to_owned(),
            );
        }
    }
    keys
}

fn starlark_call<'a>(source: &'a str, function: &str) -> &'a str {
    let start = source
        .find(&format!("{function}("))
        .expect("Starlark call exists");
    let source = &source[start..];
    let mut depth = 0_u32;
    for (index, byte) in source.bytes().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return &source[..=index];
                }
            }
            _ => {}
        }
    }
    panic!("Starlark call is balanced");
}

fn first_party_library_deps(target: &str) -> BTreeSet<String> {
    target
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.strip_prefix('"')
                .and_then(|line| line.strip_suffix("\","))
        })
        .filter(|label| label.starts_with("//crates/") && label.ends_with(":lib"))
        .map(str::to_owned)
        .collect()
}

fn workspace_root() -> PathBuf {
    if let Some(root) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
        return PathBuf::from(root);
    }
    if let (Some(runfiles), Some(workspace)) = (
        std::env::var_os("TEST_SRCDIR"),
        std::env::var_os("TEST_WORKSPACE"),
    ) {
        return PathBuf::from(runfiles).join(workspace);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives beneath workspace/crates")
        .to_owned()
}

fn rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).expect("source directory exists") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

fn capability_production_sources() -> Vec<PathBuf> {
    let mut sources = Vec::new();
    rust_sources(
        &workspace_root().join("crates/fava-simple-groups/src"),
        &mut sources,
    );
    sources.retain(|source| {
        !source
            .components()
            .any(|component| component.as_os_str() == "tests")
            && source.file_name().is_none_or(|name| name != "tests.rs")
    });
    sources.sort();
    sources
}

fn code_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with("//") && !line.starts_with("/*") && !line.starts_with('*')
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn public_exports(root: &str) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
    for line in root.lines().map(str::trim) {
        for prefix in ["pub struct ", "pub enum ", "pub trait ", "pub type "] {
            if let Some(name) = line.strip_prefix(prefix).and_then(|body| {
                body.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                    .next()
            }) {
                exports.insert(name.to_owned());
            }
        }
        let Some(body) = line.strip_prefix("pub use ") else {
            continue;
        };
        let body = body.trim_end_matches(';');
        if let Some((_, names)) = body.split_once("::{") {
            for name in names.trim_end_matches('}').split(',') {
                exports.insert(name.trim().to_owned());
            }
        } else {
            exports.insert(
                body.rsplit_once("::")
                    .map_or(body, |(_, name)| name)
                    .to_owned(),
            );
        }
    }
    exports
}

#[test]
fn normal_dependencies_are_exact() {
    let cargo_expected = BTreeSet::from([
        "fava-query".to_owned(),
        "fava-state".to_owned(),
        "fava-write".to_owned(),
    ]);
    let bazel_expected = BTreeSet::from([
        "//crates/fava-query:lib".to_owned(),
        "//crates/fava-state:lib".to_owned(),
        "//crates/fava-write:lib".to_owned(),
    ]);
    let cargo_actual = toml_table_keys(CARGO_MANIFEST, "dependencies");
    let bazel_actual = first_party_library_deps(starlark_call(BAZEL_MANIFEST, "rust_library"));
    assert_eq!(
        (cargo_actual, bazel_actual),
        (cargo_expected, bazel_expected),
        "Cargo and Bazel normal dependencies must equal the approved neutral owners"
    );
}

#[test]
fn pure_helpers_have_no_lifecycle_owner() {
    let sources = capability_production_sources();
    assert!(
        !sources.is_empty(),
        "capability source set must be non-empty"
    );
    for source in &sources {
        let code = code_lines(&fs::read_to_string(source).expect("source is readable"));
        assert!(
            !code.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with("static ") || line.starts_with("pub static ")
            }),
            "pure helper retained hidden static state in {}",
            source.display()
        );
        for forbidden in [
            "thread_local!",
            "OnceLock",
            "LazyLock",
            "Mutex<",
            "RwLock<",
            "AtomicBool",
            "AtomicUsize",
            "AtomicU64",
        ] {
            assert!(
                !code.contains(forbidden),
                "pure helper retained hidden state in {} through {forbidden}",
                source.display()
            );
        }
    }

    let host = RelayUrl::parse("wss://groups.example").expect("relay URL");
    let simple_group = SimpleGroup::on([host.clone()], "photos").expect("group");
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            .expect("generator public key");
    let draft = EventBuilder::new(author, Kind::from_u16(9))
        .created_at(Timestamp::from(7))
        .build()
        .expect("bounded draft");
    let prepared = simple_group.prepare(draft).expect("first preparation");
    assert_eq!(simple_group.prepare(prepared.clone()), Ok(prepared));
    let query = Query::events().limit(8).expect("positive limit");
    assert_eq!(simple_group.events(query.clone()), simple_group.events(query));
    assert_eq!(
        simple_group.records(SimpleGroupRecords::all()),
        simple_group.records(SimpleGroupRecords::all())
    );
    assert_eq!(simple_group.hosts().collect::<Vec<_>>(), vec![host]);
    let first = SimpleGroups::materializer();
    let second = SimpleGroups::materializer();
    assert!(!Arc::ptr_eq(&first, &second));
}

#[test]
fn capability_sources_and_exports_own_no_lifecycle() {
    for source in capability_production_sources() {
        let code = code_lines(&fs::read_to_string(&source).expect("source is readable"));
        for forbidden in [
            "use fava::",
            "fava_observe",
            "fava_publication",
            "fava_signer",
            "fava_routing",
            "fava_write_store",
            "fava_publisher",
            "fava_delivery",
            "fava_transport",
            "SimpleGroupObservation",
            "SimpleGroupPublication",
            "SimpleGroupReceipt",
            "SimpleGroupRuntime",
            "SimpleGroupProvider",
            "SimpleGroupStore",
            "SimpleGroupLifecycle",
        ] {
            assert!(
                !code.contains(forbidden),
                "capability source {} owns forbidden lifecycle edge {forbidden}",
                source.display()
            );
        }
    }
    let approved = BTreeSet::from([
        "PinnedItem".to_owned(),
        "SavedRelay".to_owned(),
        "SavedSimpleGroup".to_owned(),
        "SimpleGroup".to_owned(),
        "SimpleGroupAdmins".to_owned(),
        "SimpleGroupError".to_owned(),
        "SimpleGroupMembers".to_owned(),
        "SimpleGroupMetadata".to_owned(),
        "SimpleGroupParticipants".to_owned(),
        "SimpleGroupPins".to_owned(),
        "SimpleGroupRecords".to_owned(),
        "SimpleGroupRoles".to_owned(),
        "SimpleGroupSnapshot".to_owned(),
        "SimpleGroups".to_owned(),
    ]);
    assert_eq!(public_exports(PUBLIC_ROOT), approved);
}

#[test]
fn universal_owners_remain_nip29_blind() {
    let root = workspace_root().join("crates");
    let mut sources = Vec::new();
    for owner in ["fava", "fava-query", "fava-state", "fava-write"] {
        rust_sources(&root.join(owner).join("src"), &mut sources);
    }
    assert!(
        !sources.is_empty(),
        "universal owner source set must be non-empty"
    );
    for source in sources {
        let code = code_lines(&fs::read_to_string(&source).expect("source is readable"));
        for forbidden in [
            "fava_simple_groups",
            "fava-simple-groups",
            "NIP-29",
            "NIP29",
            "NIP_29",
            "nip29",
            "nip_29",
            "39_000",
            "39_001",
            "39_002",
            "39_003",
            "39_004",
            "39_005",
            "39000",
            "39001",
            "39002",
            "39003",
            "39004",
            "39005",
            "10_009",
            "10009",
            "9_002",
            "9002",
            "9_010",
            "9010",
        ] {
            assert!(
                !code.contains(forbidden),
                "universal owner {} contains capability branch {forbidden}",
                source.display()
            );
        }
    }
}

#[test]
fn management_kinds_remain_author_bearing_architecture_break() {
    let root = workspace_root().join("crates/fava-simple-groups/src");
    let management = code_lines(
        &fs::read_to_string(root.join("management.rs")).expect("management source is readable"),
    );
    let edit = code_lines(&fs::read_to_string(root.join("edit.rs")).expect("edit source readable"));
    assert!(management.contains("UnsignedEvent"));
    assert!(!management.contains("ReplaceableEventEdit"));
    for forbidden in ["9_002", "9_010", "9002", "9010"] {
        assert!(
            !edit.contains(forbidden),
            "management kind entered ReplaceableEventEdit source through {forbidden}"
        );
    }
}
