//! Black-box replay acceptance for the ordinary E2E command runner.

use std::path::PathBuf;
use std::process::Command;
use std::{fs, time::SystemTime};

fn run_script(name: &str) -> std::process::Output {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name);
    Command::new(env!("CARGO_BIN_EXE_simple-groups"))
        .args(["--jsonl", "--script", path.to_str().unwrap()])
        .output()
        .unwrap()
}

#[test]
fn ordinary_non_pty_replay_uses_shared_commands_and_typed_jsonl() {
    let output = run_script("shell.txt");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(rows.lines().count(), 7);
    assert!(rows.lines().all(|line| line.starts_with('{')));
    assert!(rows.contains("\"kind\":\"account-created\""));
    assert!(rows.contains("\"kind\":\"relay-list\""));
    assert!(rows.contains("\"kind\":\"dump\""));
}

#[test]
fn replay_refuses_a_missing_domain_value_without_consuming_the_next_line() {
    let output = run_script("missing-required.txt");
    assert!(!output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(rows.lines().count(), 1);
    assert!(rows.contains("\"status\":\"refused\""));
    assert!(rows.contains("interactive prompting is unavailable for script input"));
    assert!(!rows.contains("account-created"));
}

#[test]
fn replay_refuses_an_omitted_explicit_event_kind() {
    let output = run_script("missing-kind.txt");
    assert!(!output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(rows.lines().count(), 1);
    assert!(rows.contains("\"status\":\"refused\""));
    assert!(rows.contains("interactive prompting is unavailable for script input"));
    assert!(!rows.contains("\"kind\":\"quit\""));
}

#[test]
fn replay_refuses_reversed_explicit_event_kind_tokens() {
    let output = run_script("reversed-kind.txt");
    assert!(!output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    assert_eq!(rows.lines().count(), 1);
    assert!(rows.contains("group event publish --kind <kind> [content]"));
    assert!(!rows.contains("\"kind\":\"quit\""));
}

#[test]
fn group_context_refuses_before_a_ninth_insertion_and_lists_typed_values() {
    let output = run_script("group-bound.txt");
    assert!(!output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    let last: serde_json::Value = serde_json::from_str(rows.lines().last().unwrap()).unwrap();
    assert_eq!(last["status"], "refused");
    assert!(
        last["summary"]
            .as_str()
            .unwrap()
            .contains("known simple groups exceeds its limit of 8")
    );

    let output = run_script("group-list-bounded.txt");
    assert!(output.status.success());
    let rows = String::from_utf8(output.stdout).unwrap();
    let values = rows
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    for kind in ["group-list", "status"] {
        let row = values.iter().find(|row| row["kind"] == kind).unwrap();
        assert_eq!(
            row["fields"][if kind == "group-list" {
                "groups"
            } else {
                "known_groups"
            }]
            .as_array()
            .unwrap()
            .len(),
            8
        );
    }
}

#[test]
fn every_domain_command_family_has_black_box_typed_jsonl_without_live_success() {
    let commands = [
        "status",
        "routes",
        "receipt list",
        "receipt show 1",
        "diagnostics",
        "group create room missing",
        "group open room missing",
        "group list",
        "group switch room",
        "group edit name=room",
        "group invite code",
        "group join code reason",
        "group member add aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa member",
        "group member remove aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "group leave",
        "group delete",
        "group event publish --kind 12345 payload",
        "group event expect-rejection --kind 12345 payload",
        "group event delete aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "group events 1",
        "group state 1",
        "saved-list show missing 1",
        "saved-list group add missing display",
        "saved-list group rename missing display",
        "saved-list group remove missing",
        "saved-list relay add missing missing",
        "saved-list relay remove missing missing",
    ];
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(format!(".fava-simple-groups-domain-{nonce}.txt"));

    for (index, command) in commands.iter().enumerate() {
        fs::write(&script, format!("{command}\n")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_simple-groups"))
            .args(["--jsonl", "--script", script.to_str().unwrap()])
            .output()
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let row: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|error| {
            panic!(
                "command {command:?} did not produce one JSON result: {error}; stdout={stdout:?}"
            )
        });
        assert!(row["status"].is_string(), "command {command:?}");
        assert!(row["kind"].is_string(), "command {command:?}");
        assert!(row["summary"].is_string(), "command {command:?}");
        assert!(row["fields"].is_object(), "command {command:?}");
        assert!(
            !row["summary"].as_str().unwrap().contains("unknown command"),
            "valid domain grammar was not dispatched: {command:?}"
        );
        assert!(
            !output.status.success() || matches!(index, 0 | 2 | 3 | 4 | 7),
            "the black-box grammar probe must not claim a live write: {command:?}"
        );
    }
    fs::remove_file(script).unwrap();
}
