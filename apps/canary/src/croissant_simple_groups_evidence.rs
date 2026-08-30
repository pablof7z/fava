//! Safe retained evidence and independent-pair verification for the two-Croissant proof.
//!
//! This file stays above the 500-line soft limit so pair-root boundedness, exact source and child
//! provenance, cross-run exclusion, and the final semantic handoff remain one verifier authority.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use fava::RelayUrl;
use nostr::event::Event;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::croissant_simple_groups_evidence_semantics::verify_semantic_artifacts;
use crate::croissant_simple_groups_evidence_support::{
    EvidenceSnapshot, MAX_MANIFEST_BYTES, signed_digest,
};
use crate::croissant_simple_groups_source::is_lower_hex;
use crate::{CanaryError, CanaryResult};

pub(crate) const SCENARIO: &str = "croissant-simple-groups-public-flow";

/// Verify exactly two safe, independent retained runs containing four exact child facts.
///
/// # Errors
///
/// Returns an attributed refusal for unsafe, incomplete, reused, or tampered evidence.
#[allow(
    clippy::too_many_arguments,
    reason = "independent callers must supply every exact Fava and Croissant identity"
)]
pub fn verify_croissant_simple_groups_pair(
    runs_directory: impl AsRef<Path>,
    expected_fava_revision: &str,
    expected_fava_source_tree_sha256: &str,
    expected_fava_build_tree: &str,
    expected_fava_build_source_image_sha256: &str,
    expected_fava_build_source_manifest_sha256: &str,
    expected_fava_rust_base_image_sha256: &str,
    expected_fava_canary_executable_sha256: &str,
    expected_fava_canary_subject_image_sha256: &str,
    expected_croissant_revision: &str,
    expected_croissant_executable_sha256: &str,
) -> CanaryResult<()> {
    if !is_lower_hex(expected_fava_revision, 40)
        || !is_lower_hex(expected_fava_source_tree_sha256, 64)
        || !is_lower_hex(expected_fava_build_tree, 40)
        || !is_lower_hex(expected_fava_build_source_image_sha256, 64)
        || expected_fava_build_source_image_sha256
            .bytes()
            .all(|byte| byte == b'0')
        || !is_lower_hex(expected_fava_build_source_manifest_sha256, 64)
        || expected_fava_build_source_manifest_sha256
            .bytes()
            .all(|byte| byte == b'0')
        || !is_lower_hex(expected_fava_rust_base_image_sha256, 64)
        || expected_fava_rust_base_image_sha256
            .bytes()
            .all(|byte| byte == b'0')
        || !is_lower_hex(expected_fava_canary_executable_sha256, 64)
        || !is_lower_hex(expected_fava_canary_subject_image_sha256, 64)
        || expected_fava_canary_subject_image_sha256
            .bytes()
            .all(|byte| byte == b'0')
        || !is_lower_hex(expected_croissant_revision, 40)
        || !is_lower_hex(expected_croissant_executable_sha256, 64)
    {
        return Err(CanaryError::new(
            "simple-groups expected source provenance was not exact lowercase hex",
        ));
    }
    let root = runs_directory.as_ref();
    let roots = run_roots(root)?;
    let mut runs = Vec::new();
    for run_root in roots {
        let directory_run_id = run_root
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| CanaryError::new("simple-groups run directory was not UTF-8"))?;
        let snapshot = EvidenceSnapshot::capture(&run_root)?;
        reject_relay_artifact_residue(&snapshot)?;
        let manifest: Value = serde_json::from_slice(snapshot.read(
            Path::new("manifest.json"),
            MAX_MANIFEST_BYTES,
            "manifest",
        )?)?;
        validate_manifest(
            &snapshot,
            &manifest,
            directory_run_id,
            expected_fava_revision,
            expected_fava_source_tree_sha256,
            expected_fava_build_tree,
            expected_fava_build_source_image_sha256,
            expected_fava_build_source_manifest_sha256,
            expected_fava_rust_base_image_sha256,
            expected_fava_canary_executable_sha256,
            expected_fava_canary_subject_image_sha256,
            expected_croissant_revision,
            expected_croissant_executable_sha256,
        )?;
        runs.push((snapshot, manifest));
    }
    verify_pair_identity(&runs)?;
    reject_cross_run_data(&runs[0], &runs[1])
}

