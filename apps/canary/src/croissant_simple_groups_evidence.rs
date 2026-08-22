//! Safe retained evidence and independent-pair verification for the two-Croissant proof.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use fava::RelayUrl;
use nostr::event::Event;
use serde_json::{Map, Value};

use crate::croissant_simple_groups_evidence_semantics::verify_semantic_artifacts;
use crate::croissant_simple_groups_evidence_support::{
    EvidenceSnapshot, MAX_MANIFEST_BYTES, SECRET_SCAN_CLASSES, signed_digest,
};
use crate::{CanaryError, CanaryResult};

pub(crate) const SCENARIO: &str = "croissant-simple-groups-public-flow";

/// Verify exactly two safe, independent retained runs containing four exact child facts.
///
/// # Errors
///
/// Returns a redacted refusal for unsafe, incomplete, reused, or tampered evidence.
pub fn verify_croissant_simple_groups_pair(
    runs_directory: impl AsRef<Path>,
    expected_revision: &str,
) -> CanaryResult<()> {
    if expected_revision.is_empty() {
        return Err(CanaryError::new(
            "simple-groups expected Fava revision was empty",
        ));
    }
    let root = runs_directory.as_ref();
    let roots = run_roots(root)?;
    if roots.len() != 2 {
        return Err(CanaryError::new(
            "simple-groups pair must contain exactly two manifests",
        ));
    }
    let mut runs = Vec::new();
    for run_root in roots {
        let snapshot = EvidenceSnapshot::capture(&run_root)?;
        let manifest: Value = serde_json::from_slice(snapshot.read(
            Path::new("manifest.json"),
            MAX_MANIFEST_BYTES,
            "manifest",
        )?)?;
        reject_secret_fields(&manifest)?;
        validate_manifest(&snapshot, &manifest, expected_revision)?;
        runs.push((snapshot, manifest));
    }
    verify_pair_identity(&runs)?;
    reject_cross_run_data(&runs[0], &runs[1])
}

fn run_roots(root: &Path) -> CanaryResult<Vec<std::path::PathBuf>> {
    let mut roots = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !entry.file_type()?.is_dir() || name.starts_with(".fava-canary-staging-") {
            return Err(CanaryError::new(
                "simple-groups pair root contained staging or non-run residue",
            ));
        }
        roots.push(entry.path());
    }
    roots.sort();
    Ok(roots)
}

fn validate_manifest(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    expected_revision: &str,
) -> CanaryResult<()> {
    verify_hashes(snapshot, manifest)?;
    verify_seal(manifest)?;
    if required_string(manifest, "scenario")? != SCENARIO
        || manifest
            .get("pre_seal_secret_scan_passed")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest
            .get("post_manifest_secret_scan_passed")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest.get("observation_closed").and_then(Value::as_bool) != Some(true)
        || manifest.get("signed_refusals").and_then(Value::as_u64) != Some(3)
    {
        return Err(CanaryError::new(
            "simple-groups manifest completion claims were incomplete",
        ));
    }
    if manifest
        .get("secret_scan_key_count")
        .and_then(Value::as_u64)
        != Some(6)
    {
        return Err(CanaryError::new(
            "simple-groups manifest secret scan key count was incomplete",
        ));
    }
    verify_scan_classes(manifest)?;
    for field in [
        "run_id",
        "scenario_seed_sha256",
        "author_public_key",
        "relay_signer_public_key",
        "group_id",
        "shared_event_id",
        "custom_event_id",
        "write_id",
        "receipt_id",
        "fava_revision",
    ] {
        if required_string(manifest, field)?.is_empty() {
            return Err(CanaryError::new(format!(
                "simple-groups manifest omitted {field}"
            )));
        }
    }
    if required_string(manifest, "fava_revision")? != expected_revision {
        return Err(CanaryError::new(
            "simple-groups evidence revision did not match the expected revision",
        ));
    }
    verify_flow_claims(manifest)?;
    verify_bounds(manifest)?;
    verify_children(manifest)?;
    verify_semantic_artifacts(snapshot, manifest)
}

