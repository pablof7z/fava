//! Controlled two-Croissant proof for the public multi-relay simple-groups flow.

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use nostr::key::Keys;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::croissant::{
    CroissantLimits, CroissantReadyFact, CroissantSupervisor, CroissantTeardown,
};
use crate::croissant_simple_groups_evidence::SCENARIO;
use crate::croissant_simple_groups_evidence_support::{
    SECRET_SCAN_CLASSES, artifact_seal, assert_secrets_absent, secret_needles,
};
use crate::croissant_simple_groups_flow::execute_public_flow;
use crate::{
    CanaryError, CanaryResult, RunArtifacts, command_output, deterministic_keys, repository_root,
    unix_ms,
};

/// Process-memory input for one controlled two-relay simple-groups proof.
#[derive(Clone, Debug)]
pub struct CroissantSimpleGroupsOptions {
    /// Croissant executable launched twice without modifying its checkout.
    pub relay_binary: PathBuf,
    /// Croissant source checkout used for exact source-revision evidence.
    pub source_checkout: PathBuf,
    /// Disposable identity seed, never retained outside process memory.
    pub scenario_seed: String,
    /// Parent directory for one fresh durable evidence bundle.
    pub runs_directory: PathBuf,
}

/// Durable location produced by one completed controlled run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CroissantSimpleGroupsOutcome {
    /// Fresh run directory containing the completed manifest and artifacts.
    pub run_directory: PathBuf,
}

#[derive(Debug)]
pub(crate) struct OwnedPairCompletion<T> {
    pub(crate) ready: [CroissantReadyFact; 2],
    pub(crate) teardown: [CroissantTeardown; 2],
    pub(crate) flow: T,
}

#[derive(Debug)]
pub(crate) struct OwnedPairFailure {
    pub(crate) ready: Vec<CroissantReadyFact>,
    pub(crate) teardown: Vec<Result<CroissantTeardown, String>>,
    pub(crate) flow_error: Option<String>,
    pub(crate) startup_error: Option<String>,
}

impl std::fmt::Display for OwnedPairFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "two-child Croissant run failed after {} ready: startup={:?}; flow={:?}; cleanup={:?}",
            self.ready.len(),
            self.startup_error,
            self.flow_error,
            self.teardown
        )
    }
}

impl std::error::Error for OwnedPairFailure {}

pub(crate) fn prepare_owned_supervisors(
    options: &CroissantSimpleGroupsOptions,
    root: &Path,
    relay_keys: &Keys,
    owner_public_keys: [&str; 2],
    limits: CroissantLimits,
) -> CanaryResult<[CroissantSupervisor; 2]> {
    let seed_hash = hex::encode(Sha256::digest(options.scenario_seed.as_bytes()));
    let relay_roots = [root.join("relays/a"), root.join("relays/b")];
    let supervisors = [
        CroissantSupervisor::prepare(
            &options.relay_binary,
            &options.source_checkout,
            &relay_roots[0],
            owner_public_keys[0],
            &seed_hash,
            limits,
        )
        .map_err(error)?,
        CroissantSupervisor::prepare(
            &options.relay_binary,
            &options.source_checkout,
            &relay_roots[1],
            owner_public_keys[1],
            &seed_hash,
            limits,
        )
        .map_err(error)?,
    ];
    let settings = serde_json::to_vec_pretty(&json!({
        "relay_secret_key": relay_keys.secret_key().to_secret_hex(),
    }))?;
    for relay_root in relay_roots {
        fs::write(relay_root.join("data/settings.json"), &settings)?;
    }
    Ok(supervisors)
}

