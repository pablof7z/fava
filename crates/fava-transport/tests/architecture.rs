//! Executable boundary for the relay-session contract.
//!
//! The transport speaks NIP-01 now: it builds the envelopes Fava sends and
//! routes the messages a relay returns. That is a deliberate, bounded amount of
//! protocol knowledge -- four verbs and one correlation mapping -- and this test
//! is what keeps it bounded. Query meaning, attribution, retained demand, plans,
//! and retry belong to owners above it (`GOALS:1089`, RELAY-005;
//! `.planning/REQUIREMENTS.md` OWN-02).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CARGO_MANIFEST: &str = include_str!("../Cargo.toml");

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
        return PathBuf::from(workspace).join("crates/fava-transport");
    }
    if let (Some(runfiles), Some(workspace)) = (
        std::env::var_os("TEST_SRCDIR"),
        std::env::var_os("TEST_WORKSPACE"),
    ) {
        return PathBuf::from(runfiles)
            .join(workspace)
            .join("crates/fava-transport");
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

/// The contract's dependencies are exactly these, so adding one is a decision
/// rather than an accident.
#[test]
fn the_transport_contract_depends_on_exactly_its_declared_set() {
    assert_eq!(
        dependency_names(CARGO_MANIFEST, "[dependencies]"),
        BTreeSet::from([
            "fava-relay".to_owned(),
            "fava-state".to_owned(),
            "fava-wire".to_owned(),
            "nostr".to_owned(),
            "thiserror".to_owned(),
            "tokio".to_owned(),
        ]),
        "the transport contract gained or lost a dependency"
    );
}

/// The transport owns sessions, envelopes, and delivery. It does not own what
/// any of it means.
#[test]
fn the_transport_owns_no_query_meaning() {
    let mut sources = Vec::new();
    rust_sources(&crate_root().join("src"), &mut sources);
    assert!(!sources.is_empty(), "the crate has sources to check");

    let production = sources
        .iter()
        .map(|path| fs::read_to_string(path).expect("source is readable"))
        .collect::<Vec<_>>()
        .join("\n");

    for forbidden in [
        "fava_observe",
        "fava_publication",
        "fava_routing",
        "fava_subscriptions",
        "fava_query",
        "fava_ingest",
        "fava_event_cache",
        "fava_write_store",
        "fava_publisher",
        "fava_delivery",
        "fava_runtime",
    ] {
        assert!(
            !production.contains(forbidden),
            "the transport reached for an owner above it: {forbidden}"
        );
    }

    // Retained demand, plans, and attribution belong to `fava-observe`. The
    // transport holds a correlation key against a delivery channel, valid for
    // one connection, and nothing else (OWN-02).
    for forbidden in [
        "RelayDemand",
        "SubscriptionPlan",
        "PlanRevision",
        "InstalledSubscriptions",
        "ObservationId",
        "DemandId",
    ] {
        assert!(
            !production.contains(forbidden),
            "the transport retained something an owner above it owns: {forbidden}"
        );
    }
}

/// A connection with no socket carries nothing, whatever it once proved.
#[test]
fn a_dropped_connection_serves_no_work_it_previously_could() {
    let alice = nostr::key::PublicKey::parse(
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )
    .expect("public key");
    let identity = fava_transport::RelaySessionIdentity {
        key: fava_relay::RelaySessionKey {
            relay: nostr::types::RelayUrl::parse("wss://relay.example").expect("relay URL"),
            access: fava_relay::RelayAccess::Authenticated(alice),
        },
        connection: fava_transport::RelayConnection::new(1).expect("non-zero"),
    };
    let live = fava_transport::Connection {
        identity: identity.clone(),
        connectivity: fava_relay::Connectivity::Connected,
        authentication: fava_relay::Authentication::Authenticated { as_of: alice },
    };
    let want = fava_relay::Authority::As(alice);
    assert!(
        live.can_serve(&want),
        "a live authenticated connection serves its own account"
    );

    let dropped = fava_transport::Connection {
        connectivity: fava_relay::Connectivity::Disconnected {
            detail: fava_relay::BoundedText::new("socket closed"),
            spent: None,
        },
        ..live
    };
    assert!(
        !dropped.can_serve(&want),
        "a connection with no socket carries nothing"
    );
}
