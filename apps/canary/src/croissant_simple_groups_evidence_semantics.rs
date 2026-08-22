//! Semantic derivation from bounded retained simple-groups artifacts.

use std::collections::BTreeSet;
use std::path::Path;

use nostr::event::Event;
use serde_json::{Map, Value, json};

use crate::croissant_simple_groups_evidence_support::{
    collect_files, read_bounded, stream_contains,
};
use crate::{CanaryError, CanaryResult};

const FLOW_LIMIT: u64 = 128 * 1024;
const PROCESS_LIMIT: u64 = 256 * 1024;
const WIRE_LIMIT: u64 = 2 * 1024 * 1024;

pub(super) fn verify_semantic_artifacts(root: &Path, manifest: &Value) -> CanaryResult<()> {
    verify_flow(root, manifest)?;
    verify_processes(root, manifest)?;
    verify_wire(root, manifest)?;
    rescan_secret_markers(root)
}

fn rescan_secret_markers(root: &Path) -> CanaryResult<()> {
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
        for relative in collect_files(root)? {
            if stream_contains(root, &relative, needle)? {
                return Err(CanaryError::new(
                    "simple-groups retained evidence contained a secret marker",
                ));
            }
        }
    }
    Ok(())
}

fn verify_flow(root: &Path, manifest: &Value) -> CanaryResult<()> {
    let flow: Value = serde_json::from_slice(&read_bounded(
        root,
        Path::new("flow.json"),
        FLOW_LIMIT,
        "flow",
    )?)?;
    for field in [
        "group_id",
        "relay_urls",
        "shared_event_id",
        "unique_event_ids",
        "shared_evidence",
        "metadata_names",
        "metadata_authors",
        "admin_targets",
        "admin_authors",
        "custom_event_id",
        "custom_destinations",
        "custom_acknowledged",
        "handoffs",
        "signed_refusals",
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

fn verify_processes(root: &Path, manifest: &Value) -> CanaryResult<()> {
    let processes: Value = serde_json::from_slice(&read_bounded(
        root,
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
    Ok(())
}

fn verify_wire(root: &Path, manifest: &Value) -> CanaryResult<()> {
    let relay_urls = strings(manifest, "relay_urls", 2)?;
    let shared_evidence = strings(manifest, "shared_evidence", 2)?;
    if relay_urls.iter().collect::<BTreeSet<_>>() != shared_evidence.iter().collect::<BTreeSet<_>>()
    {
        return Err(CanaryError::new(
            "simple-groups shared evidence did not cover both exact hosts",
        ));
    }
    for (index, label) in ["a", "b"].iter().enumerate() {
        verify_one_wire(root, manifest, index, label)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass derives one relay's complete wire proof"
)]
fn verify_one_wire(root: &Path, manifest: &Value, index: usize, label: &str) -> CanaryResult<()> {
    let frames = wire_frames(root, label)?;
    let group = string(manifest, "group_id")?;
    let shared = string(manifest, "shared_event_id")?;
    let unique = strings(manifest, "unique_event_ids", 2)?;
    let custom = string(manifest, "custom_event_id")?;
    let metadata_names = strings(manifest, "metadata_names", 2)?;
    let admin_targets = strings(manifest, "admin_targets", 2)?;
    let relay_signer = string(manifest, "relay_signer_public_key")?;
    let mut content_subscription = None;
    let mut records_subscription = None;
    let mut content_events = BTreeSet::new();
    let mut saw_metadata = false;
    let mut saw_admin = false;
    let mut custom_handoffs = 0_u64;
    let mut custom_acknowledged = 0_u64;
    let mut client_events = 0_u64;

    for frame in &frames {
        let direction = frame
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(payload) = frame.get("decoded") else {
            continue;
        };
        let Some(kind) = payload.get(0).and_then(Value::as_str) else {
            continue;
        };
        match (direction, kind) {
            ("client_to_relay", "REQ") => {
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let filter = payload.get(2).and_then(Value::as_object);
                if filter.is_some_and(|filter| exact_filter(filter, "#h", group, &[9], 16)) {
                    assign_once(&mut content_subscription, subscription, "content REQ")?;
                }
                if filter.is_some_and(|filter| {
                    exact_filter(
                        filter,
                        "#d",
                        group,
                        &[39000, 39001, 39002, 39003, 39004, 39005],
                        4096,
                    )
                }) {
                    assign_once(&mut records_subscription, subscription, "records REQ")?;
                }
            }
            ("client_to_relay", "EVENT") => {
                client_events += 1;
                let event = event_at(payload, 1)?;
                event.verify().map_err(error)?;
                if !has_exact_tag(&event, "h", group) {
                    return Err(CanaryError::new(
                        "simple-groups client EVENT omitted its exact h authority",
                    ));
                }
                if event.id.to_hex() == custom {
                    custom_handoffs += 1;
                }
            }
            ("relay_to_client", "OK")
                if payload.get(1).and_then(Value::as_str) == Some(custom)
                    && payload.get(2).and_then(Value::as_bool) == Some(true) =>
            {
                custom_acknowledged += 1;
            }
            ("relay_to_client", "EVENT") => {
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let event = event_at(payload, 2)?;
                event.verify().map_err(error)?;
                if Some(subscription) == content_subscription.as_deref() {
                    if !has_exact_tag(&event, "h", group) || event.kind.as_u16() != 9 {
                        return Err(CanaryError::new(
                            "simple-groups content result escaped its exact group query",
                        ));
                    }
                    content_events.insert(event.id.to_hex());
                }
                if Some(subscription) == records_subscription.as_deref() {
                    if !has_exact_tag(&event, "d", group) {
                        return Err(CanaryError::new(
                            "simple-groups record result escaped its exact group query",
                        ));
                    }
                    if event.kind.as_u16() == 39000
                        && event.pubkey.to_hex() == relay_signer
                        && has_exact_tag(&event, "name", &metadata_names[index])
                    {
                        saw_metadata = true;
                    }
                    if event.kind.as_u16() == 39001
                        && event.pubkey.to_hex() == relay_signer
                        && has_tag_value(&event, "p", &admin_targets[index])
                    {
                        saw_admin = true;
                    }
                }
            }
            _ => {}
        }
    }
    let content_subscription = content_subscription
        .ok_or_else(|| CanaryError::new("simple-groups wire omitted exact content REQ"))?;
    let records_subscription = records_subscription
        .ok_or_else(|| CanaryError::new("simple-groups wire omitted exact records REQ"))?;
    let expected = BTreeSet::from([shared.to_owned(), unique[index].clone()]);
    if content_events != expected
        || !saw_metadata
        || !saw_admin
        || custom_handoffs != 1
        || custom_acknowledged != 1
        || client_events != 6
        || count_close(&frames, &content_subscription) != 1
        || count_close(&frames, &records_subscription) != 1
    {
        return Err(CanaryError::new(
            "simple-groups wire did not derive the complete public flow",
        ));
    }
    Ok(())
}

fn wire_frames(root: &Path, label: &str) -> CanaryResult<Vec<Value>> {
    let relative = format!("wire/{label}.jsonl");
    let bytes = read_bounded(root, Path::new(&relative), WIRE_LIMIT, "wire log")?;
    if !bytes.ends_with(b"\n") {
        return Err(CanaryError::new("simple-groups wire log was incomplete"));
    }
    let mut frames = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut frame: Value = serde_json::from_slice(line)?;
        if frame.get("frame_type").and_then(Value::as_str) == Some("text") {
            let payload = frame
                .get("payload")
                .and_then(Value::as_str)
                .ok_or_else(|| CanaryError::new("simple-groups text frame omitted payload"))?;
            let decoded: Value = serde_json::from_str(payload.trim_end())?;
            frame
                .as_object_mut()
                .ok_or_else(|| CanaryError::new("simple-groups wire frame was not an object"))?
                .insert("decoded".to_owned(), decoded);
        }
        frames.push(frame);
    }
    if frames.is_empty() {
        return Err(CanaryError::new("simple-groups wire log was empty"));
    }
    Ok(frames)
}

fn exact_filter(
    filter: &Map<String, Value>,
    axis: &str,
    group: &str,
    kinds: &[u64],
    limit: u64,
) -> bool {
    filter.len() == 3
        && filter.get(axis) == Some(&json!([group]))
        && filter.get("kinds") == Some(&json!(kinds))
        && filter.get("limit").and_then(Value::as_u64) == Some(limit)
}

fn assign_once(slot: &mut Option<String>, value: &str, label: &str) -> CanaryResult<()> {
    if value.is_empty() || slot.replace(value.to_owned()).is_some() {
        return Err(CanaryError::new(format!(
            "simple-groups wire repeated {label}"
        )));
    }
    Ok(())
}

fn event_at(payload: &Value, index: usize) -> CanaryResult<Event> {
    serde_json::from_value(
        payload
            .get(index)
            .cloned()
            .ok_or_else(|| CanaryError::new("simple-groups EVENT omitted event body"))?,
    )
    .map_err(Into::into)
}

fn has_exact_tag(event: &Event, name: &str, value: &str) -> bool {
    let matches = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
        .collect::<Vec<_>>();
    matches.len() == 1 && matches[0].as_slice().get(1).map(String::as_str) == Some(value)
}

fn has_tag_value(event: &Event, name: &str, value: &str) -> bool {
    event.tags.iter().any(|tag| {
        tag.as_slice().first().map(String::as_str) == Some(name)
            && tag.as_slice().get(1).map(String::as_str) == Some(value)
    })
}

fn count_close(frames: &[Value], subscription: &str) -> usize {
    frames
        .iter()
        .filter(|frame| {
            frame.get("direction").and_then(Value::as_str) == Some("client_to_relay")
                && frame
                    .get("decoded")
                    .and_then(|value| value.get(0))
                    .and_then(Value::as_str)
                    == Some("CLOSE")
                && frame
                    .get("decoded")
                    .and_then(|value| value.get(1))
                    .and_then(Value::as_str)
                    == Some(subscription)
        })
        .count()
}

fn string<'a>(value: &'a Value, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("simple-groups evidence omitted {field}")))
}

fn strings(value: &Value, field: &str, count: usize) -> CanaryResult<Vec<String>> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new(format!("simple-groups evidence omitted {field}")))?;
    if values.len() != count {
        return Err(CanaryError::new(format!(
            "simple-groups evidence required exactly {count} {field}"
        )));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CanaryError::new(format!("simple-groups {field} was not text")))
        })
        .collect()
}

fn error(error: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(error.to_string())
}