fn reject_relay_artifact_residue(snapshot: &EvidenceSnapshot) -> CanaryResult<()> {
    for relative in snapshot.files() {
        for label in ["a", "b"] {
            if let Ok(child) = relative.strip_prefix(format!("relays/{label}"))
                && child != Path::new("stdout.log")
                && child != Path::new("stderr.log")
            {
                return Err(CanaryError::new(
                    "simple-groups retained an unowned relay artifact",
                ));
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one manifest oracle binds all retained semantic and provenance claims"
)]
fn validate_manifest(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    directory_run_id: &str,
    expected_fava_revision: &str,
    expected_fava_source_tree_sha256: &str,
    expected_fava_build_tree: &str,
    expected_fava_build_source_image_sha256: &str,
    expected_fava_build_source_manifest_sha256: &str,
    expected_fava_rust_base_image_sha256: &str,
    expected_fava_canary_executable_sha256: &str,
    expected_fava_canary_subject_image_sha256: &str,
    expected_croissant_revision: &str,
    expected_croissant_executable_sha256: &str,
) -> CanaryResult<()> {
    verify_hashes(snapshot, manifest)?;
    verify_seal(manifest)?;
    if required_string(manifest, "scenario")? != SCENARIO
        || manifest.get("observation_closed").and_then(Value::as_bool) != Some(true)
        || manifest.get("prepared_contexts").and_then(Value::as_u64) != Some(3)
    {
        return Err(CanaryError::new(
            "simple-groups manifest completion claims were incomplete",
        ));
    }
    for field in [
        "run_id",
        "scenario_seed_sha256",
        "author_public_key",
        "relay_signer_public_key",
        "simple_group_id",
        "shared_event_id",
        "custom_event_id",
        "custom_event_signature",
        "write_id",
        "receipt_id",
        "fava_revision",
        "fava_source_tree_sha256",
        "fava_build_revision",
        "fava_build_tree",
        "fava_build_source_tree_sha256",
        "fava_build_source_manifest_sha256",
        "fava_build_source_image_sha256",
        "fava_build_rust_base_image_sha256",
        "fava_build_command_sha256",
        "fava_build_target_storage",
        "fava_build_subject_digest_origin",
        "fava_canary_subject_image_sha256",
        "fava_build_source_transport",
        "fava_build_source_transport_image_sha256",
    ] {
        if required_string(manifest, field)?.is_empty() {
            return Err(CanaryError::new(format!(
                "simple-groups manifest omitted {field}"
            )));
        }
    }
    if required_string(manifest, "run_id")? != directory_run_id
        || required_string(manifest, "fava_revision")? != expected_fava_revision
        || required_string(manifest, "fava_source_tree_sha256")? != expected_fava_source_tree_sha256
        || required_string(manifest, "fava_canary_executable_sha256")?
            != expected_fava_canary_executable_sha256
        || required_string(manifest, "fava_canary_subject_image_sha256")?
            != expected_fava_canary_subject_image_sha256
        || required_string(manifest, "fava_build_revision")? != expected_fava_revision
        || required_string(manifest, "fava_build_tree")? != expected_fava_build_tree
        || required_string(manifest, "fava_build_source_tree_sha256")?
            != expected_fava_source_tree_sha256
        || required_string(manifest, "fava_build_source_image_sha256")?
            != expected_fava_build_source_image_sha256
        || required_string(manifest, "fava_build_source_manifest_sha256")?
            != expected_fava_build_source_manifest_sha256
        || required_string(manifest, "fava_build_rust_base_image_sha256")?
            != expected_fava_rust_base_image_sha256
        || required_string(manifest, "fava_build_command_sha256")?
            != "8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08"
        || required_string(manifest, "fava_build_target_storage")?
            != "engine-content-addressed-image"
        || manifest
            .get("fava_build_target_maximum_bytes")
            .and_then(Value::as_u64)
            != Some(4_294_967_296)
        || required_string(manifest, "fava_build_subject_digest_origin")? != "engine-image"
        || required_string(manifest, "fava_build_source_transport")? != "owned-loopback-registry"
        || required_string(manifest, "fava_build_source_transport_image_sha256")?
            != crate::pinned_build_input::REGISTRY_IMAGE_SHA256
        || manifest
            .get("fava_build_source_immutable")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest.get("fava_source_clean").and_then(Value::as_bool) != Some(true)
        || required_string(manifest, "fava_execution_platform")? != "linux-sealed-memfd-proc-fd"
        || required_string(manifest, "execution_platform")? != "linux-sealed-memfd-container"
    {
        return Err(CanaryError::new(
            "simple-groups evidence run/source provenance did not match its exact expectation",
        ));
    }
    verify_source_provenance(snapshot, manifest)?;
    verify_flow_claims(manifest)?;
    verify_bounds(manifest)?;
    verify_children(
        manifest,
        expected_croissant_revision,
        expected_croissant_executable_sha256,
    )?;
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
    let multi_groups = exact_strings(manifest, "multi_group_ids", 2)?;
    let multi_group_creates = exact_strings(manifest, "multi_group_create_event_ids", 2)?;
    let relay_signer = required_string(manifest, "relay_signer_public_key")?;
    let simple_group = required_string(manifest, "simple_group_id")?;
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
        || multi_groups[0] == multi_groups[1]
        || multi_group_creates[0] == multi_group_creates[1]
        || multi_groups.iter().any(|group| group == simple_group)
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

#[allow(
    clippy::too_many_lines,
    reason = "one verifier binds each child's launch, provenance, limits, and teardown facts"
)]
fn verify_children(
    manifest: &Value,
    expected_croissant_revision: &str,
    expected_croissant_executable_sha256: &str,
) -> CanaryResult<()> {
    let ready = exact_objects(manifest, "ready", 2)?;
    let teardown = exact_objects(manifest, "teardown", 2)?;
    let mut pids = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut executable_inodes = BTreeSet::new();
    for index in 0..2 {
        let pid = ready[index].get("pid").and_then(Value::as_u64).unwrap_or(0);
        let endpoint = object_string(ready[index], "endpoint")?;
        let data_path = object_string(ready[index], "data_path")?;
        let executable_device = ready[index]
            .get("executable_device")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let executable_inode = ready[index]
            .get("executable_inode")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if pid == 0
            || pid == 75_649
            || !pids.insert(pid)
            || !endpoints.insert(endpoint)
            || !paths.insert(data_path)
            || executable_device == 0
            || executable_inode == 0
            || !executable_inodes.insert((executable_device, executable_inode))
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
        if object_string(ready[index], "source_head")? != expected_croissant_revision
            || object_string(ready[index], "executable_sha256")?
                != expected_croissant_executable_sha256
        {
            return Err(CanaryError::new(
                "simple-groups Croissant child provenance did not match exact expectations",
            ));
        }
        if object_string(ready[index], "execution_platform")? != "linux-sealed-memfd-proc-fd" {
            return Err(CanaryError::new(
                "simple-groups Croissant child did not use descriptor execution",
            ));
        }
        for field in [
            "executable",
            "executable_sha256",
            "source_checkout",
            "source_head",
            "scenario_seed_sha256",
            "execution_platform",
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
            || teardown[index]
                .get("executable_removed")
                .and_then(Value::as_bool)
                != Some(true)
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
        "run_id",
        "scenario_seed_sha256",
        "author_public_key",
        "simple_group_id",
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
    let first_group = required_string(&first.1, "simple_group_id")?.to_owned();
    let second_group = required_string(&second.1, "simple_group_id")?.to_owned();
    let first_run = required_string(&first.1, "run_id")?.to_owned();
    let second_run = required_string(&second.1, "run_id")?.to_owned();
    let first_identities = event_identities(&first.1)?;
    let second_identities = event_identities(&second.1)?;
    for (snapshot, foreign, label) in [
        (&first.0, second_group.as_str(), "group_id"),
        (&second.0, first_group.as_str(), "group_id"),
        (&first.0, second_run.as_str(), "run_id"),
        (&second.0, first_run.as_str(), "run_id"),
    ] {
        for path in snapshot.files() {
            if path == Path::new("manifest.json") || !snapshot.contains(path, foreign.as_bytes())? {
                continue;
            }
            return Err(CanaryError::new(format!(
                "simple-groups run retained the other run's {label} in {}",
                path.display()
            )));
        }
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

include!("croissant_simple_groups_evidence/value_support.rs");

#[cfg(test)]
mod pair_root_tests {
    use std::cell::Cell;
    use std::fs;

    use tempfile::TempDir;

    use super::run_roots_with;

    #[test]
    fn pair_root_refuses_on_third_entry_without_enumerating_the_remainder() {
        let root = TempDir::new().expect("pair root");
        for index in 0..64 {
            fs::create_dir(root.path().join(format!("run-{index:02}"))).expect("run directory");
        }
        let visited = Cell::new(0_usize);
        let error = run_roots_with(root.path(), |_| visited.set(visited.get() + 1))
            .expect_err("third entry exceeds the exact pair bound");
        assert!(error.to_string().contains("exactly two"));
        assert_eq!(visited.get(), 3, "hostile remainder was enumerated");
    }
}
