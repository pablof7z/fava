//! Independent capability and product-graph semantic canary.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use fava::{EventValue, Kind, ReplaceableEventMaterializer, Timestamp};
use fava_event_cache_memory::MemoryEventCache;
use fava_signer::Signer;
use nostr::event::Tag;
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

    let mut external_cargo = Command::new("cargo");
    external_cargo
        .args([
            "metadata",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
            manifest.to_string_lossy().as_ref(),
        ])
        .current_dir(&root);
    let external_output = run_owned(external_cargo, Duration::from_secs(60)).await?;
    owned_children_reaped &= external_output.owner_reaped;
    require_success(&external_output, "external locked Cargo metadata")?;
    let external_metadata: Value = serde_json::from_slice(&external_output.stdout)?;
    let external_package_id = external_package_id(&external_metadata, &manifest)?;

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
    let cargo_product_reachable = cargo_reaches_external(&metadata, &external_package_id)?;

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
    let raw = raw_future_attempt(seed).await?;
    Ok(json!({
        "external_manifest": "falsifiers/external-semantic-capability/Cargo.toml",
        "external_capability": true,
        "raw_future_kind": true,
        "future_kind": 50_001,
        "product_dependency": product_dependency,
        "cargo_metadata_locked": true,
        "external_package_id": external_package_id,
        "cargo_product_reachable": cargo_product_reachable,
        "bazel_product_reachable": bazel_product_reachable,
        "owned_children_reaped": owned_children_reaped,
        "attempt": raw["attempt"].clone(),
        "raw_event_id": raw["event_id"].clone(),
        "raw_accepted_event_id": raw["accepted_event_id"].clone(),
        "raw_signed_event_id": raw["signed_event_id"].clone(),
        "raw_published_event_id": raw["published_event_id"].clone(),
        "raw_created_at": raw["created_at"].clone(),
        "raw_content": raw["content"].clone(),
        "raw_tags": raw["tags"].clone(),
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

fn external_package_id(metadata: &Value, manifest: &Path) -> CanaryResult<String> {
    let expected = std::fs::canonicalize(manifest)?;
    let workspace = string_array(metadata, "workspace_members")?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| CanaryError::new("external Cargo metadata omitted packages"))?;
    let mut matches = packages.iter().filter_map(|package| {
        let id = package["id"].as_str()?;
        let package_manifest = package["manifest_path"].as_str()?;
        let canonical = std::fs::canonicalize(package_manifest).ok()?;
        (canonical == expected).then(|| id.to_owned())
    });
    let id = matches
        .next()
        .ok_or_else(|| CanaryError::new("external metadata omitted canonical package"))?;
    if matches.next().is_some() || !workspace.contains(&id) {
        return Err(CanaryError::new(
            "external canonical manifest did not identify one workspace package",
        ));
    }
    Ok(id)
}

fn cargo_reaches_external(metadata: &Value, external_package_id: &str) -> CanaryResult<bool> {
    let workspace = string_array(metadata, "workspace_members")?;
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or_else(|| CanaryError::new("Cargo metadata omitted resolved nodes"))?;
    let mut graph = BTreeMap::new();
    for node in nodes {
        let id = node["id"]
            .as_str()
            .ok_or_else(|| CanaryError::new("Cargo metadata node omitted id"))?;
        let deps = node["deps"]
            .as_array()
            .ok_or_else(|| CanaryError::new("Cargo metadata node omitted deps"))?;
        let mut normal = Vec::new();
        for dependency in deps {
            let dep_kinds = dependency["dep_kinds"]
                .as_array()
                .ok_or_else(|| CanaryError::new("Cargo metadata dependency omitted kinds"))?;
            if dep_kinds
                .iter()
                .any(|kind| kind["kind"].is_null() || kind["kind"] == "normal")
            {
                normal.push(
                    dependency["pkg"]
                        .as_str()
                        .ok_or_else(|| CanaryError::new("Cargo metadata dependency omitted id"))?
                        .to_owned(),
                );
            }
        }
        graph.insert(id.to_owned(), normal);
    }
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
    Ok(reached.contains(external_package_id))
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
        .tag(Tag::parse(["x", "future"]).map_err(error)?)
        .build()
        .map_err(error)?;
    let expected = event
        .id
        .ok_or_else(|| CanaryError::new("future event has no id"))?;
    let accepted = fava
        .publish(explicit_event(event.clone())?)
        .map_err(error)?;
    let receipt = wait_terminal(&fava, accepted.receipt_id).await?;
    let exact_event = crate::semantic_write_support::published_event(&receipt)?;
    let attempts = publisher.attempts();
    let attempt = attempts
        .first()
        .ok_or_else(|| CanaryError::new("future publication attempt missing"))?;
    if accepted.current.id() != expected
        || accepted.current.event != EventValue::Unsigned(event.clone())
        || receipt.current.id() != expected
        || exact_event.id != expected
        || attempt.event != exact_event
        || exact_event.created_at != Timestamp::from(42)
        || exact_event.content != "opaque future content"
        || exact_event.tags != event.tags
    {
        return Err(CanaryError::new(
            "raw future event fields changed across publication lifecycle",
        ));
    }
    Ok(json!({
        "attempt": attempt_evidence(&accepted, &receipt, attempt)?,
        "event_id": expected.to_hex(),
        "accepted_event_id": accepted.current.id().to_hex(),
        "signed_event_id": exact_event.id.to_hex(),
        "published_event_id": attempt.event.id.to_hex(),
        "created_at": exact_event.created_at.as_secs(),
        "content": exact_event.content,
        "tags": exact_event.tags,
    }))
}

fn selected_materializers() -> Vec<Arc<dyn ReplaceableEventMaterializer>> {
    vec![fava_nip02::materializer(), fava_bookmarks::materializer()]
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::cargo_reaches_external;

    #[test]
    fn actual_external_package_is_reachable_over_a_normal_product_edge() {
        let metadata = json!({
            "workspace_members": ["root 0.1.0 (path+file:///repo/root)"],
            "packages": [{
                "id": "path+file:///repo/falsifiers/external-semantic-capability#fava-external-semantic-capability-proof@0.1.0",
                "name": "fava-external-semantic-capability-proof",
                "manifest_path": "/repo/falsifiers/external-semantic-capability/Cargo.toml"
            }],
            "resolve": {"nodes": [
                {"id": "root 0.1.0 (path+file:///repo/root)", "deps": [{
                    "pkg": "path+file:///repo/falsifiers/external-semantic-capability#fava-external-semantic-capability-proof@0.1.0",
                    "dep_kinds": [{"kind": null, "target": null}]
                }]},
                {"id": "path+file:///repo/falsifiers/external-semantic-capability#fava-external-semantic-capability-proof@0.1.0", "deps": []}
            ]}
        });
        let external = "path+file:///repo/falsifiers/external-semantic-capability#fava-external-semantic-capability-proof@0.1.0";
        assert!(cargo_reaches_external(&metadata, external).expect("valid fixture metadata"));
        let mut dev_only = metadata;
        dev_only["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = json!("dev");
        assert!(!cargo_reaches_external(&dev_only, external).expect("valid dev fixture"));
    }
}
