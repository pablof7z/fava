//! There is one way to reach a relay, and one place that reads what it says.
//!
//! Before this boundary existed, three crates each built their own NIP-01
//! envelopes and each parsed the whole connection to find their own replies.
//! That is what let a publication mistake another component's traffic for its
//! own answer. The verbs and the per-handle delivery removed it; this test is
//! what keeps it removed.

use std::fs;
use std::path::{Path, PathBuf};

/// Crates permitted to name a client message or decode a relay one.
///
/// `fava-transport*` is the boundary itself. `fava-subscriptions*` builds a
/// `REQ` only to measure its exact encoded length against a relay's advertised
/// message limit, during planning, with no session in hand -- it sends nothing.
/// `fava-wire` is the grammar being used.
const PERMITTED: &[&str] = &[
    "crates/fava-transport/",
    "crates/fava-transport-websocket/",
    "crates/fava-transport-testkit/",
    "crates/fava-subscriptions/",
    "crates/fava-subscriptions-standard/",
    "crates/fava-subscriptions-no-grouping/",
    "crates/fava-wire/",
];

/// Naming any of these outside the boundary means building an envelope or
/// reading a relay's words by hand.
const RESERVED: &[&str] = &["encode_client", "decode_relay", "ClientMessage"];

fn workspace_root() -> PathBuf {
    if let Some(workspace) = std::env::var_os("BUILD_WORKSPACE_DIRECTORY") {
        return PathBuf::from(workspace);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

fn production_sources(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Tests may write frames by hand: they stand in for a relay.
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            production_sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn no_crate_outside_the_transport_builds_an_envelope_or_reads_a_relay() {
    let root = workspace_root();
    let mut sources = Vec::new();
    production_sources(&root.join("crates"), &mut sources);
    assert!(!sources.is_empty(), "the workspace has sources to check");

    let mut offenders = Vec::new();
    for path in sources {
        let relative = path
            .strip_prefix(&root)
            .expect("every source sits under the workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        if PERMITTED.iter().any(|allowed| relative.starts_with(allowed)) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("source is readable");
        for reserved in RESERVED {
            if source.contains(reserved) {
                offenders.push(format!("{relative} names {reserved}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "reaching a relay is the session's verbs, and reading one is its handles:\n  {}",
        offenders.join("\n  ")
    );
}
