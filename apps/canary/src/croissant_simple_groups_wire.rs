//! Exact terminal wire witness for the controlled simple-groups scenario.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use fava_write::EventId;

use crate::{CanaryError, CanaryResult};

const COMPLETION_DEADLINE: Duration = Duration::from_secs(5);
const EXPECTED_QUERY_COUNT: usize = 3;
const MAXIMUM_WIRE_BYTES: u64 = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum QueryRole {
    Bootstrap,
    Content,
    Records,
}

#[derive(Debug)]
struct QueryState {
    connection: u64,
    subscription: String,
    saw_eose: bool,
    saw_text_close: bool,
    saw_socket_close: bool,
}

pub(crate) async fn wait_for_query_completion(
    paths: &[PathBuf; 2],
    group_id: &str,
    bootstrap_event_id: &str,
) -> CanaryResult<()> {
    tokio::time::timeout(COMPLETION_DEADLINE, async {
        loop {
            let wire_a = read_wire(&paths[0])?;
            let wire_b = read_wire(&paths[1])?;
            if pair_query_completion([&wire_a, &wire_b], group_id, bootstrap_event_id)? {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("exact query wire completion deadline elapsed"))?
}

pub(crate) fn verify_query_completion(
    paths: &[PathBuf; 2],
    group_id: &str,
    bootstrap_event_id: &str,
) -> CanaryResult<()> {
    let wire_a = read_wire(&paths[0])?;
    let wire_b = read_wire(&paths[1])?;
    if !pair_query_completion([&wire_a, &wire_b], group_id, bootstrap_event_id)? {
        return Err(CanaryError::new(
            "both final wire logs must carry exact terminal query completion",
        ));
    }
    Ok(())
}

pub(crate) fn exact_event_handoffs(path: &Path, id: EventId) -> CanaryResult<usize> {
    let id = id.to_hex();
    let wire = read_wire(path)?;
    wire.split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| serde_json::from_str::<Value>(line.trim_end()).map_err(CanaryError::from))
        .collect::<CanaryResult<Vec<_>>>()
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry.get("direction").and_then(Value::as_str) == Some("client_to_relay")
                        && entry
                            .get("payload")
                            .and_then(Value::as_str)
                            .is_some_and(|payload| {
                                payload.contains("\"EVENT\"") && payload.contains(&id)
                            })
                })
                .count()
        })
}

fn read_wire(path: &Path) -> CanaryResult<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAXIMUM_WIRE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(|_| CanaryError::new("wire length overflow"))?
        > MAXIMUM_WIRE_BYTES
    {
        return Err(CanaryError::new("wire log exceeded its byte bound"));
    }
    String::from_utf8(bytes).map_err(CanaryError::from)
}

fn pair_query_completion(
    wires: [&str; 2],
    group_id: &str,
    bootstrap_event_id: &str,
) -> CanaryResult<bool> {
    let mut complete = true;
    for (relay_index, wire) in wires.into_iter().enumerate() {
        complete &= query_completion(
            wire,
            group_id,
            bootstrap_event_id,
            &format!("group-create-{relay_index}"),
        )?;
    }
    Ok(complete)
}

