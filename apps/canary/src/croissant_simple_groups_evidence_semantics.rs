//! Semantic derivation from bounded retained simple-groups artifacts.

use std::collections::BTreeSet;
use std::path::Path;

use nostr::event::Event;
use serde_json::{Map, Value, json};

use crate::croissant_simple_groups_evidence_support::EvidenceSnapshot;
use crate::{CanaryError, CanaryResult};

const FLOW_LIMIT: u64 = 128 * 1024;
const PROCESS_LIMIT: u64 = 256 * 1024;
const WIRE_LIMIT: u64 = 2 * 1024 * 1024;

pub(super) fn verify_semantic_artifacts(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
) -> CanaryResult<()> {
    verify_flow(snapshot, manifest)?;
    verify_processes(snapshot, manifest)?;
    verify_wire(snapshot, manifest)?;
    rescan_secret_markers(snapshot)
}

fn rescan_secret_markers(snapshot: &EvidenceSnapshot) -> CanaryResult<()> {
    for needle in [
        b"nsec1".as_slice(),
        b"NSEC1".as_slice(),
        b"nostr:nsec1".as_slice(),
        b"NOSTR:NSEC1".as_slice(),
        b"\"scenario_seed\":".as_slice(),
        b"\"raw_seed\":".as_slice(),
        b"\"private_key\":".as_slice(),
        b"\"secret_key\":".as_slice(),
    ] {
        for relative in snapshot.files() {
            if relative == Path::new("source/fava-canary") {
                continue;
            }
            if snapshot.contains(relative, needle)? {
                return Err(CanaryError::new(
                    "simple-groups retained evidence contained a secret marker",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod marker_tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use tempfile::TempDir;

    use super::{EvidenceSnapshot, rescan_secret_markers};
    use crate::croissant_simple_groups_source::MAX_PINNED_FAVA_EXECUTABLE_BYTES;

    #[test]
    fn generic_markers_ignore_only_the_exact_retained_executable() {
        let fixture = TempDir::new().expect("marker fixture");
        fs::create_dir(fixture.path().join("source")).expect("source directory");
        let executable = fixture.path().join("source/fava-canary");
        let bytes = b"#!/bin/sh\n# embedded literal: \"private_key\":\nexit 0\n";
        assert!((bytes.len() as u64) < MAX_PINNED_FAVA_EXECUTABLE_BYTES);
        fs::write(&executable, bytes).expect("small executable fixture");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
            .expect("executable mode");
        assert!(
            Command::new(&executable)
                .status()
                .expect("execute fixture")
                .success()
        );
        let snapshot = EvidenceSnapshot::capture(fixture.path()).expect("binary snapshot");
        rescan_secret_markers(&snapshot).expect("embedded verifier literals are not secrets");

        fs::write(fixture.path().join("retained-copy"), bytes).expect("hostile marker copy");
        let snapshot = EvidenceSnapshot::capture(fixture.path()).expect("hostile snapshot");
        assert!(rescan_secret_markers(&snapshot).is_err());
    }
}

fn verify_flow(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let flow: Value =
        serde_json::from_slice(snapshot.read(Path::new("flow.json"), FLOW_LIMIT, "flow")?)?;
    for field in [
        "simple_group_id",
        "relay_urls",
        "shared_event_id",
        "unique_event_ids",
        "shared_evidence",
        "metadata_names",
        "metadata_authors",
        "admin_targets",
        "admin_authors",
        "multi_group_ids",
        "multi_group_create_event_ids",
        "custom_event_id",
        "custom_event_signature",
        "custom_destinations",
        "custom_acknowledged",
        "handoffs",
        "prepared_contexts",
        "observation_closed",
    ] {
        if flow.get(field) != manifest.get(field) {
            return Err(CanaryError::new(format!(
                "simple-groups {field} claim was not derived from flow.json"
            )));
        }
    }
    let run_id = string(manifest, "run_id")?;
    for field in ["write_id", "receipt_id"] {
        let value = flow.get(field).and_then(Value::as_u64).ok_or_else(|| {
            CanaryError::new(format!("simple-groups flow omitted numeric {field}"))
        })?;
        if string(manifest, field)? != format!("{run_id}:{value}") {
            return Err(CanaryError::new(format!(
                "simple-groups {field} claim was not derived from flow.json"
            )));
        }
    }
    Ok(())
}

fn verify_processes(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let processes: Value = serde_json::from_slice(snapshot.read(
        Path::new("children/processes.json"),
        PROCESS_LIMIT,
        "process evidence",
    )?)?;
    for field in ["ready", "teardown"] {
        if processes.get(field) != manifest.get(field) {
            return Err(CanaryError::new(format!(
                "simple-groups {field} claim was not derived from process evidence"
            )));
        }
    }
    let bounds = manifest
        .get("bounds")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("simple-groups manifest omitted bounds"))?;
    let log_limit = bounds.get("log_bytes").and_then(Value::as_u64).unwrap_or(0);
    let teardown = processes
        .get("teardown")
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new("simple-groups process evidence omitted teardown"))?;
    for (index, label) in ["a", "b"].iter().enumerate() {
        for (field, suffix) in [
            ("stdout_bytes", "stdout.log"),
            ("stderr_bytes", "stderr.log"),
        ] {
            let claimed = teardown[index]
                .get(field)
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX);
            let actual = snapshot
                .read(
                    Path::new(&format!("relays/{label}/{suffix}")),
                    log_limit,
                    "child log",
                )?
                .len();
            if claimed != u64::try_from(actual).unwrap_or(u64::MAX) {
                return Err(CanaryError::new(
                    "simple-groups child log bytes disagreed with teardown evidence",
                ));
            }
        }
    }
    Ok(())
}

fn verify_wire(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let relay_urls = strings(manifest, "relay_urls", 2)?;
    let shared_evidence = strings(manifest, "shared_evidence", 2)?;
    if relay_urls != shared_evidence {
        return Err(CanaryError::new(
            "simple-groups shared evidence was not bound to exact relay routes",
        ));
    }
    let mut observed = 0_u64;
    for (index, label) in ["a", "b"].iter().enumerate() {
        observed = observed
            .checked_add(verify_one_wire(snapshot, manifest, index, label)?)
            .ok_or_else(|| CanaryError::new("simple-groups wire byte count overflow"))?;
    }
    if manifest
        .get("bounds")
        .and_then(|bounds| bounds.get("wire_bytes_observed"))
        .and_then(Value::as_u64)
        != Some(observed)
    {
        return Err(CanaryError::new(
            "simple-groups wire byte claim disagreed with retained logs",
        ));
    }
    Ok(())
}

include!("croissant_simple_groups_evidence_semantics/wire_state.rs");
include!("croissant_simple_groups_evidence_semantics/value_support.rs");
