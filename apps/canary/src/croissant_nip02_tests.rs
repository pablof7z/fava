use std::fs;
use std::path::PathBuf;

use nostr::nips::nip19::ToBech32;
use serde_json::Value;
use tempfile::TempDir;

use super::{
    CroissantNip02Options, SecretScanHook, SecretScanStage, reject_cross_run_data,
    run_croissant_nip02_scenario, run_croissant_nip02_scenario_inner, verify_croissant_run_pair,
};
use crate::croissant_nip02_evidence::{
    artifact_seal, assert_secrets_absent, secret_needles, verify_artifact_seal,
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
    let first_secret = crate::deterministic_keys("croissant-author\0pair-first-private-sentinel")
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
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn generation_secret_scans_reject_direct_injection_at_both_boundaries() {
    for (stage, seed) in [
        (SecretScanStage::BeforeSeal, "pre-seal-secret-scan"),
        (SecretScanStage::AfterManifest, "post-manifest-secret-scan"),
    ] {
        let temporary = TempDir::new().expect("secret injection root");
        let persistent_parent = temporary.path().join("persistent-parent");
        let runs_directory = persistent_parent.join("runs");
        fs::create_dir_all(&runs_directory).expect("persistent runs root");
        let injected = crate::deterministic_keys(&format!("croissant-author\0{seed}"))
            .expect("derived author")
            .secret_key()
            .to_secret_bytes();
        let hook: SecretScanHook = std::sync::Arc::new(move |actual, root, needles| {
            if actual == stage {
                fs::write(root.join("app.stdout.log"), &needles[1])?;
            }
            Ok(())
        });
        let error = Box::pin(run_croissant_nip02_scenario_inner(
            CroissantNip02Options {
                relay_binary: PathBuf::from(CROISSANT),
                scenario_seed: seed.to_owned(),
                runs_directory: runs_directory.clone(),
            },
            Some(hook),
        ))
        .await
        .expect_err("generation scan must reject injected derived secret");
        assert_eq!(
            error.to_string(),
            "retained Croissant evidence contained secret material",
            "wrong causal refusal at {stage:?}"
        );
        assert!(
            fs::read_dir(&runs_directory)
                .expect("persistent runs root remains readable")
                .next()
                .is_none(),
            "a refused run remained beneath the persistent runs root at {stage:?}"
        );
        let parent_entries = fs::read_dir(&persistent_parent)
            .expect("persistent parent remains readable")
            .map(|entry| entry.expect("persistent parent entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            parent_entries,
            [std::ffi::OsString::from("runs")],
            "owner-private staging remained after refusal at {stage:?}"
        );
        assert!(
            !retained_binary(&persistent_parent)
                .windows(injected.len())
                .any(|window| window == injected),
            "injected derived secret remained after refusal at {stage:?}"
        );
    }
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
        assert!(
            error.to_string().contains(field),
            "wrong causal check: {error}"
        );
        fs::write(first_root.join("retained.bin"), b"clean").expect("clean first run");

        fs::write(
            second_root.join("retained.bin"),
            first.1[field].as_str().expect("first identity"),
        )
        .expect("inject first identity into second run");
        let error = reject_cross_run_data(&first, &second)
            .expect_err("second run must reject first-run data");
        assert!(
            error.to_string().contains(field),
            "wrong causal check: {error}"
        );
        fs::write(second_root.join("retained.bin"), b"clean").expect("clean second run");
    }
}

#[test]
fn signed_evidence_rejects_mutated_verification_claims() {
    let keys = nostr::key::Keys::generate();
    let mut manifest = serde_json::json!({
        "scenario": "croissant-nip02-public-flow",
        "scenario_seed_sha256": "seed-hash",
        "author_public_key": keys.public_key().to_hex(),
        "target_public_key": "target",
        "group_id": "aa11",
        "group_event_id": "bb22",
        "baseline_event_id": "cc33",
        "event_id": "dd44",
        "write_id": "run:1",
        "receipt_id": "run:1",
        "write_sequence": 1,
        "receipt_sequence": 1,
        "materialization_id": 1,
        "materialization_source": "cc33",
        "local_revision": 2,
        "relay_revision": 3,
        "foreign_tags_preserved": true,
        "foreign_content_preserved": true,
        "typed_decode_exact": true,
        "secret_scan_passed": true,
        "secret_scan_classes": ["one"],
        "executable_sha256": "binary",
        "source_head": "source",
        "ready": { "endpoint": "127.0.0.1:1" },
        "teardown": {
            "completed": true,
            "pid": 42,
            "pid_alive_after": false,
            "port_open_after": false,
            "endpoint": "127.0.0.1:1"
        },
        "bounds": {
            "operation_ms": 30_000,
            "wire_bytes": 1_048_576,
            "wire_bytes_observed": 12,
            "log_bytes": 1_048_576,
            "readiness_ms": 5_000,
            "readiness_stability_ms": 100,
            "teardown_ms": 5_000
        },
        "terminal": {
            "outcome": "Complete",
            "acknowledged": 1,
            "destinations": 1
        },
        "artifact_sha256": { "flow.json": "11".repeat(32) },
        "started_unix_ms": 10,
        "ended_unix_ms": 20
    });
    manifest["artifact_seal"] =
        serde_json::to_value(artifact_seal(&keys, &manifest).expect("manifest seal"))
            .expect("seal JSON");
    verify_artifact_seal(&manifest).expect("control seal verifies");

    for (pointer, value) in [
        ("/scenario_seed_sha256", serde_json::json!("changed")),
        ("/group_id", serde_json::json!("changed")),
        ("/group_event_id", serde_json::json!("changed")),
        ("/baseline_event_id", serde_json::json!("changed")),
        ("/event_id", serde_json::json!("changed")),
        ("/write_id", serde_json::json!("changed")),
        ("/receipt_id", serde_json::json!("changed")),
        ("/foreign_tags_preserved", serde_json::json!(false)),
        ("/foreign_content_preserved", serde_json::json!(false)),
        ("/typed_decode_exact", serde_json::json!(false)),
        ("/secret_scan_passed", serde_json::json!(false)),
        ("/terminal/outcome", serde_json::json!("Open")),
        ("/terminal/acknowledged", serde_json::json!(0)),
        ("/terminal/destinations", serde_json::json!(0)),
        ("/bounds/operation_ms", serde_json::json!(1)),
        ("/bounds/wire_bytes", serde_json::json!(1)),
        ("/bounds/wire_bytes_observed", serde_json::json!(1)),
        ("/bounds/log_bytes", serde_json::json!(1)),
        ("/bounds/readiness_ms", serde_json::json!(1)),
        ("/bounds/readiness_stability_ms", serde_json::json!(1)),
        ("/bounds/teardown_ms", serde_json::json!(1)),
        ("/teardown/completed", serde_json::json!(false)),
        ("/teardown/pid", serde_json::json!(1)),
        ("/teardown/pid_alive_after", serde_json::json!(true)),
        ("/teardown/port_open_after", serde_json::json!(true)),
        ("/teardown/endpoint", serde_json::json!("127.0.0.1:2")),
    ] {
        let mut changed = manifest.clone();
        *changed.pointer_mut(pointer).expect("claim pointer") = value;
        let error = verify_artifact_seal(&changed)
            .expect_err("signed evidence must reject a mutated verification claim");
        assert!(
            error.to_string().contains("signed manifest claims"),
            "wrong refusal for {pointer}: {error}"
        );
    }
}

#[test]
fn secret_scanner_refuses_every_supported_derived_encoding_directly() {
    let temporary = TempDir::new().expect("secret scan root");
    let author = nostr::key::Keys::generate();
    let target = nostr::key::Keys::generate();
    let needles = secret_needles(b"direct-secret-seed", [&author, &target])
        .expect("derived scanner inventory");
    let mut expected = vec![b"direct-secret-seed".to_vec()];
    for keys in [&author, &target] {
        let secret = keys.secret_key();
        let hex = secret.to_secret_hex();
        let nsec = secret.to_bech32().expect("nsec encoding");
        let upper_nsec = nsec.to_ascii_uppercase();
        expected.extend([
            secret.to_secret_bytes().to_vec(),
            hex.as_bytes().to_vec(),
            hex.to_ascii_uppercase().into_bytes(),
            nsec.as_bytes().to_vec(),
            upper_nsec.as_bytes().to_vec(),
            format!("nostr:{nsec}").into_bytes(),
            format!("nostr:{upper_nsec}").into_bytes(),
            format!("NOSTR:{nsec}").into_bytes(),
            format!("NOSTR:{upper_nsec}").into_bytes(),
        ]);
    }
    assert_eq!(needles, expected, "scanner variant inventory drifted");

    let retained = temporary.path().join("retained.bin");
    for needle in needles {
        fs::write(&retained, &needle).expect("inject direct scanner needle");
        let error = assert_secrets_absent(temporary.path(), std::slice::from_ref(&needle))
            .expect_err("scanner must reject every supported secret representation");
        assert_eq!(
            error.to_string(),
            "retained Croissant evidence contained secret material"
        );
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
    String::from_utf8_lossy(&retained_binary(root)).into_owned()
}

fn retained_binary(root: &std::path::Path) -> Vec<u8> {
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
    bytes
}
