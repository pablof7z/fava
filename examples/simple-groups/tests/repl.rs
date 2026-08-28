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
fn noninteractive_jsonl_is_byte_for_byte_stable() {
    let output = run_script("noninteractive-jsonl.txt");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        include_str!("noninteractive-jsonl.golden")
    );
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

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_no_color_matches_the_ansi_stripped_golden() {
    let raw = run_interactive_pty("env NO_COLOR=1 COLUMNS=88", true);
    assert!(
        !raw.contains("\x1b[38;"),
        "NO_COLOR emitted an ANSI color: {raw:?}"
    );
    assert!(
        !raw.contains("\x1b[9"),
        "NO_COLOR emitted an ANSI color: {raw:?}"
    );
    assert_eq!(
        normalize_pty(&raw),
        include_str!("interactive-no-color.golden")
    );
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_emits_color_until_no_color_is_requested() {
    let raw = run_interactive_pty("env -u NO_COLOR COLUMNS=88", false);
    assert!(raw.contains("\x1b[38;") || raw.contains("\x1b[1;96m"));
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_no_color_flag_removes_color_codes() {
    let raw = run_interactive_pty_with_args("env -u NO_COLOR COLUMNS=88", false, "--no-color");
    assert!(!raw.contains("\x1b[38;"), "--no-color emitted ANSI color");
    assert!(!raw.contains("\x1b[9"), "--no-color emitted ANSI color");
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_receipt_completion_lists_documented_actions() {
    let raw = run_logged_interactive_pty_commands(
        "env NO_COLOR=1 COLUMNS=88",
        "",
        r#"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {no-account}
send -raw "receipt "
send -raw "\t"
expect -re {list}
expect -re {show}
send -raw "\003"
expect -re {input cancelled}
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {no-account}
send "quit\r"
expect eof
"#,
    );
    let plain = plain_pty(&raw);
    let plain_lower = plain.to_ascii_lowercase();
    assert!(
        plain_lower.contains("list"),
        "receipt list was not completed: {plain:?}"
    );
    assert!(
        plain_lower.contains("show"),
        "receipt show was not completed: {plain:?}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_elides_legal_context_names_at_40_columns() {
    let account = "a".repeat(32);
    let group = "g".repeat(32);
    let commands = format!(
        r#"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{no-account}}
send "account new {account}\r"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{account created}}
send "relay add group ws://127.0.0.1:18101\r"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{relay added}}
send "group open {group} group\r"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{group opened}}
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{1 relay}}
send "quit\r"
after 100
"#,
    );
    let raw = run_logged_interactive_pty_commands("env NO_COLOR=1 COLUMNS=40", "", &commands);
    let plain = plain_pty(&raw);
    let expected = "aaaaaaaaaa… · ggggggggggg… · 1 relay › ";
    assert_eq!(expected.chars().count(), 39);
    assert!(
        plain.lines().any(|line| line.starts_with(expected)),
        "missing elided 40-column prompt: {plain:?}"
    );
}

#[cfg(target_os = "macos")]
fn run_interactive_pty(environment: &str, full_session: bool) -> String {
    run_interactive_pty_with_args(environment, full_session, "")
}

#[cfg(target_os = "macos")]
fn run_interactive_pty_with_args(environment: &str, full_session: bool, arguments: &str) -> String {
    let commands = if full_session {
        r#"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {no-account}
send "relay add group ws://127.0.0.1:18101\r"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {no-account}
send "group open weekend-builders group\r"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {weekend-builders}
send "status\r"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {weekend-builders}
send "quit\r"
expect eof
"#
    } else {
        r#"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {no-account}
send "quit\r"
expect eof
"#
    };
    run_interactive_pty_commands(environment, arguments, commands)
}

#[cfg(target_os = "macos")]
fn run_interactive_pty_commands(environment: &str, arguments: &str, commands: &str) -> String {
    let binary = env!("CARGO_BIN_EXE_simple-groups");
    let program = format!(
        "set timeout 15\nspawn {environment} $env(FAVA_REPL_BINARY) {arguments}\n{commands}"
    );
    let output = Command::new("/usr/bin/expect")
        .args(["-c", &program])
        .env("FAVA_REPL_BINARY", binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[cfg(target_os = "macos")]
fn run_logged_interactive_pty_commands(
    environment: &str,
    arguments: &str,
    commands: &str,
) -> String {
    let binary = env!("CARGO_BIN_EXE_simple-groups");
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let log = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(format!(".fava-simple-groups-pty-{nonce}.log"));
    let program = format!(
        "set timeout 3\nexpect_after {{ exit 1 }}\nlog_file -noappend $env(FAVA_REPL_LOG)\nspawn {environment} $env(FAVA_REPL_BINARY) {arguments}\n{commands}"
    );
    let output = Command::new("/usr/bin/expect")
        .args(["-c", &program])
        .env("FAVA_REPL_BINARY", binary)
        .env("FAVA_REPL_LOG", &log)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let raw = fs::read_to_string(&log).unwrap();
    fs::remove_file(log).unwrap();
    raw
}

#[cfg(target_os = "macos")]
fn normalize_pty(raw: &str) -> String {
    let text = plain_pty(raw);
    let text = &text[text.find("fava simple-groups").unwrap()..];
    text.lines()
        .map(normalize_timing)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

#[cfg(target_os = "macos")]
fn plain_pty(raw: &str) -> String {
    String::from_utf8(strip_ansi_escapes::strip(raw))
        .unwrap()
        .replace('\r', "")
}

#[cfg(target_os = "macos")]
fn normalize_timing(line: &str) -> String {
    let Some(ms) = line.rfind(" ms") else {
        return line.to_owned();
    };
    let digits = line[..ms]
        .char_indices()
        .rev()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map_or(ms, |(index, _)| index);
    format!("{}<ms>{}", &line[..digits], &line[ms + 3..])
}