fn query_completion(
    wire: &str,
    group_id: &str,
    bootstrap_event_id: &str,
    bootstrap_subscription: &str,
) -> CanaryResult<bool> {
    let mut frames = wire
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .map(|line| serde_json::from_str::<Value>(line.trim_end()).map_err(CanaryError::from))
        .collect::<CanaryResult<Vec<_>>>()?;
    frames.sort_by_key(|frame| frame.get("sequence").and_then(Value::as_u64));
    let mut sequences = BTreeSet::new();
    let mut queries = BTreeMap::<QueryRole, QueryState>::new();

    for frame in frames {
        let sequence = required_u64(&frame, "sequence")?;
        if !sequences.insert(sequence) {
            return Err(CanaryError::new("wire log repeated a sequence"));
        }
        let connection = required_u64(&frame, "connection")?;
        let direction = required_str(&frame, "direction")?;
        let frame_type = required_str(&frame, "frame_type")?;
        if frame_type == "close" && direction == "client_to_relay" {
            if let Some(state) = queries
                .values_mut()
                .find(|state| state.connection == connection)
            {
                if !state.saw_text_close || state.saw_socket_close {
                    return Err(CanaryError::new(
                        "query socket CLOSE was duplicate or preceded text CLOSE",
                    ));
                }
                state.saw_socket_close = true;
            }
            continue;
        }
        if frame_type != "text" {
            continue;
        }
        let payload = required_str(&frame, "payload")?;
        let decoded: Value = serde_json::from_str(payload)?;
        let Some(array) = decoded.as_array() else {
            return Err(CanaryError::new("wire text frame was not an array"));
        };
        match array.first().and_then(Value::as_str) {
            Some("REQ") => {
                let (role, subscription) =
                    classify_request(array, group_id, bootstrap_event_id, bootstrap_subscription)?;
                if queries.values().any(|state| {
                    state.connection == connection || state.subscription == subscription
                }) || queries
                    .insert(
                        role,
                        QueryState {
                            connection,
                            subscription,
                            saw_eose: false,
                            saw_text_close: false,
                            saw_socket_close: false,
                        },
                    )
                    .is_some()
                {
                    return Err(CanaryError::new("wire repeated an exact query role"));
                }
            }
            Some("EOSE") => {
                let subscription = exact_terminal_subscription(array, "EOSE")?;
                let state = exact_query_mut(&mut queries, connection, subscription)?;
                if state.saw_eose || state.saw_text_close || state.saw_socket_close {
                    return Err(CanaryError::new("wire EOSE was duplicate or out of order"));
                }
                state.saw_eose = true;
            }
            Some("CLOSE") => {
                let subscription = exact_terminal_subscription(array, "CLOSE")?;
                let state = exact_query_mut(&mut queries, connection, subscription)?;
                if !state.saw_eose || state.saw_text_close || state.saw_socket_close {
                    return Err(CanaryError::new(
                        "wire CLOSE was duplicate or preceded EOSE",
                    ));
                }
                state.saw_text_close = true;
            }
            _ => {}
        }
    }

    Ok(queries.len() == EXPECTED_QUERY_COUNT
        && queries
            .values()
            .all(|state| state.saw_eose && state.saw_text_close && state.saw_socket_close))
}

fn classify_request(
    array: &[Value],
    group_id: &str,
    bootstrap_event_id: &str,
    bootstrap_subscription: &str,
) -> CanaryResult<(QueryRole, String)> {
    if array.len() != 3 {
        return Err(CanaryError::new("wire REQ did not have exact shape"));
    }
    let subscription = array[1]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CanaryError::new("wire REQ omitted its subscription"))?
        .to_owned();
    let filter = &array[2];
    let role = if filter == &json!({"ids": [bootstrap_event_id]})
        && subscription == bootstrap_subscription
    {
        QueryRole::Bootstrap
    } else if filter == &json!({"kinds": [9], "limit": 16, "#h": [group_id]}) {
        QueryRole::Content
    } else if filter
        == &json!({
            "kinds": [39000, 39001, 39002, 39003, 39004, 39005],
            "limit": 4096,
            "#d": [group_id]
        })
    {
        QueryRole::Records
    } else {
        return Err(CanaryError::new("wire carried an auxiliary or altered REQ"));
    };
    Ok((role, subscription))
}

fn exact_terminal_subscription<'a>(array: &'a [Value], kind: &str) -> CanaryResult<&'a str> {
    if array.len() != 2 || array.first().and_then(Value::as_str) != Some(kind) {
        return Err(CanaryError::new(format!(
            "wire {kind} did not have exact shape"
        )));
    }
    array[1]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CanaryError::new(format!("wire {kind} omitted its subscription")))
}

fn exact_query_mut<'a>(
    queries: &'a mut BTreeMap<QueryRole, QueryState>,
    connection: u64,
    subscription: &str,
) -> CanaryResult<&'a mut QueryState> {
    queries
        .values_mut()
        .find(|state| state.connection == connection && state.subscription == subscription)
        .ok_or_else(|| CanaryError::new("wire terminal frame did not bind its exact REQ"))
}

