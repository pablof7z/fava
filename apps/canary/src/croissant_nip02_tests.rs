use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use tempfile::TempDir;

use super::{CroissantNip02Options, run_croissant_nip02_scenario, verify_croissant_run_pair};

const CROISSANT: &str = "/Users/pablofernandez/Work/croissant/croissant";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_unique_public_flows_pass_exact_pair_verification() {
    let temporary = TempDir::new().expect("temporary pair root");
    let first = run(
        temporary.path().to_path_buf(),
        "pair-first-private-sentinel",
    )
    .await;
    let second = run(
        temporary.path().to_path_buf(),
        "pair-second-private-sentinel",
    )
    .await;

    assert_ne!(first.run_directory, second.run_directory);
    verify_croissant_run_pair(temporary.path()).expect("exact pair verifies");

    for root in [&first.run_directory, &second.run_directory] {
        let retained = retained_bytes(root);
        assert!(!retained.contains("pair-first-private-sentinel"));
        assert!(!retained.contains("pair-second-private-sentinel"));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pair_verifier_refuses_reuse_old_data_missing_bounds_live_child_and_secret_fields() {
    let temporary = TempDir::new().expect("temporary pair root");
    let first = run(
        temporary.path().to_path_buf(),
        "negative-first-private-sentinel",
    )
    .await;
    let second = run(
        temporary.path().to_path_buf(),
        "negative-second-private-sentinel",
    )
    .await;
    verify_croissant_run_pair(temporary.path()).expect("control pair verifies");

    let manifest_path = second.run_directory.join("manifest.json");
    let original = fs::read(&manifest_path).expect("manifest bytes");
    let first_manifest = manifest(&first.run_directory);

    for mutation in [
        (
            "scenario_seed_sha256",
            first_manifest["scenario_seed_sha256"].clone(),
        ),
        ("group_id", first_manifest["group_id"].clone()),
        ("event_id", first_manifest["event_id"].clone()),
        ("write_id", first_manifest["write_id"].clone()),
        ("receipt_id", first_manifest["receipt_id"].clone()),
        ("artifact_sha256", Value::Null),
        ("bounds", Value::Null),
        ("teardown", serde_json::json!({ "completed": false })),
        (
            "scenario_seed",
            Value::String("forbidden-private-value".to_owned()),
        ),
    ] {
        let mut changed: Value = serde_json::from_slice(&original).expect("manifest JSON");
        changed[mutation.0] = mutation.1;
        fs::write(
            &manifest_path,
            serde_json::to_vec_pretty(&changed).expect("changed JSON"),
        )
        .expect("write mutation");
        assert!(
            verify_croissant_run_pair(temporary.path()).is_err(),
            "pair verifier accepted {} mutation",
            mutation.0
        );
    }
    fs::write(&manifest_path, original).expect("restore manifest");

    fs::write(
        second.run_directory.join("old-data-sentinel.txt"),
        first_manifest["group_id"].as_str().expect("first group"),
    )
    .expect("inject old data");
    assert!(verify_croissant_run_pair(temporary.path()).is_err());
}

async fn run(root: PathBuf, seed: &str) -> super::CroissantNip02Outcome {
    run_croissant_nip02_scenario(CroissantNip02Options {
        relay_binary: PathBuf::from(CROISSANT),
        scenario_seed: seed.to_owned(),
        runs_directory: root,
    })
    .await
    .expect("controlled Croissant flow")
}

fn manifest(root: &std::path::Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest bytes"))
        .expect("manifest JSON")
}

fn retained_bytes(root: &std::path::Path) -> String {
    fn collect(path: &std::path::Path, bytes: &mut Vec<u8>) {
        for entry in fs::read_dir(path).expect("artifact directory") {
            let path = entry.expect("artifact entry").path();
            if path.is_dir() {
                collect(&path, bytes);
            } else {
                bytes.extend(fs::read(path).expect("artifact file"));
            }
        }
    }
    let mut bytes = Vec::new();
    collect(root, &mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}
