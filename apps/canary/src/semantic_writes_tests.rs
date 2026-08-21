use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::TempDir;

use super::run_semantic_write_scenario;
use crate::SmokeOptions;

async fn run(id: &str) -> Value {
    let directory = TempDir::new().expect("temporary run directory");
    let root = run_semantic_write_scenario(
        id,
        SmokeOptions {
            runs_directory: directory.path().to_path_buf(),
            seed: format!("{id}-seed"),
            relay_binary: PathBuf::from("unused-by-deterministic-m7-canary"),
        },
    )
    .await
    .expect("M7 scenario passes");
    serde_json::from_slice(&fs::read(root.join("semantic.json")).expect("semantic evidence"))
        .expect("valid semantic evidence")
}

#[tokio::test]
async fn replaceable_edit_first_value_records_materialization() {
    let evidence = run("replaceable-edit-first-value").await;
    assert_eq!(evidence["materialization_id"], 1);
    assert!(evidence["source_id"].is_null());
    assert_eq!(evidence["publisher_attempts"], 1);
    assert_eq!(evidence["query_events"], 1);
    assert_eq!(evidence["cache_events"], 0);
}

#[tokio::test]
async fn replaceable_edit_rematerialization_records_retired_inertness() {
    let evidence = run("replaceable-edit-rematerialization").await;
    assert_eq!(evidence["first_materialization_id"], 1);
    assert_eq!(evidence["current_materialization_id"], 2);
    assert_eq!(evidence["retired_materializations"], 1);
    assert_eq!(evidence["publisher_attempts"], 1);
    assert_eq!(evidence["preserved_bob_carol_unrelated"], true);
}

#[tokio::test]
async fn replaceable_edit_inverse_covers_both_capabilities() {
    let evidence = run("replaceable-edit-inverse").await;
    assert_eq!(evidence["operations"], 10);
    assert_eq!(evidence["nip02_final_targets"], 0);
    assert_eq!(evidence["bookmark_final_targets"], 0);
    assert_eq!(evidence["empty_and_adjacent"], true);
    assert_eq!(evidence["publisher_attempts"], 10);
}

#[tokio::test]
async fn protocol_crate_n_plus_one_records_external_and_raw_proofs() {
    let evidence = run("protocol-crate-n-plus-one").await;
    assert_eq!(evidence["external_capability"], true);
    assert_eq!(evidence["raw_future_kind"], true);
    assert_eq!(evidence["future_kind"], 50_001);
    assert_eq!(evidence["product_dependency"], false);
}
