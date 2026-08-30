//! Executable dependency and layering boundary for the bookmarks protocol crate.

use std::collections::BTreeSet;

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

#[test]
fn bookmarks_remains_engine_provider_free() {
    assert_eq!(
        dependency_names(CARGO_MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "fava-state".to_owned(),
            "fava-write".to_owned(),
            "nostr".to_owned(),
        ])
    );
    // "fava" itself is excluded from this substring scan (it is a substring
    // of every other dependency name here, `fava-state` included); the exact
    // `assert_eq!` above is what actually forbids it.
    for forbidden in [
        "fava-query",
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
    for required in ["//crates/fava-state:lib", "//crates/fava-write:lib"] {
        assert!(
            library.contains(required),
            "missing neutral Bazel edge: {required}"
        );
    }
    assert!(
        !library.contains("//crates/fava:lib"),
        "forbidden Bazel edge onto the universal facade"
    );

    let production = include_str!("../src/lib.rs");
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
