//! Black-box replay evidence through the real relay-auth binary.
//!
//! These tests never touch a real relay: `query open` opens instantly from
//! local state (LOCAL-08) even against an unreachable relay alias, and no
//! command here calls `publish`, which needs a real relay round trip. Full
//! live proof, including every authentication-state transition, lives in
//! `live/harness.py`.

use std::io::Write;
use std::process::{Command, Stdio};

const ALICE_SECRET: &str = "0000000000000000000000000000000000000000000000000000000000000001";
const ALICE_PUBKEY: &str = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";

#[test]
fn replay_exercises_policy_auth_and_query_grammar() {
    let script = include_str!("../scenarios/policy-and-demands.txt");
    let output = run(script);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let rows = rows(&output);
    let kinds: Vec<&str> = rows
        .iter()
        .map(|row| row["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "relay-added",
            "account-imported",
            "account-imported",
            "policy-set",
            "auth-pending",
            "auth-state",
            "auth-state",
            "policy-set",
            "policy-set",
            "query-opened",
            "query-snapshot",
            "query-opened",
            "query-snapshot",
            "query-closed",
            "query-closed",
            "routes",
            "diagnostics",
            "receipt-list",
            "account-selected",
            "diagnostics",
            "account-cleared",
            "help",
            "dump",
            "quit",
        ]
    );
    assert!(rows.iter().all(|row| row["status"] == "ok"));
    let pending = &rows[4]["fields"];
    assert_eq!(pending["count"], 0);
    assert_eq!(pending["first_id"], 0);
    assert_eq!(rows[5]["fields"]["state"], "unknown");
    assert_eq!(rows[5]["fields"]["access"], "as:alice");
    assert_eq!(rows[6]["fields"]["access"], "public");
}

#[test]
fn query_open_distinguishes_public_and_authenticated_sessions_on_one_relay() {
    let script = format!(
        "relay add dead ws://127.0.0.1:1\n\
         account import alice {ALICE_SECRET}\n\
         query open pub public 1 dead\n\
         query open mine as:alice 1 dead\n\
         query snapshot pub\n\
         query snapshot mine\n\
         routes\n\
         quit\n"
    );
    let output = run(&script);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let rows = rows(&output);
    let pub_id = rows[2]["fields"]["observation_id"].as_u64().unwrap();
    let mine_id = rows[3]["fields"]["observation_id"].as_u64().unwrap();
    assert_ne!(
        pub_id, mine_id,
        "public and authenticated access open distinct sessions"
    );
    assert_eq!(rows[2]["fields"]["access"], "public");
    assert_eq!(rows[3]["fields"]["access"], "as:alice");
}

#[test]
fn publish_public_authors_the_current_account_against_an_unreachable_relay() {
    // Publication is accepted and reported immediately, not once it settles
    // -- see `App::publish`'s doc -- so this needs no reachable relay:
    // unlike `publish as:<account>`, plain public routing has no
    // `Authenticator::watch_session` to await before the write is accepted.
    let script = format!(
        "relay add dead ws://127.0.0.1:1\n\
         account import alice {ALICE_SECRET}\n\
         publish public 1 \"hello\" dead\n\
         quit\n"
    );
    let output = run(&script);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let rows = rows(&output);
    assert_eq!(rows[2]["kind"], "published");
    assert_eq!(rows[2]["fields"]["author"], ALICE_PUBKEY);
    assert_eq!(rows[2]["fields"]["access"], "public");
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

#[test]
fn unknown_demand_id_is_a_typed_domain_refusal_not_a_crash() {
    let output = run("auth answer 9 decline\nquit\n");
    assert!(!output.status.success());
    let rows = rows(&output);
    assert_eq!(rows[0]["kind"], "domain-failed");
    assert!(
        rows[0]["summary"]
            .as_str()
            .unwrap()
            .contains("no demand awaits this answer")
    );
}

fn run(script: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_relay-auth"))
        .arg("--jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("relay-auth binary starts");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("script writes");
    child.wait_with_output().expect("relay-auth binary exits")
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
