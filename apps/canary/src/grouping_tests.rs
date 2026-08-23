use super::*;

fn fixture() -> (Filter, Vec<RelayDemand>) {
    let key = literal_key('d').expect("tag key");
    let values = (0..LOGICAL_QUERY_COUNT)
        .map(|index| format!("value-{index:03}"))
        .collect::<Vec<_>>();
    let grouped = Filter::new().custom_tags(key, values.iter().cloned());
    let demand = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            demand_for_query(
                observation(index),
                QueryBranchId::ROOT,
                &Query::events().tag_values(key, [value.clone()]),
            )
        })
        .collect();
    (grouped, demand)
}

fn write_wire(grouped_payload: &Value, separate_payloads: Vec<Value>) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("wire.jsonl");
    let mut lines = Vec::with_capacity(LOGICAL_QUERY_COUNT + 1);
    lines.push(json!({
        "direction": "client_to_relay",
        "frame_type": "text",
        "connection": 1,
        "payload": serde_json::to_string(&json!(["REQ", "grouped", grouped_payload]))
            .expect("payload serializes"),
    }));
    for (index, filter) in separate_payloads.into_iter().enumerate() {
        lines.push(json!({
            "direction": "client_to_relay",
            "frame_type": "text",
            "connection": 2,
            "payload": serde_json::to_string(&json!(["REQ", format!("separate-{index}"), filter]))
                .expect("payload serializes"),
        }));
    }
    lines.push(json!({
        "direction": "client_to_relay",
        "frame_type": "close",
        "connection": 1,
        "payload": "None",
    }));
    lines.push(json!({
        "direction": "client_to_relay",
        "frame_type": "binary",
        "connection": 2,
        "payload": "non-json binary frame",
    }));
    let contents = lines
        .into_iter()
        .map(|line| serde_json::to_string(&line).expect("wire entry serializes"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, contents).expect("wire log writes");
    directory
}

fn execution() -> PlanExecution {
    PlanExecution {
        request_count: LOGICAL_QUERY_COUNT,
        mode: "test",
        concurrent_attempt_requests: 0,
        capacity_refusal: None,
    }
}

#[test]
fn wire_witness_accepts_exact_grouped_and_separate_filters() {
    let (grouped, demand) = fixture();
    let grouped_payload = serde_json::to_value(&grouped).expect("grouped filter serializes");
    let separate_payloads = demand
        .iter()
        .map(|item| serde_json::to_value(&item.filter).expect("filter serializes"))
        .collect();
    let directory = write_wire(&grouped_payload, separate_payloads);

    verify_wire(
        &directory.path().join("wire.jsonl"),
        &grouped,
        &demand,
        &execution(),
    )
    .expect("exact wire witness passes");
}

#[test]
fn wire_witness_rejects_correct_req_counts_with_overbroad_grouped_filter() {
    let (grouped, demand) = fixture();
    let separate_payloads = demand
        .iter()
        .map(|item| serde_json::to_value(&item.filter).expect("filter serializes"))
        .collect();
    let directory = write_wire(&json!({}), separate_payloads);

    assert!(
        verify_wire(
            &directory.path().join("wire.jsonl"),
            &grouped,
            &demand,
            &execution(),
        )
        .is_err()
    );
}

#[test]
fn wire_witness_rejects_correct_req_counts_with_wrong_separate_filter() {
    let (grouped, demand) = fixture();
    let grouped_payload = serde_json::to_value(&grouped).expect("grouped filter serializes");
    let mut separate_payloads = demand
        .iter()
        .map(|item| serde_json::to_value(&item.filter).expect("filter serializes"))
        .collect::<Vec<_>>();
    separate_payloads[0] = json!({});
    let directory = write_wire(&grouped_payload, separate_payloads);
    let separate = PlanExecution {
        request_count: LOGICAL_QUERY_COUNT,
        mode: "test",
        concurrent_attempt_requests: 0,
        capacity_refusal: None,
    };

    assert!(
        verify_wire(
            &directory.path().join("wire.jsonl"),
            &grouped,
            &demand,
            &separate,
        )
        .is_err()
    );
}
