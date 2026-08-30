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
}

#[test]
fn current_pubkey_query_opens_once_and_keeps_its_public_id() {
    let script = format!(
        "relay add dead ws://127.0.0.1:1\n\
         account import alice {ALICE_SECRET}\n\
         query open mine $currentPubkey 1 dead\n\
         query snapshot mine\n\
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
    assert_eq!(rows[5]["fields"]["query_ids"], serde_json::json!([1]));
    assert_eq!(rows[6]["fields"]["observation_id"], 1);
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
