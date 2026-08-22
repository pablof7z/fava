#[test]
fn pair_verifier_rejects_noncausal_wire_mutations() {
    let after_close = PairEvidenceFixture::new();
    after_close.rewrite_wire_values(0, "a", |frames| {
        let event = frames
            .iter()
            .position(|frame| {
                decoded_kind(frame) == Some("EVENT")
                    && frame.get("direction").and_then(Value::as_str) == Some("relay_to_client")
                    && decoded_subscription(frame) == Some("content-0")
            })
            .expect("content response");
        let event = frames.remove(event);
        let close = frames
            .iter()
            .position(|frame| decoded_kind(frame) == Some("CLOSE"))
            .expect("content close");
        frames.insert(close + 1, event);
    });
    assert!(
        verify_fixture_pair(after_close.root()).is_err(),
        "response EVENT after CLOSE must be refused"
    );

    let wrong_connection = PairEvidenceFixture::new();
    wrong_connection.rewrite_wire_values(0, "a", |frames| {
        let custom = frames
            .iter_mut()
            .find(|frame| {
                decoded_kind(frame) == Some("OK")
                    && frame
                        .get("decoded")
                        .and_then(|value| value.get(1))
                        .and_then(Value::as_str)
                        == Some(
                            read_manifest(&wrong_connection.roots[0])["custom_event_id"]
                                .as_str()
                                .expect("custom id"),
                        )
            })
            .expect("custom OK");
        custom["connection"] = json!(99);
    });
    assert!(
        verify_fixture_pair(wrong_connection.root()).is_err(),
        "OK on another connection must be refused"
    );
}

#[test]
fn pair_verifier_rejects_wrong_author_and_unready_children() {
    let wrong_author = PairEvidenceFixture::new();
    let attacker = Keys::generate();
    let group = read_manifest(&wrong_author.roots[0])["group_id"]
        .as_str()
        .expect("group")
        .to_owned();
    let event = signed_fixture_event(&attacker, 9007, &group, "attacker bootstrap");
    wrong_author.rewrite_wire_values(0, "a", |frames| {
        let frame = frames
            .iter_mut()
            .find(|frame| {
                decoded_kind(frame) == Some("EVENT")
                    && frame.get("direction").and_then(Value::as_str) == Some("client_to_relay")
            })
            .expect("client event");
        frame["decoded"][1] = serde_json::to_value(&event).expect("event json");
        frame["payload"] = json!(serde_json::to_string(&frame["decoded"]).expect("payload"));
    });
    assert!(
        verify_fixture_pair(wrong_author.root()).is_err(),
        "valid signature from the wrong author must be refused"
    );

    for field in ["readiness_completed", "limits"] {
        let fixture = PairEvidenceFixture::new();
        let path = fixture.roots[0].join("children/processes.json");
        let mut processes: Value =
            serde_json::from_slice(&fs::read(&path).expect("processes")).expect("process JSON");
        if field == "readiness_completed" {
            processes["ready"][0][field] = json!(false);
        } else {
            processes["ready"][0][field]["log_bytes"] = json!(1);
        }
        fs::write(
            &path,
            serde_json::to_vec_pretty(&processes).expect("process bytes"),
        )
        .expect("process mutation");
        fixture.mutate(0, true, |manifest| {
            manifest["ready"] = processes["ready"].clone();
        });
        assert!(
            verify_fixture_pair(fixture.root()).is_err(),
            "hostile child {field} claim must be refused"
        );
    }
}

#[test]
fn pair_verifier_rejects_duplicate_routes_and_wrong_revision() {
    let duplicate = PairEvidenceFixture::new();
    let first = read_manifest(&duplicate.roots[0])["relay_urls"][0].clone();
    let flow_path = duplicate.roots[0].join("flow.json");
    let mut flow: Value =
        serde_json::from_slice(&fs::read(&flow_path).expect("flow")).expect("flow JSON");
    flow["relay_urls"] = json!([first.clone(), first.clone()]);
    flow["shared_evidence"] = json!([first.clone(), first.clone()]);
    fs::write(
        &flow_path,
        serde_json::to_vec_pretty(&flow).expect("flow bytes"),
    )
    .expect("flow mutation");
    duplicate.mutate(0, true, |manifest| {
        manifest["relay_urls"] = flow["relay_urls"].clone();
        manifest["shared_evidence"] = flow["shared_evidence"].clone();
    });
    assert!(
        verify_fixture_pair(duplicate.root()).is_err(),
        "two entries for one route must not prove two relays"
    );

    let revision = PairEvidenceFixture::new();
    assert!(
        verify_croissant_simple_groups_pair(
            revision.root(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            FIXTURE_FAVA_TREE,
            FIXTURE_FAVA_BUILD_TREE,
            FIXTURE_FAVA_BUILD_IMAGE,
            FIXTURE_FAVA_SOURCE_MANIFEST,
            FIXTURE_FAVA_RUST_BASE_IMAGE,
            FIXTURE_FAVA_EXECUTABLE,
            FIXTURE_FAVA_SUBJECT_IMAGE,
            FIXTURE_CROISSANT_REVISION,
            FIXTURE_CROISSANT_EXECUTABLE,
        )
        .is_err(),
        "both manifests must match the explicitly expected revision"
    );
}

fn decoded_kind(frame: &Value) -> Option<&str> {
    frame
        .get("decoded")
        .and_then(|payload| payload.get(0))
        .and_then(Value::as_str)
}

fn decoded_subscription(frame: &Value) -> Option<&str> {
    frame
        .get("decoded")
        .and_then(|payload| payload.get(1))
        .and_then(Value::as_str)
}

impl PairEvidenceFixture {
    fn rewrite_wire_values(&self, index: usize, label: &str, update: impl FnOnce(&mut Vec<Value>)) {
        self.rewrite_wire(index, label, |wire| {
            let mut frames = wire
                .lines()
                .map(|line| {
                    let mut frame: Value = serde_json::from_str(line).expect("wire line");
                    if frame.get("frame_type").and_then(Value::as_str) == Some("text") {
                        frame["decoded"] =
                            serde_json::from_str(frame["payload"].as_str().expect("wire payload"))
                                .expect("decoded payload");
                    }
                    frame
                })
                .collect::<Vec<_>>();
            update(&mut frames);
            frames
                .iter_mut()
                .enumerate()
                .map(|(index, frame)| {
                    frame["sequence"] = json!(index + 1);
                    frame
                        .as_object_mut()
                        .expect("frame object")
                        .remove("decoded");
                    serde_json::to_string(frame).expect("wire JSON")
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        });
    }
}
