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
            if snapshot.contains(relative, needle)? {
                return Err(CanaryError::new(
                    "simple-groups retained evidence contained a secret marker",
                ));
            }
        }
    }
    Ok(())
}

fn verify_flow(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let flow: Value =
        serde_json::from_slice(snapshot.read(Path::new("flow.json"), FLOW_LIMIT, "flow")?)?;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryKind {
    Content,
    Records,
    Auxiliary,
}

enum ConnectionState {
    Publish {
        event_id: String,
        acknowledged: bool,
    },
    Query {
        subscription: String,
        kind: QueryKind,
        eose: bool,
        closed: bool,
    },
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass derives one relay's complete wire proof"
)]
fn verify_one_wire(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    index: usize,
    label: &str,
) -> CanaryResult<u64> {
    let (frames, wire_bytes) = wire_frames(snapshot, label)?;
    let group = string(manifest, "group_id")?;
    let shared = string(manifest, "shared_event_id")?;
    let unique = strings(manifest, "unique_event_ids", 2)?;
    let custom = string(manifest, "custom_event_id")?;
    let metadata_names = strings(manifest, "metadata_names", 2)?;
    let admin_targets = strings(manifest, "admin_targets", 2)?;
    let relay_signer = string(manifest, "relay_signer_public_key")?;
    let author = string(manifest, "author_public_key")?;
    let mut connections = std::collections::BTreeMap::<u64, ConnectionState>::new();
    let mut content_subscription = None;
    let mut records_subscription = None;
    let mut content_events = BTreeSet::new();
    let mut saw_metadata = false;
    let mut saw_admin = false;
    let mut custom_handoffs = 0_u64;
    let mut custom_acknowledged = 0_u64;
    let mut client_events = 0_u64;

    let mut expected_sequence = 1_u64;
    for frame in &frames {
        if frame.get("sequence").and_then(Value::as_u64) != Some(expected_sequence) {
            return Err(CanaryError::new(
                "simple-groups wire sequence was not strict and contiguous",
            ));
        }
        expected_sequence = expected_sequence.saturating_add(1);
        let connection = frame
            .get("connection")
            .and_then(Value::as_u64)
            .filter(|value| *value != 0)
            .ok_or_else(|| CanaryError::new("simple-groups wire omitted connection identity"))?;
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
                if connections.contains_key(&connection) {
                    return Err(CanaryError::new(
                        "simple-groups wire reused a connection for a second exchange",
                    ));
                }
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let filter = payload.get(2).and_then(Value::as_object);
                let query_kind =
                    if filter.is_some_and(|filter| exact_filter(filter, "#h", group, &[9], 16)) {
                        assign_once(&mut content_subscription, subscription, "content REQ")?;
                        QueryKind::Content
                    } else if filter.is_some_and(|filter| {
                        exact_filter(
                            filter,
                            "#d",
                            group,
                            &[39000, 39001, 39002, 39003, 39004, 39005],
                            4096,
                        )
                    }) {
                        assign_once(&mut records_subscription, subscription, "records REQ")?;
                        QueryKind::Records
                    } else {
                        QueryKind::Auxiliary
                    };
                if subscription.is_empty() {
                    return Err(CanaryError::new("simple-groups REQ omitted subscription"));
                }
                connections.insert(
                    connection,
                    ConnectionState::Query {
                        subscription: subscription.to_owned(),
                        kind: query_kind,
                        eose: false,
                        closed: false,
                    },
                );
            }
            ("client_to_relay", "EVENT") => {
                if connections.contains_key(&connection) {
                    return Err(CanaryError::new(
                        "simple-groups wire reused a connection for a second exchange",
                    ));
                }
                client_events += 1;
                let event = event_at(payload, 1)?;
                event.verify().map_err(error)?;
                if event.pubkey.to_hex() != author || !has_exact_tag(&event, "h", group) {
                    return Err(CanaryError::new(
                        "simple-groups client EVENT escaped its author or h authority",
                    ));
                }
                if event.id.to_hex() == custom {
                    custom_handoffs += 1;
                }
                connections.insert(
                    connection,
                    ConnectionState::Publish {
                        event_id: event.id.to_hex(),
                        acknowledged: false,
                    },
                );
            }
            ("relay_to_client", "OK") => {
                let acknowledged = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let accepted = payload.get(2).and_then(Value::as_bool) == Some(true);
                let Some(ConnectionState::Publish {
                    event_id,
                    acknowledged: was_acknowledged,
                }) = connections.get_mut(&connection)
                else {
                    return Err(CanaryError::new(
                        "simple-groups OK was not on its EVENT connection",
                    ));
                };
                if !accepted || *was_acknowledged || acknowledged != event_id {
                    return Err(CanaryError::new(
                        "simple-groups OK did not causally acknowledge its EVENT",
                    ));
                }
                *was_acknowledged = true;
                if acknowledged == custom {
                    custom_acknowledged += 1;
                }
            }
            ("relay_to_client", "EVENT") => {
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let event = event_at(payload, 2)?;
                event.verify().map_err(error)?;
                let Some(ConnectionState::Query {
                    subscription: expected,
                    kind,
                    closed,
                    ..
                }) = connections.get(&connection)
                else {
                    return Err(CanaryError::new(
                        "simple-groups response EVENT preceded its REQ",
                    ));
                };
                if subscription != expected || *closed {
                    return Err(CanaryError::new(
                        "simple-groups response EVENT escaped its open REQ",
                    ));
                }
                if *kind == QueryKind::Content {
                    if event.pubkey.to_hex() != author {
                        return Err(CanaryError::new(
                            "simple-groups content result escaped author authority",
                        ));
                    }
                    if !has_exact_tag(&event, "h", group) || event.kind.as_u16() != 9 {
                        return Err(CanaryError::new(
                            "simple-groups content result escaped its exact group query",
                        ));
                    }
                    content_events.insert(event.id.to_hex());
                }
                if *kind == QueryKind::Records {
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
            ("relay_to_client", "EOSE") => {
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let Some(ConnectionState::Query {
                    subscription: expected,
                    eose,
                    closed,
                    ..
                }) = connections.get_mut(&connection)
                else {
                    return Err(CanaryError::new("simple-groups EOSE preceded its REQ"));
                };
                if subscription != expected || *eose || *closed {
                    return Err(CanaryError::new("simple-groups EOSE was not causal"));
                }
                *eose = true;
            }
            ("client_to_relay", "CLOSE") => {
                let subscription = payload.get(1).and_then(Value::as_str).unwrap_or_default();
                let Some(ConnectionState::Query {
                    subscription: expected,
                    eose,
                    closed,
                    ..
                }) = connections.get_mut(&connection)
                else {
                    return Err(CanaryError::new("simple-groups CLOSE preceded its REQ"));
                };
                if subscription != expected || !*eose || *closed {
                    return Err(CanaryError::new("simple-groups CLOSE was not causal"));
                }
                *closed = true;
            }
            _ => {}
        }
    }
    content_subscription
        .ok_or_else(|| CanaryError::new("simple-groups wire omitted exact content REQ"))?;
    records_subscription
        .ok_or_else(|| CanaryError::new("simple-groups wire omitted exact records REQ"))?;
    let expected = BTreeSet::from([shared.to_owned(), unique[index].clone()]);
    if content_events != expected
        || !saw_metadata
        || !saw_admin
        || custom_handoffs != 1
        || custom_acknowledged != 1
        || client_events != 6
        || connections.values().any(|state| match state {
            ConnectionState::Publish { acknowledged, .. } => !acknowledged,
            ConnectionState::Query { eose, closed, .. } => !eose || !closed,
        })
    {
        return Err(CanaryError::new(
            "simple-groups wire did not derive the complete public flow",
        ));
    }
    Ok(wire_bytes)
}

include!("croissant_simple_groups_evidence_semantics/value_support.rs");