fn required_u64(value: &Value, key: &str) -> CanaryResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| CanaryError::new(format!("wire frame omitted {key}")))
}

fn required_str<'a>(value: &'a Value, key: &str) -> CanaryResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("wire frame omitted {key}")))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{pair_query_completion, query_completion};

    const GROUP: &str = "group";
    const EVENT: &str = "event";

    fn frame(
        sequence: u64,
        connection: u64,
        direction: &str,
        kind: &str,
        payload: &Value,
    ) -> String {
        serde_json::to_string(&json!({
            "sequence": sequence,
            "connection": connection,
            "direction": direction,
            "frame_type": kind,
            "payload": if kind == "text" { serde_json::to_string(payload).unwrap() } else { "None".to_owned() },
        }))
        .unwrap()
            + "\n"
    }

    fn query(sequence: &mut u64, connection: u64, subscription: &str, filter: &Value) -> String {
        let mut wire = String::new();
        for (direction, kind, payload) in [
            (
                "client_to_relay",
                "text",
                json!(["REQ", subscription, filter]),
            ),
            ("relay_to_client", "text", json!(["EOSE", subscription])),
            ("client_to_relay", "text", json!(["CLOSE", subscription])),
            ("client_to_relay", "close", Value::Null),
        ] {
            wire += &frame(*sequence, connection, direction, kind, &payload);
            *sequence += 1;
        }
        wire
    }

    fn complete_wire() -> String {
        let mut sequence = 1;
        query(&mut sequence, 2, "bootstrap", &json!({"ids": [EVENT]}))
            + &query(
                &mut sequence,
                7,
                "content",
                &json!({"kinds": [9], "limit": 16, "#h": [GROUP]}),
            )
            + &query(
                &mut sequence,
                8,
                "records",
                &json!({"kinds": [39000, 39001, 39002, 39003, 39004, 39005], "limit": 4096, "#d": [GROUP]}),
            )
    }

    #[test]
    fn exact_three_query_completion_is_required_per_relay() {
        let complete = complete_wire();
        assert!(query_completion(&complete, GROUP, EVENT, "bootstrap").unwrap());

        let bootstrap_only = complete.lines().take(4).collect::<Vec<_>>().join("\n") + "\n";
        assert!(!query_completion(&bootstrap_only, GROUP, EVENT, "bootstrap").unwrap());

        let missing_final_socket_close =
            complete.lines().take(11).collect::<Vec<_>>().join("\n") + "\n";
        assert!(!query_completion(&missing_final_socket_close, GROUP, EVENT, "bootstrap").unwrap());
    }

    #[test]
    fn one_complete_relay_cannot_substitute_for_an_incomplete_peer() {
        let relay_a = complete_wire().replace("bootstrap", "group-create-0");
        let relay_b = complete_wire().replace("bootstrap", "group-create-1");
        let relay_b_incomplete = relay_b.lines().take(8).collect::<Vec<_>>().join("\n") + "\n";
        assert!(pair_query_completion([&relay_a, &relay_b], GROUP, EVENT).unwrap());
        assert!(!pair_query_completion([&relay_a, &relay_b_incomplete], GROUP, EVENT).unwrap());
    }

    #[test]
    fn auxiliary_malformed_and_late_duplicate_terminals_are_refused() {
        let complete = complete_wire();
        let auxiliary = complete.clone()
            + &frame(
                13,
                9,
                "client_to_relay",
                "text",
                &json!(["REQ", "other", {"kinds": [1]}]),
            );
        assert!(query_completion(&auxiliary, GROUP, EVENT, "bootstrap").is_err());

        let malformed = complete.replace(
            "[\\\"CLOSE\\\",\\\"content\\\"]",
            "[\\\"CLOSE\\\",\\\"content\\\",\\\"extra\\\"]",
        );
        assert!(query_completion(&malformed, GROUP, EVENT, "bootstrap").is_err());

        let duplicate = complete
            + &frame(
                13,
                7,
                "client_to_relay",
                "text",
                &json!(["CLOSE", "content"]),
            );
        assert!(query_completion(&duplicate, GROUP, EVENT, "bootstrap").is_err());
    }
}