fn verify_flow_claims(manifest: &Value) -> CanaryResult<()> {
    let relay_urls = exact_strings(manifest, "relay_urls", 2)?;
    let evidence = exact_strings(manifest, "shared_evidence", 2)?;
    let owners = exact_strings(manifest, "relay_owner_public_keys", 2)?;
    let metadata = exact_strings(manifest, "metadata_names", 2)?;
    let metadata_authors = exact_strings(manifest, "metadata_authors", 2)?;
    let admin_targets = exact_strings(manifest, "admin_targets", 2)?;
    let admin_authors = exact_strings(manifest, "admin_authors", 2)?;
    let unique = exact_strings(manifest, "unique_event_ids", 2)?;
    let relay_signer = required_string(manifest, "relay_signer_public_key")?;
    let shared = required_string(manifest, "shared_event_id")?;
    let custom = required_string(manifest, "custom_event_id")?;
    let parsed_relays = relay_urls
        .iter()
        .map(|url| RelayUrl::parse(url).map_err(error))
        .collect::<CanaryResult<Vec<_>>>()?;
    if parsed_relays[0] == parsed_relays[1]
        || relay_urls.iter().collect::<BTreeSet<_>>() != evidence.iter().collect::<BTreeSet<_>>()
        || owners[0] == owners[1]
        || metadata[0] == metadata[1]
        || metadata_authors != [relay_signer, relay_signer]
        || admin_targets[0] == admin_targets[1]
        || admin_authors != [relay_signer, relay_signer]
        || unique[0] == unique[1]
        || unique.iter().any(|id| id == shared)
        || custom == shared
        || unique.iter().any(|id| id == custom)
        || exact_u64s(manifest, "handoffs", 2)? != [1, 1]
        || manifest.get("custom_destinations").and_then(Value::as_u64) != Some(2)
        || manifest.get("custom_acknowledged").and_then(Value::as_u64) != Some(2)
    {
        return Err(CanaryError::new(
            "simple-groups manifest flow claims were incomplete",
        ));
    }
    Ok(())
}

fn verify_bounds(manifest: &Value) -> CanaryResult<()> {
    let bounds = manifest
        .get("bounds")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("simple-groups manifest omitted bounds"))?;
    for field in [
        "operation_ms",
        "wire_bytes",
        "log_bytes",
        "readiness_ms",
        "readiness_stability_ms",
        "teardown_ms",
    ] {
        if bounds.get(field).and_then(Value::as_u64).unwrap_or(0) == 0 {
            return Err(CanaryError::new(format!(
                "simple-groups manifest omitted {field}"
            )));
        }
    }
    if bounds
        .get("wire_bytes_observed")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
        > bounds
            .get("wire_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    {
        return Err(CanaryError::new(
            "simple-groups wire evidence exceeded its bound",
        ));
    }
    Ok(())
}

fn verify_children(manifest: &Value) -> CanaryResult<()> {
    let ready = exact_objects(manifest, "ready", 2)?;
    let teardown = exact_objects(manifest, "teardown", 2)?;
    let mut pids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for index in 0..2 {
        let pid = ready[index].get("pid").and_then(Value::as_u64).unwrap_or(0);
        let endpoint = object_string(ready[index], "endpoint")?;
        let data_path = object_string(ready[index], "data_path")?;
        if pid == 0
            || pid == 75_649
            || !pids.insert(pid)
            || !endpoints.insert(endpoint)
            || !paths.insert(data_path)
        {
            return Err(CanaryError::new(
                "simple-groups child identities were reused",
            ));
        }
        if ready[index]
            .get("readiness_completed")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(CanaryError::new(
                "simple-groups child readiness was incomplete",
            ));
        }
        let limits = ready[index]
            .get("limits")
            .and_then(Value::as_object)
            .ok_or_else(|| CanaryError::new("simple-groups child omitted readiness limits"))?;
        let bounds = manifest
            .get("bounds")
            .and_then(Value::as_object)
            .ok_or_else(|| CanaryError::new("simple-groups manifest omitted bounds"))?;
        for field in [
            "log_bytes",
            "readiness_ms",
            "readiness_stability_ms",
            "teardown_ms",
        ] {
            if limits.get(field) != bounds.get(field) {
                return Err(CanaryError::new(
                    "simple-groups child limits disagreed with manifest bounds",
                ));
            }
        }
        for field in [
            "executable",
            "executable_sha256",
            "source_checkout",
            "source_head",
            "scenario_seed_sha256",
            "stdout_path",
            "stderr_path",
        ] {
            if object_string(ready[index], field)?.is_empty() {
                return Err(CanaryError::new(
                    "simple-groups readiness provenance was incomplete",
                ));
            }
        }
        if object_string(ready[index], "scenario_seed_sha256")?
            != required_string(manifest, "scenario_seed_sha256")?
        {
            return Err(CanaryError::new(
                "simple-groups readiness seed digest disagreed with its run",
            ));
        }
        let stopped_pid = teardown[index]
            .get("pid")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if stopped_pid != pid
            || object_string(teardown[index], "endpoint")? != endpoint
            || teardown[index].get("completed").and_then(Value::as_bool) != Some(true)
            || teardown[index]
                .get("pid_alive_after")
                .and_then(Value::as_bool)
                != Some(false)
            || teardown[index]
                .get("port_open_after")
                .and_then(Value::as_bool)
                != Some(false)
        {
            return Err(CanaryError::new(
                "simple-groups child teardown was incomplete",
            ));
        }
    }
    Ok(())
}

