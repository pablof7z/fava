//! Executable dependency and lifecycle boundary for the NIP-02 protocol crate.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");
const BAZEL_MANIFEST: &str = include_str!("../BUILD.bazel");

fn section<'a>(manifest: &'a str, header: &str) -> &'a str {
    manifest
        .split_once(header)
        .map(|(_, tail)| tail.split("\n[").next().unwrap_or(tail))
        .expect("manifest section exists")
}

fn dependency_names(manifest: &str, header: &str) -> BTreeSet<String> {
    section(manifest, header)
        .lines()
        .filter_map(|line| {
            line.split_once('=').map(|(name, _)| {
                name.trim()
                    .strip_suffix(".workspace")
                    .unwrap_or(name.trim())
                    .to_owned()
            })
        })
        .filter(|name| !name.is_empty())
        .collect()
}

fn crate_root() -> PathBuf {
    if let Some(workspace) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
        return PathBuf::from(workspace).join("crates/fava-nip02");
    }
    // Under `bazel test` the sources live in the runfiles tree, not in the
    // source checkout. Same resolution order as
    // `crates/fava-simple-groups/tests/architecture.rs`.
    if let (Some(runfiles), Some(workspace)) = (
        std::env::var_os("TEST_SRCDIR"),
        std::env::var_os("TEST_WORKSPACE"),
    ) {
        return PathBuf::from(runfiles)
            .join(workspace)
            .join("crates/fava-nip02");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

#[test]
fn nip02_remains_engine_provider_free() {
    assert_eq!(
        dependency_names(CARGO_MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "fava-query".to_owned(),
            "fava-state".to_owned(),
            "fava-write".to_owned(),
        ])
    );
    for forbidden in [
        "fava-observe",
        "fava-publication",
        "fava-routing",
        "fava-signer",
        "fava-transport",
        "fava-event-cache",
        "fava-write-store",
        "fava-publisher",
        "fava-ingest",
    ] {
        assert!(
            !section(CARGO_MANIFEST, "[dependencies]").contains(forbidden),
            "forbidden normal dependency: {forbidden}"
        );
    }

    let library = BAZEL_MANIFEST
        .split_once("rust_library(")
        .expect("library target")
        .1
        .split_once("rust_test(")
        .expect("following test target")
        .0;
    for required in [
        "//crates/fava-query:lib",
        "//crates/fava-state:lib",
        "//crates/fava-write:lib",
    ] {
        assert!(
            library.contains(required),
            "missing neutral Bazel edge: {required}"
        );
    }
    let production = [
        include_str!("../src/lib.rs"),
        include_str!("../src/query.rs"),
        include_str!("../src/contact_list.rs"),
    ]
    .join("\n");
    for forbidden in [
        "use fava::",
        "fava_observe",
        "fava_publication",
        "fava_routing",
        "fava_signer",
        "fava_transport",
        "fava_event_cache",
        "fava_write_store",
        "fava_publisher",
        "fava_ingest",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden source edge: {forbidden}"
        );
    }
}

#[test]
fn universal_publication_and_query_owners_remain_kind_blind() {
    let crate_root = crate_root();
    let root = crate_root.parent().expect("workspace crates directory");
    let mut sources = Vec::new();
    for owner in [
        "fava",
        "fava-publication",
        "fava-query",
        "fava-observe",
        "fava-write",
    ] {
        rust_sources(&root.join(owner).join("src"), &mut sources);
    }
    assert!(
        !sources.is_empty(),
        "universal owner source set must be non-empty"
    );

    for source in sources {
        let text = fs::read_to_string(&source).expect("universal owner source is readable");
        for forbidden in ["Kind::ContactList", "Kind::from_u16(3)"] {
            assert!(
                !text.contains(forbidden),
                "universal owner {} contains NIP-02 switch {forbidden}",
                source.display()
            );
        }
    }
}
