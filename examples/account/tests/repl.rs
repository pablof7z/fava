//! Black-box replay evidence through the real account binary.

use std::io::Write;
use std::process::{Command, Stdio};

const ALICE_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000001";
const BOB_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000002";
const ALICE_PUBKEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

#[test]
fn replay_exercises_account_lifecycle_and_fava_selection() {
    let script = format!(
        "account import alice {ALICE_SECRET}\n\
         account import bob {BOB_SECRET}\n\
         account switch alice\n\
         account replace alice {ALICE_SECRET}\n\
         account clear\n\
         account remove alice\n\
         account add-pubkey viewer {ALICE_PUBKEY}\n\
         account list\n\
         diagnostics\n\
         account remove bob\n\
         quit\n"
    );
    let output = run(&script);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let rows = rows(&output);
    assert_eq!(rows.len(), 11);
    assert!(rows.iter().all(|row| row["status"] == "ok"));
    assert_eq!(rows[2]["kind"], "account-selected");
    assert_eq!(rows[3]["kind"], "account-replaced");
    assert_eq!(rows[4]["kind"], "account-cleared");
    assert_eq!(rows[5]["kind"], "account-removed");
    assert_eq!(rows[6]["kind"], "account-added");
    assert_eq!(rows[8]["fields"]["current_pubkey"], ALICE_PUBKEY);
    assert_eq!(
        rows[8]["fields"]["signer_availability"],
        serde_json::json!(["pubkey-only", "Available"])
    );
    assert_eq!(rows[8]["fields"]["signer_generations"][0], "");
    assert!(
        rows[8]["fields"]["signer_generations"][1]
            .as_u64()
            .is_some_and(|generation| generation > 0)
    );
}

#[test]
fn current_pubkey_query_opens_once_and_keeps_its_public_id() {
    let script = format!(
        "relay add dead ws://127.0.0.1:1\n\
         account import alice {ALICE_SECRET}\n\
         query open mine $currentPubkey 1 dead\n\
         query snapshot mine\n\
         routes\n\
         account clear\n\
         diagnostics\n\
         query close mine\n\
         quit\n"
    );
    let output = run(&script);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let rows = rows(&output);
    assert_eq!(rows[2]["kind"], "query-opened");
    assert_eq!(rows[2]["fields"]["observation_id"], 1);
    assert_eq!(rows[3]["fields"]["observation_id"], 1);
    assert_eq!(rows[4]["kind"], "routes");
    assert_eq!(
        rows[4]["fields"]["demand_observations"],
        serde_json::json!([1])
    );
    assert_eq!(rows[6]["fields"]["query_ids"], serde_json::json!([1]));
    assert_eq!(rows[7]["fields"]["observation_id"], 1);
}

#[test]
fn replay_omission_refuses_without_consuming_the_next_command() {
    let output = run("account import alice\naccount list\n");
    assert!(!output.status.success());
    let rows = rows(&output);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "shell-refused");
    assert!(
        rows[0]["summary"]
            .as_str()
            .expect("summary")
            .contains("interactive prompting is unavailable")
    );
    assert!(stderr(&output).contains("NonInteractivePrompt"));
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_renders_context_and_results_without_color() {
    let script = format!(
        r#"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{no-account.*0r.*0q.*›}}
send "account import alice {ALICE_SECRET}\r"
expect "account-imported and selected alice"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{alice.*0r.*0q.*›}}
send "relay add local ws://127.0.0.1:1\r"
expect "local -> ws://127.0.0.1:1"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{alice.*1r.*0q.*›}}
send "account list\r"
expect "known local accounts"
expect -re {{6n}}
send -raw "\033\[1;1R"
expect -re {{alice.*1r.*0q.*›}}
send "quit\r"
expect eof
"#
    );
    let raw = run_pty("env NO_COLOR=1 COLUMNS=88", "", &script);
    assert!(!raw.contains("\x1b[38;"), "NO_COLOR emitted ANSI color");
    let plain = plain_pty(&raw);
    for expected in [
        "fava account",
        "account-imported and selected alice",
        "local -> ws://127.0.0.1:1",
        "known local accounts",
    ] {
        assert!(plain.contains(expected), "missing {expected:?}: {plain:?}");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_completion_exposes_current_pubkey() {
    let raw = run_pty(
        "env NO_COLOR=1 COLUMNS=88",
        "",
        r#"
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {›}
send -raw "query open "
send -raw "\t"
expect -re {CURRENTPUBKEY}
send -raw "\003"
expect -re {cancelled}
expect -re {6n}
send -raw "\033\[1;1R"
expect -re {›}
send "quit\r"
expect eof
"#,
    );
    assert!(
        plain_pty(&raw)
            .to_ascii_lowercase()
            .contains("$currentpubkey"),
        "{raw:?}"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn interactive_pty_color_and_no_color_flag_are_distinct() {
    let color = run_pty(
        "env -u NO_COLOR COLUMNS=88",
        "",
        r#"
expect -re {6n}
send "\033\[1;1R"
expect -re {›}
send "quit\r"
expect eof
"#,
    );
    assert!(color.contains("\x1b[38;") || color.contains("\x1b[1;96m"));
    let plain = run_pty(
        "env -u NO_COLOR COLUMNS=88",
        "--no-color",
        r#"
expect -re {6n}
send "\033\[1;1R"
expect -re {›}
send "quit\r"
expect eof
"#,
    );
    assert!(!plain.contains("\x1b[38;"));
}

#[cfg(target_os = "macos")]
fn run_pty(environment: &str, arguments: &str, commands: &str) -> String {
    let program = format!(
        "set timeout 5\nspawn {environment} $env(FAVA_ACCOUNT_BINARY) {arguments}\nexpect_after timeout {{ puts stderr {{EXPECT TIMEOUT}}; exit 2 }}\n{commands}"
    );
    let output = Command::new("expect")
        .arg("-c")
        .arg(program)
        .env("FAVA_ACCOUNT_BINARY", env!("CARGO_BIN_EXE_account"))
        .output()
        .expect("expect runs the account PTY");
    assert!(
        output.status.success(),
        "expect failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("PTY output is UTF-8")
}

#[cfg(target_os = "macos")]
fn plain_pty(raw: &str) -> String {
    String::from_utf8(strip_ansi_escapes::strip(raw))
        .expect("stripped PTY output is UTF-8")
        .replace('\r', "")
}

fn run(script: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_account"))
        .arg("--jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("account binary starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("script writes");
    child.wait_with_output().expect("account binary exits")
}

fn rows(output: &std::process::Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSONL row"))
        .collect()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
