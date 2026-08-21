//! Independent capability and product-graph semantic canary.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use fava::{Kind, ReplaceableEventMaterializer, Timestamp};
use fava_event_cache_memory::MemoryEventCache;
use fava_signer::Signer;
use serde_json::{Value, json};

use crate::semantic_process::{OwnedOutput, run_owned};
use crate::semantic_write_support::{
    DeterministicSigner, RecordingPublisher, assembly, attempt_evidence, explicit_event,
    wait_terminal,
};
use crate::{CanaryError, CanaryResult, deterministic_keys, repository_root};

pub(super) async fn execute(seed: &str) -> CanaryResult<Value> {
    let root = repository_root()?;
    let manifest = root.join("falsifiers/external-semantic-capability/Cargo.toml");
    let mut owned_children_reaped = true;
    for test in [
        "external_capability_composes_through_public_fava",
        "raw_future_event_kind_publishes_unchanged",
    ] {
        let mut command = Command::new("cargo");
        command
            .args([
                "test",
                "--locked",
                "--manifest-path",
                manifest.to_string_lossy().as_ref(),
                "--test",
                "public_capability",
                test,
                "--",
                "--exact",
            ])
            .current_dir(&root);
        let output = run_owned(command, Duration::from_secs(60)).await?;
        owned_children_reaped &= output.owner_reaped;
        require_success(&output, &format!("external proof {test}"))?;
    }

    let root_manifest = root.join("Cargo.toml");
    let mut cargo = Command::new("cargo");
    cargo
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--manifest-path",
            root_manifest.to_string_lossy().as_ref(),
        ])
        .current_dir(&root);
    let cargo_output = run_owned(cargo, Duration::from_secs(60)).await?;
    owned_children_reaped &= cargo_output.owner_reaped;
    require_success(&cargo_output, "locked Cargo metadata")?;
    let metadata: Value = serde_json::from_slice(&cargo_output.stdout)?;
    let cargo_product_reachable = cargo_reaches_external(&metadata)?;

    let mut bazel = Command::new("bazel");
    bazel
        .args(["query", "deps(//...)", "--noshow_progress"])
        .current_dir(&root);
    let bazel_output = run_owned(bazel, Duration::from_secs(60)).await?;
    owned_children_reaped &= bazel_output.owner_reaped;
    require_success(&bazel_output, "Bazel product graph")?;
    let bazel_product_reachable =
        String::from_utf8_lossy(&bazel_output.stdout).contains("external-semantic-capability");
    let product_dependency = cargo_product_reachable || bazel_product_reachable;
    if product_dependency {
        return Err(CanaryError::new(
            "external capability entered the product graph",
        ));
    }
    let attempt = raw_future_attempt(seed).await?;
    Ok(json!({
        "external_manifest": "falsifiers/external-semantic-capability/Cargo.toml",
        "external_capability": true,
        "raw_future_kind": true,
        "future_kind": 50_001,
        "product_dependency": product_dependency,
        "cargo_metadata_locked": true,
        "cargo_product_reachable": cargo_product_reachable,
        "bazel_product_reachable": bazel_product_reachable,
        "owned_children_reaped": owned_children_reaped,
        "attempt": attempt,
    }))
}

fn require_success(output: &OwnedOutput, label: &str) -> CanaryResult<()> {
    if output.status.success() {
        return Ok(());
    }
    Err(CanaryError::new(format!(
        "{label} failed with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn cargo_reaches_external(metadata: &Value) -> CanaryResult<bool> {
    let workspace = string_array(metadata, "workspace_members")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| CanaryError::new("Cargo metadata omitted packages"))?;
    let external = packages
        .iter()
        .filter(|package| package["name"] == "external-semantic-capability")
        .filter_map(|package| package["id"].as_str().map(str::to_owned))
        .collect::<BTreeSet<_>>();
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or_else(|| CanaryError::new("Cargo metadata omitted resolved nodes"))?;
    let graph = nodes
        .iter()
        .filter_map(|node| {
            let id = node["id"].as_str()?.to_owned();
            let dependencies = node["deps"]
                .as_array()?
                .iter()
                .filter_map(|dependency| dependency["pkg"].as_str().map(str::to_owned))
                .collect::<Vec<_>>();
            Some((id, dependencies))
        })
        .collect::<BTreeMap<_, _>>();
    let mut pending = VecDeque::from(workspace);
    let mut reached = BTreeSet::new();
    while let Some(package) = pending.pop_front() {
        if !reached.insert(package.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&package) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    Ok(!external.is_disjoint(&reached))
}

fn string_array(value: &Value, field: &str) -> CanaryResult<Vec<String>> {
    value[field]
        .as_array()
        .ok_or_else(|| CanaryError::new(format!("Cargo metadata omitted {field}")))?
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CanaryError::new(format!("Cargo metadata {field} was not text")))
        })
        .collect()
}

async fn raw_future_attempt(seed: &str) -> CanaryResult<Value> {
    let keys = deterministic_keys(&format!("{seed}-future-actor"))?;
    let publisher = Arc::new(RecordingPublisher::default());
    let signer: Arc<dyn Signer> = Arc::new(DeterministicSigner::new(keys.clone()));
    let (fava, _completions) = assembly(
        Arc::new(MemoryEventCache::default()),
        signer,
        selected_materializers(),
        Arc::clone(&publisher),
    )?;
    let event = fava::EventBuilder::new(keys.public_key(), Kind::Custom(50_001))
        .created_at(Timestamp::from(42))
        .content("opaque future content")
        .build()
        .map_err(error)?;
    let expected = event
        .id
        .ok_or_else(|| CanaryError::new("future event has no id"))?;
    let accepted = fava.publish(explicit_event(event)?).map_err(error)?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    if receipt.current.id() != expected {
        return Err(CanaryError::new("raw future event changed"));
    }
    let attempts = publisher.attempts();
    let attempt = attempts
        .first()
        .ok_or_else(|| CanaryError::new("future publication attempt missing"))?;
    attempt_evidence(&accepted, &receipt, attempt)
}

fn selected_materializers() -> Vec<Arc<dyn ReplaceableEventMaterializer>> {
    vec![fava_nip02::materializer(), fava_bookmarks::materializer()]
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
