use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::TempDir;

use super::run_semantic_write_scenario;
use crate::SmokeOptions;

async fn run_with_seed(id: &str, seed: &str) -> Value {
    let directory = TempDir::new().expect("temporary run directory");
    let root = run_semantic_write_scenario(
        id,
        SmokeOptions {
            runs_directory: directory.path().to_path_buf(),
            seed: seed.to_owned(),
            relay_binary: PathBuf::from("unused-by-deterministic-m7-canary"),
        },
    )
    .await
    .expect("M7 scenario passes");
    serde_json::from_slice(&fs::read(root.join("semantic.json")).expect("semantic evidence"))
        .expect("valid semantic evidence")
}

async fn run(id: &str) -> Value {
    run_with_seed(id, &format!("{id}-seed")).await
}

fn assert_attempt_correlation(attempt: &Value, materialization_id: u64) {
    assert_eq!(attempt["write_id"], attempt["receipt_id"]);
    assert_eq!(attempt["materialization_id"], materialization_id);
    assert_eq!(attempt["event_id"], attempt["receipt_event_id"]);
    assert_eq!(attempt["session"], "wss://m7-semantic.example");
    assert_eq!(attempt["attempt"], 1);
}

#[tokio::test]
async fn replaceable_edit_first_value_records_materialization() {
    let evidence = run("replaceable-edit-first-value").await;
    assert_eq!(evidence["materialization_id"], 1);
    assert!(evidence["source_id"].is_null());
    assert_eq!(evidence["publisher_attempts"], 1);
    assert_eq!(evidence["query_events"], 1);
    assert_eq!(evidence["cache_events"], 0);
    assert_attempt_correlation(&evidence["attempt"], 1);
    assert!(evidence["event_bytes"].as_str().is_some());
}

#[tokio::test]
async fn replaceable_edit_rematerialization_records_retired_inertness() {
    let evidence = run("replaceable-edit-rematerialization").await;
    assert_eq!(evidence["first_materialization_id"], 1);
    assert_eq!(evidence["current_materialization_id"], 2);
    assert_eq!(evidence["retired_materializations"], 1);
    assert_eq!(evidence["publisher_attempts"], 1);
    assert_eq!(evidence["preserved_bob_carol_unrelated"], true);
    assert_eq!(evidence["retired_completion_processed"], true);
    assert_eq!(evidence["retired_stale_effects"], 0);
    assert_attempt_correlation(&evidence["attempt"], 2);
}

#[tokio::test]
async fn replaceable_edit_inverse_covers_both_capabilities() {
    let evidence = run("replaceable-edit-inverse").await;
    assert_eq!(evidence["operations"], 10);
    assert_eq!(evidence["nip02_final_targets"], 0);
    assert_eq!(evidence["bookmark_final_targets"], 0);
    assert_eq!(evidence["empty_and_adjacent"], true);
    assert_eq!(evidence["publisher_attempts"], 10);
    let attempts = evidence["attempts"].as_array().expect("ten exact attempts");
    assert_eq!(attempts.len(), 10);
    for attempt in attempts {
        assert_attempt_correlation(attempt, 1);
    }
}

#[tokio::test]
async fn protocol_crate_n_plus_one_records_external_and_raw_proofs() {
    let evidence = run("protocol-crate-n-plus-one").await;
    assert_eq!(evidence["external_capability"], true);
    assert_eq!(evidence["raw_future_kind"], true);
    assert_eq!(evidence["future_kind"], 50_001);
    assert_eq!(evidence["product_dependency"], false);
    assert_eq!(evidence["cargo_metadata_locked"], true);
    assert_eq!(evidence["cargo_product_reachable"], false);
    assert_eq!(evidence["bazel_product_reachable"], false);
    assert_eq!(evidence["owned_children_reaped"], true);
    assert_attempt_correlation(&evidence["attempt"], 1);
}

#[tokio::test]
async fn same_seed_replays_exact_event_bytes_and_ids() {
    let first = run_with_seed("replaceable-edit-first-value", "same-seed").await;
    let second = run_with_seed("replaceable-edit-first-value", "same-seed").await;
    assert_eq!(first["event_id"], second["event_id"]);
    assert_eq!(first["event_bytes"], second["event_bytes"]);
    assert!(first["event_bytes"].as_str().is_some());
}