fn verify_pair_identity(runs: &[(EvidenceSnapshot, Value)]) -> CanaryResult<()> {
    for field in [
        "scenario_seed_sha256",
        "author_public_key",
        "group_id",
        "write_id",
        "receipt_id",
    ] {
        if required_string(&runs[0].1, field)? == required_string(&runs[1].1, field)? {
            return Err(CanaryError::new(format!(
                "simple-groups pair reused {field}"
            )));
        }
    }
    let first_events = event_identities(&runs[0].1)?;
    let second_events = event_identities(&runs[1].1)?;
    if !first_events.is_disjoint(&second_events) {
        return Err(CanaryError::new(
            "simple-groups pair reused an event identity",
        ));
    }
    let mut pids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (_, manifest) in runs {
        for child in exact_objects(manifest, "ready", 2)? {
            if !pids.insert(child.get("pid").and_then(Value::as_u64).unwrap_or(0))
                || !endpoints.insert(object_string(child, "endpoint")?.to_owned())
                || !paths.insert(object_string(child, "data_path")?.to_owned())
            {
                return Err(CanaryError::new(
                    "simple-groups pair reused a child identity",
                ));
            }
        }
    }
    Ok(())
}

fn reject_cross_run_data(
    first: &(EvidenceSnapshot, Value),
    second: &(EvidenceSnapshot, Value),
) -> CanaryResult<()> {
    let first_group = required_string(&first.1, "group_id")?.to_owned();
    let second_group = required_string(&second.1, "group_id")?.to_owned();
    let first_identities = event_identities(&first.1)?;
    let second_identities = event_identities(&second.1)?;
    if tree_contains(&first.0, second_group.as_bytes(), true)?
        || tree_contains(&second.0, first_group.as_bytes(), true)?
    {
        return Err(CanaryError::new(
            "simple-groups run retained the other run's group_id",
        ));
    }
    for identity in &second_identities {
        if tree_contains(&first.0, identity.as_bytes(), true)? {
            return Err(CanaryError::new(
                "simple-groups run retained the other run's event identity",
            ));
        }
    }
    for identity in &first_identities {
        if tree_contains(&second.0, identity.as_bytes(), true)? {
            return Err(CanaryError::new(
                "simple-groups run retained the other run's event identity",
            ));
        }
    }
    Ok(())
}

fn event_identities(manifest: &Value) -> CanaryResult<BTreeSet<String>> {
    let mut identities = BTreeSet::from([
        required_string(manifest, "shared_event_id")?.to_owned(),
        required_string(manifest, "custom_event_id")?.to_owned(),
    ]);
    for identity in exact_strings(manifest, "unique_event_ids", 2)? {
        identities.insert(identity);
    }
    if identities.len() != 4 {
        return Err(CanaryError::new(
            "simple-groups run reused an event identity",
        ));
    }
    Ok(identities)
}

fn verify_hashes(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let expected = manifest
        .get("artifact_sha256")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("simple-groups manifest omitted artifact hashes"))?;
    let actual = snapshot
        .artifact_hashes()?
        .into_iter()
        .map(|(path, hash)| (path, Value::String(hash)))
        .collect::<Map<_, _>>();
    if expected.is_empty() || expected != &actual {
        return Err(CanaryError::new(
            "simple-groups artifact hashes did not verify",
        ));
    }
    Ok(())
}

fn verify_seal(manifest: &Value) -> CanaryResult<()> {
    let seal: Event = serde_json::from_value(
        manifest
            .get("artifact_seal")
            .cloned()
            .ok_or_else(|| CanaryError::new("simple-groups manifest omitted artifact seal"))?,
    )?;
    seal.verify().map_err(error)?;
    if seal.pubkey.to_hex() != required_string(manifest, "author_public_key")?
        || seal.content != signed_digest(manifest)?
    {
        return Err(CanaryError::new(
            "simple-groups artifact seal did not verify",
        ));
    }
    Ok(())
}

fn verify_scan_classes(manifest: &Value) -> CanaryResult<()> {
    let values = manifest
        .get("secret_scan_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new("simple-groups manifest omitted scan classes"))?;
    if !values
        .iter()
        .map(Value::as_str)
        .eq(SECRET_SCAN_CLASSES.iter().copied().map(Some))
    {
        return Err(CanaryError::new(
            "simple-groups secret scan classes were incomplete",
        ));
    }
    Ok(())
}

fn reject_secret_fields(value: &Value) -> CanaryResult<()> {
    match value {
        Value::Object(map) => {
            if map.keys().any(|key| {
                ["scenario_seed", "raw_seed", "private_key", "secret_key"].contains(&key.as_str())
            }) {
                return Err(CanaryError::new(
                    "simple-groups manifest contained a secret field",
                ));
            }
            for child in map.values() {
                reject_secret_fields(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_secret_fields(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

include!("croissant_simple_groups_evidence/value_support.rs");
