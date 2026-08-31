//! OWN-07's `nip42_challenge_state_lives_only_in_fava_auth`.
//!
//! NIP-42 authenticates a connection, not a message. If two components each
//! remember whether a connection is authenticated, they disagree the moment one
//! of them misses a reconnect -- and one of them will, because a reconnect is
//! not addressed to anybody. Exactly one component holds that state.
//!
//! This is a source falsifier rather than a behavioural one: the defect it
//! guards is a second component quietly growing its own copy, which no runtime
//! assertion sees until the two disagree in production.

use std::fs;
use std::path::{Path, PathBuf};

/// Where the challenge lifecycle is allowed to live.
const OWNER: &str = "crates/fava-auth/";

/// The transport may *recognise* a challenge, because it routes each relay
/// message to whoever asked for it and a challenge is one of them. Recognising
/// which reader a message belongs to is not holding its state: the transport
/// keeps no verdict, no attempt count, and no memory across connections.
const ROUTERS: &[&str] = &["crates/fava-transport/"];

/// Naming any of these outside the owner is holding challenge state, or a
/// verdict derived from one.
const RESERVED: &[&str] = &[
    "AuthenticationDemand",
    "SessionAuthentication",
    "AuthenticationDecision",
    "Challenge::new",
    "auth_event",
];

/// A verdict is a fact one component determines. Every other component may
/// report it, so naming the enum is fine; deriving one is not.
const DERIVATION: &[&str] = &["RelayMessage::Auth", "MachineReadablePrefix"];

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
            // A test may impersonate a relay, which means writing an AUTH frame.
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
fn nip42_challenge_state_lives_only_in_fava_auth() {
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
        if relative.starts_with(OWNER) {
            continue;
        }
        let source = fs::read_to_string(&path).expect("source is readable");
        let routing = ROUTERS.iter().any(|allowed| relative.starts_with(allowed));
        let checked: Vec<&&str> = if routing {
            RESERVED.iter().collect()
        } else {
            RESERVED.iter().chain(DERIVATION).collect()
        };
        for reserved in checked {
            if source.contains(reserved) {
                offenders.push(format!("{relative} names {reserved}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a relay challenge is one component's business:\n  {}",
        offenders.join("\n  ")
    );
}