/// Run one fresh two-relay proof and atomically retain it only after cleanup and both scans.
///
/// # Errors
///
/// Returns an attributed staging, child, flow, cleanup, scan, seal, or promotion refusal.
#[allow(
    clippy::too_many_lines,
    reason = "one finalizer keeps staging, cleanup, two scans, sealing, and promotion ordered"
)]
pub async fn run_croissant_simple_groups_scenario(
    options: CroissantSimpleGroupsOptions,
) -> CanaryResult<CroissantSimpleGroupsOutcome> {
    let seed = &options.scenario_seed;
    let author = deterministic_keys(&format!("simple-groups-author\0{seed}"))?;
    let relay = deterministic_keys(&format!("simple-groups-relay\0{seed}"))?;
    let owner_a = deterministic_keys(&format!("simple-groups-owner-a\0{seed}"))?;
    let owner_b = deterministic_keys(&format!("simple-groups-owner-b\0{seed}"))?;
    let target_a = deterministic_keys(&format!("simple-groups-admin-a\0{seed}"))?;
    let target_b = deterministic_keys(&format!("simple-groups-admin-b\0{seed}"))?;
    let mut artifacts = RunArtifacts::create_staged(&options.runs_directory, SCENARIO, seed)?;
    let started = unix_ms()?;
    artifacts.record(
        "scenario_started",
        json!({
            "scenario": SCENARIO,
            "scenario_seed_sha256": hex::encode(Sha256::digest(seed.as_bytes())),
            "author_public_key": author.public_key().to_hex(),
        }),
    )?;
    let supervisors = prepare_owned_supervisors(
        &options,
        artifacts.root(),
        &relay,
        [
            &owner_a.public_key().to_hex(),
            &owner_b.public_key().to_hex(),
        ],
        CroissantLimits::default(),
    )?;
    let flow_root = artifacts.root().to_owned();
    let flow_seed = seed.to_owned();
    let completion = Box::pin(supervise_owned_pair(supervisors, move |ready| {
        Box::pin(async move { Box::pin(execute_public_flow(&flow_root, &flow_seed, ready)).await })
    }))
    .await
    .map_err(error)?;
    artifacts.record("children_ready", &completion.ready)?;
    artifacts.record("children_teardown", &completion.teardown)?;
    artifacts.record("scenario_passed", &completion.flow)?;
    artifacts.write_json(
        "children/processes.json",
        &json!({"ready": completion.ready, "teardown": completion.teardown}),
    )?;
    artifacts.write_json("flow.json", &completion.flow)?;
    for label in ["a", "b"] {
        let data = artifacts.root().join(format!("relays/{label}/data"));
        if data.exists() {
            fs::remove_dir_all(data)?;
        }
    }
    let keys = [&author, &relay, &owner_a, &owner_b, &target_a, &target_b];
    let needles = secret_needles(seed.as_bytes(), &keys)?;
    assert_secrets_absent(artifacts.root(), &needles)?;
    let run_id = artifacts.run_id()?;
    let wire_bytes = ["wire/a.jsonl", "wire/b.jsonl"]
        .into_iter()
        .map(|path| fs::metadata(artifacts.root().join(path)).map(|item| item.len()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<u64>();
    let revision = command_output(&repository_root()?, "git", &["rev-parse", "HEAD"])?;
    let mut manifest = json!({
        "run_id": run_id,
        "scenario": SCENARIO,
        "scenario_seed_sha256": hex::encode(Sha256::digest(seed.as_bytes())),
        "author_public_key": author.public_key().to_hex(),
        "relay_signer_public_key": relay.public_key().to_hex(),
        "relay_owner_public_keys": [owner_a.public_key().to_hex(), owner_b.public_key().to_hex()],
        "group_id": completion.flow.group_id,
        "relay_urls": completion.flow.relay_urls,
        "shared_event_id": completion.flow.shared_event_id,
        "unique_event_ids": completion.flow.unique_event_ids,
        "shared_evidence": completion.flow.shared_evidence,
        "metadata_names": completion.flow.metadata_names,
        "metadata_authors": completion.flow.metadata_authors,
        "admin_targets": completion.flow.admin_targets,
        "admin_authors": completion.flow.admin_authors,
        "custom_event_id": completion.flow.custom_event_id,
        "write_id": format!("{run_id}:{}", completion.flow.write_id),
        "receipt_id": format!("{run_id}:{}", completion.flow.receipt_id),
        "custom_destinations": completion.flow.custom_destinations,
        "custom_acknowledged": completion.flow.custom_acknowledged,
        "handoffs": completion.flow.handoffs,
        "signed_refusals": completion.flow.signed_refusals,
        "observation_closed": completion.flow.observation_closed,
        "ready": completion.ready,
        "teardown": completion.teardown,
        "pre_seal_secret_scan_passed": true,
        "post_manifest_secret_scan_passed": true,
        "secret_scan_classes": SECRET_SCAN_CLASSES,
        "secret_scan_key_count": keys.len(),
        "bounds": {
            "operation_ms": 30_000,
            "wire_bytes": 2_097_152,
            "wire_bytes_observed": wire_bytes,
            "log_bytes": CroissantLimits::default().log_bytes,
            "readiness_ms": CroissantLimits::default().readiness_ms,
            "readiness_stability_ms": CroissantLimits::default().readiness_stability_ms,
            "teardown_ms": CroissantLimits::default().teardown_ms,
        },
        "artifact_sha256": artifacts.artifact_hashes()?,
        "fava_revision": revision,
        "started_unix_ms": started,
        "ended_unix_ms": unix_ms()?,
    });
    let seal = artifact_seal(&author, &manifest)?;
    manifest
        .as_object_mut()
        .ok_or_else(|| CanaryError::new("simple-groups manifest was not an object"))?
        .insert("artifact_seal".to_owned(), serde_json::to_value(seal)?);
    artifacts.write_json("manifest.json", &manifest)?;
    assert_secrets_absent(artifacts.root(), &needles)?;
    let run_directory = artifacts.promote()?;
    Ok(CroissantSimpleGroupsOutcome { run_directory })
}

pub(crate) async fn supervise_owned_pair<T, F, Fut>(
    supervisors: [CroissantSupervisor; 2],
    flow: F,
) -> Result<OwnedPairCompletion<T>, OwnedPairFailure>
where
    F: FnOnce([CroissantReadyFact; 2]) -> Fut,
    Fut: Future<Output = CanaryResult<T>>,
{
    let [supervisor_a, supervisor_b] = supervisors;
    let process_a = supervisor_a
        .start()
        .await
        .map_err(|error| OwnedPairFailure {
            ready: Vec::new(),
            teardown: Vec::new(),
            flow_error: None,
            startup_error: Some(error.to_string()),
        })?;
    let ready_a = process_a.ready_fact();
    let process_b = match supervisor_b.start().await {
        Ok(process) => process,
        Err(error) => {
            let cleanup_a = process_a
                .stop()
                .await
                .map_err(|cleanup| cleanup.to_string());
            return Err(OwnedPairFailure {
                ready: vec![ready_a],
                teardown: vec![cleanup_a],
                flow_error: None,
                startup_error: Some(error.to_string()),
            });
        }
    };
    let ready_b = process_b.ready_fact();
    let ready = [ready_a, ready_b];
    let flow = flow(ready.clone()).await;
    let cleanup_a = process_a.stop().await.map_err(|error| error.to_string());

    let cleanup_b = process_b.stop().await.map_err(|error| error.to_string());
    match (flow, cleanup_a, cleanup_b) {
        (Ok(flow), Ok(teardown_a), Ok(teardown_b)) => Ok(OwnedPairCompletion {
            ready,
            teardown: [teardown_a, teardown_b],
            flow,
        }),
        (flow, cleanup_a, cleanup_b) => Err(OwnedPairFailure {
            ready: ready.into(),
            teardown: vec![cleanup_a, cleanup_b],
            flow_error: flow.err().map(|error| error.to_string()),
            startup_error: None,
        }),
    }
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
