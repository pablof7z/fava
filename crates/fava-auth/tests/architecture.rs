//! Dependency fence for the authentication owner.
//!
//! `fava-auth` owns one lifecycle. It reaches the signer, the transport, the
//! runtime, and the relay vocabulary, and nothing else. In particular it does
//! not depend on `fava-query`: that is why the lifecycle a connection carries
//! lives in `fava-relay`, where both this crate and query evidence can name it
//! without either depending on the other.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn dependencies_section() -> String {
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml")).expect("manifest reads");
    let start = manifest
        .find("[dependencies]")
        .expect("the manifest declares dependencies");
    let rest = &manifest[start + "[dependencies]".len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    rest[..end].to_owned()
}

fn declared_dependencies() -> Vec<String> {
    dependencies_section()
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim().to_owned()))
        .map(|name| name.trim_end_matches(".workspace").to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

fn production_sources() -> Vec<String> {
    fn walk(root: &Path, found: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(fs::read_to_string(&path).expect("source reads"));
            }
        }
    }
    let mut found = Vec::new();
    walk(&crate_root().join("src"), &mut found);
    assert!(!found.is_empty(), "the crate has production source");
    found
}

#[test]
fn the_authentication_owner_declares_exactly_its_allowed_dependencies() {
    let mut declared = declared_dependencies();
    declared.sort();

    let mut allowed = vec![
        "fava-relay",
        "fava-runtime",
        "fava-session",
        "fava-transport",
        "fava-wire",
        "fava-write",
        "nostr",
        "thiserror",
        "tokio",
    ];
    allowed.sort_unstable();

    assert_eq!(
        declared, allowed,
        "the authentication owner's dependency set is closed; adding one is an architecture change"
    );
}

#[test]
fn the_authentication_owner_names_no_higher_level_owner() {
    let banned = [
        "fava-query",
        "fava_query",
        "fava-observe",
        "fava_observe",
        "fava-publication",
        "fava_publication",
        "fava-publisher",
        "fava_publisher",
        "fava-routing",
        "fava_routing",
        "fava-ingest",
        "fava_ingest",
        "fava-event-cache",
        "fava_event_cache",
        "fava-write-store",
        "fava_write_store",
        "fava-diagnostics",
        "fava_diagnostics",
    ];

    let manifest = dependencies_section();
    let sources = production_sources();

    for name in banned {
        assert!(
            !manifest.contains(name),
            "{name} must not appear in the authentication owner's dependencies"
        );
        for source in &sources {
            assert!(
                !source.contains(name),
                "{name} must not be named in the authentication owner's production source"
            );
        }
    }
}
