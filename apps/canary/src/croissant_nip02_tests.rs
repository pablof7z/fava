use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use sha2::Digest;
use tempfile::TempDir;

use super::{
    CroissantNip02Options, reject_cross_run_data, run_croissant_nip02_scenario,
    verify_croissant_run_pair,
};

const CROISSANT: &str = "/Users/pablofernandez/Work/croissant/croissant";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_unique_public_flows_pass_exact_pair_verification() {
    let temporary = TempDir::new().expect("temporary pair root");
    let first = Box::pin(run(
        temporary.path().to_path_buf(),
        "pair-first-private-sentinel",
    ))
    .await;
    let second = Box::pin(run(
        temporary.path().to_path_buf(),
        "pair-second-private-sentinel",
    ))
    .await;

    assert_ne!(first.run_directory, second.run_directory);
    verify_croissant_run_pair(temporary.path()).expect("exact pair verifies");

    for root in [&first.run_directory, &second.run_directory] {
        let retained = retained_bytes(root);
        assert!(!retained.contains("pair-first-private-sentinel"));
        assert!(!retained.contains("pair-second-private-sentinel"));
    }
    let first_secret = crate::deterministic_keys(
        "croissant-author\0pair-first-private-sentinel",
    )
    .expect("derived author")
    .secret_key()
    .to_secret_hex();
    assert!(!retained_bytes(&first.run_directory).contains(&first_secret));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pair_verifier_refuses_reuse_old_data_missing_bounds_live_child_and_secret_fields() {
    let temporary = TempDir::new().expect("temporary pair root");
    let first = Box::pin(run(
        temporary.path().to_path_buf(),
        "negative-first-private-sentinel",
    ))
    .await;
    let second = Box::pin(run(
        temporary.path().to_path_buf(),
        "negative-second-private-sentinel",
    ))
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
    fs::write(&manifest_path, &original).expect("restore manifest");

    let derived_secret = crate::deterministic_keys(
        "croissant-author\0negative-second-private-sentinel",
    )
    .expect("derived author")
    .secret_key()
    .to_secret_hex();
    let injected = second.run_directory.join("app.stdout.log");
    let original_artifact = fs::read(&injected).expect("original artifact");
    fs::write(&injected, &derived_secret).expect("inject derived secret");
    refresh_hashes(&second.run_directory);
    assert!(verify_croissant_run_pair(temporary.path()).is_err());
    fs::write(&injected, original_artifact).expect("restore artifact");
    fs::write(&manifest_path, &original).expect("restore sealed manifest");
}

#[test]
fn old_data_negatives_reach_each_identity_check_in_both_directions() {
    let temporary = TempDir::new().expect("temporary pair root");
    let first_root = temporary.path().join("first");
    let second_root = temporary.path().join("second");
    fs::create_dir_all(&first_root).expect("first root");
    fs::create_dir_all(&second_root).expect("second root");
    let first = (
        first_root.clone(),
        identity_manifest("aa11", "bb22", "cc33", "dd44"),
    );
    let second = (
        second_root.clone(),
        identity_manifest("ee55", "ff66", "gg77", "hh88"),
    );

    for field in [
        "group_id",
        "group_event_id",
        "baseline_event_id",
        "event_id",
    ] {
        fs::write(
            first_root.join("retained.bin"),
            second.1[field].as_str().expect("second identity"),
        )
        .expect("inject second identity into first run");
        let error = reject_cross_run_data(&first, &second)
            .expect_err("first run must reject second-run data");
        assert!(error.to_string().contains(field), "wrong causal check: {error}");
        fs::write(first_root.join("retained.bin"), b"clean").expect("clean first run");

        fs::write(
            second_root.join("retained.bin"),
            first.1[field].as_str().expect("first identity"),
        )
        .expect("inject first identity into second run");
        let error = reject_cross_run_data(&first, &second)
            .expect_err("second run must reject first-run data");
        assert!(error.to_string().contains(field), "wrong causal check: {error}");
        fs::write(second_root.join("retained.bin"), b"clean").expect("clean second run");
    }
}

fn identity_manifest(group: &str, group_event: &str, baseline: &str, event: &str) -> Value {
    serde_json::json!({
        "group_id": group,
        "group_event_id": group_event,
        "baseline_event_id": baseline,
        "event_id": event,
    })
}

fn refresh_hashes(root: &std::path::Path) {
    let mut manifest = manifest(root);
    let mut hashes = serde_json::Map::new();
    let mut files = Vec::new();
    collect_files(root, root, &mut files);
    files.sort();
    for relative in files {
        if relative == std::path::Path::new("manifest.json") {
            continue;
        }
        let hash = hex::encode(sha2::Sha256::digest(
            fs::read(root.join(&relative)).expect("artifact bytes"),
        ));
        hashes.insert(relative.to_string_lossy().into_owned(), Value::String(hash));
    }
    manifest["artifact_sha256"] = Value::Object(hashes);
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest JSON"),
    )
    .expect("refresh artifact hashes");
}

fn collect_files(root: &std::path::Path, directory: &std::path::Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("artifact directory") {
        let path = entry.expect("artifact entry").path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(path.strip_prefix(root).expect("relative artifact").to_owned());
        }
    }
}

async fn run(root: PathBuf, seed: &str) -> super::CroissantNip02Outcome {
    Box::pin(run_croissant_nip02_scenario(CroissantNip02Options {
        relay_binary: PathBuf::from(CROISSANT),
        scenario_seed: seed.to_owned(),
        runs_directory: root,
    }))
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
