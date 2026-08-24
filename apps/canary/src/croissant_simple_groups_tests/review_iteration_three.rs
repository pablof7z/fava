fn signed_fixture_event(keys: &Keys, kind: u16, simple_group: &str, content: &str) -> Event {
    EventBuilder::new(Kind::from(kind), content)
        .tags([Tag::parse(["h", simple_group]).expect("h tag")])
        .custom_created_at(Timestamp::from(u64::from(kind) + 1))
        .finalize(keys)
        .expect("signed fixture event")
}

impl PairEvidenceFixture {
    fn mutate(&self, index: usize, reseal: bool, update: impl FnOnce(&mut Value)) {
        let mut manifest = read_manifest(&self.roots[index]);
        update(&mut manifest);
        if reseal {
            manifest["bounds"]["wire_bytes_observed"] = json!(
                ["a", "b"]
                    .into_iter()
                    .map(|label| fs::metadata(
                        self.roots[index].join(format!("wire/{label}.jsonl"))
                    )
                    .expect("wire metadata")
                    .len())
                    .sum::<u64>()
            );
            manifest["artifact_sha256"] =
                serde_json::to_value(artifact_hashes(&self.roots[index]).expect("fixture hashes"))
                    .expect("hash value");
            let seal = artifact_seal(&self.authors[index], &manifest).expect("fixture reseal");
            manifest["artifact_seal"] = serde_json::to_value(seal).expect("seal value");
        }
        fs::write(
            self.roots[index].join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("manifest bytes"),
        )
        .expect("mutated manifest");
    }

    fn reverse_shared_evidence(&self, index: usize) {
        let flow_path = self.roots[index].join("flow.json");
        let mut flow: Value =
            serde_json::from_slice(&fs::read(&flow_path).expect("flow read")).expect("flow json");
        flow["shared_evidence"]
            .as_array_mut()
            .expect("shared evidence array")
            .reverse();
        fs::write(
            &flow_path,
            serde_json::to_vec_pretty(&flow).expect("flow bytes"),
        )
        .expect("flow rewrite");
        self.mutate(index, true, |manifest| {
            manifest["shared_evidence"]
                .as_array_mut()
                .expect("shared evidence array")
                .reverse();
        });
    }
}

#[test]
fn pair_verifier_binds_every_publication_and_bootstrap_result() {
    let publication = PairEvidenceFixture::new();
    let manifest = read_manifest(&publication.roots[0]);
    let simple_group = manifest["simple_group_id"].as_str().expect("group");
    let shared = manifest["shared_event_id"].as_str().expect("shared id");
    let replacement = signed_fixture_event(&publication.authors[0], 9, simple_group, "arbitrary-valid");
    publication.rewrite_wire_values(0, "a", |frames| {
        let event = frames
            .iter_mut()
            .find(|frame| {
                decoded_kind(frame) == Some("EVENT")
                    && frame.get("direction").and_then(Value::as_str) == Some("client_to_relay")
                    && frame["decoded"][1]["id"].as_str() == Some(shared)
            })
            .expect("shared publication");
        event["decoded"][1] = serde_json::to_value(&replacement).expect("replacement event");
        event["payload"] = json!(serde_json::to_string(&event["decoded"]).expect("payload"));
        let accepted = frames
            .iter_mut()
            .find(|frame| {
                decoded_kind(frame) == Some("OK") && frame["decoded"][1].as_str() == Some(shared)
            })
            .expect("shared accepted OK");
        accepted["decoded"][1] = json!(replacement.id.to_hex());
        accepted["payload"] = json!(serde_json::to_string(&accepted["decoded"]).expect("payload"));
    });
    assert!(
        verify_fixture_pair(publication.root()).is_err(),
        "resealed arbitrary valid publication must not prove the claimed shared result"
    );

    let bootstrap = PairEvidenceFixture::new();
    let simple_group = read_manifest(&bootstrap.roots[0])["simple_group_id"]
        .as_str()
        .expect("group")
        .to_owned();
    let replacement = signed_fixture_event(&bootstrap.authors[0], 9007, &simple_group, "not-published");
    bootstrap.rewrite_wire_values(0, "a", |frames| {
        let response = frames
            .iter_mut()
            .find(|frame| {
                decoded_kind(frame) == Some("EVENT")
                    && decoded_subscription(frame) == Some("bootstrap-0")
            })
            .expect("bootstrap response");
        response["decoded"][2] = serde_json::to_value(&replacement).expect("replacement event");
        response["payload"] = json!(serde_json::to_string(&response["decoded"]).expect("payload"));
    });
    assert!(
        verify_fixture_pair(bootstrap.root()).is_err(),
        "auxiliary query must return the exact bootstrap publication"
    );
}

#[test]
fn pair_verifier_binds_metadata_and_admin_command_semantics() {
    for kind in [9002_u16, 9000] {
        let fixture = PairEvidenceFixture::new();
        let manifest = read_manifest(&fixture.roots[0]);
        let simple_group = manifest["simple_group_id"].as_str().expect("group");
        let wrong = if kind == 9002 {
            EventBuilder::new(Kind::from(kind), "")
                .tags([
                    Tag::parse(["name", "wrong-metadata-command"]).expect("name tag"),
                    Tag::parse(["h", simple_group]).expect("h tag"),
                ])
                .custom_created_at(Timestamp::from(19_002))
                .finalize(&fixture.authors[0])
                .expect("wrong metadata command")
        } else {
            EventBuilder::new(Kind::from(kind), "")
                .tags([
                    Tag::parse(["p", &Keys::generate().public_key().to_hex(), "admin"])
                        .expect("admin tag"),
                    Tag::parse(["h", simple_group]).expect("h tag"),
                ])
                .custom_created_at(Timestamp::from(19_000))
                .finalize(&fixture.authors[0])
                .expect("wrong admin command")
        };
        fixture.rewrite_wire_values(0, "a", |frames| {
            let event = frames
                .iter_mut()
                .find(|frame| {
                    decoded_kind(frame) == Some("EVENT")
                        && frame.get("direction").and_then(Value::as_str) == Some("client_to_relay")
                        && frame["decoded"][1]["kind"].as_u64() == Some(u64::from(kind))
                })
                .expect("management command");
            let original = event["decoded"][1]["id"]
                .as_str()
                .expect("event id")
                .to_owned();
            event["decoded"][1] = serde_json::to_value(&wrong).expect("wrong command JSON");
            event["payload"] = json!(serde_json::to_string(&event["decoded"]).expect("payload"));
            let accepted = frames
                .iter_mut()
                .find(|frame| {
                    decoded_kind(frame) == Some("OK")
                        && frame["decoded"][1].as_str() == Some(&original)
                })
                .expect("management OK");
            accepted["decoded"][1] = json!(wrong.id.to_hex());
            accepted["payload"] =
                json!(serde_json::to_string(&accepted["decoded"]).expect("payload"));
        });
        assert!(
            verify_fixture_pair(fixture.root()).is_err(),
            "arbitrary valid kind-{kind} must not prove the claimed management outcome"
        );
    }
}

#[test]
fn pair_verifier_derives_the_current_replacement_winner() {
    let fixture = PairEvidenceFixture::new();
    let manifest = read_manifest(&fixture.roots[0]);
    let simple_group = manifest["simple_group_id"].as_str().expect("group");
    let wrong = EventBuilder::new(Kind::from(39000), "")
        .tags([
            Tag::parse(["d", simple_group]).expect("d tag"),
            Tag::parse(["name", "wrong-newer-winner"]).expect("name tag"),
        ])
        .custom_created_at(Timestamp::from(99_999))
        .finalize(&fixture.relays[0])
        .expect("wrong winner");
    fixture.rewrite_wire_values(0, "a", |frames| {
        let eose = frames
            .iter()
            .position(|frame| {
                decoded_kind(frame) == Some("EOSE")
                    && decoded_subscription(frame) == Some("records-0")
            })
            .expect("records EOSE");
        let mut response = frames[eose - 1].clone();
        response["decoded"] = json!(["EVENT", "records-0", wrong]);
        response["payload"] = json!(serde_json::to_string(&response["decoded"]).expect("payload"));
        frames.insert(eose, response);
    });
    assert!(
        verify_fixture_pair(fixture.root()).is_err(),
        "an older expected record must not hide a newer wrong winner"
    );
}

#[test]
fn pair_verifier_binds_distinct_run_ids_to_directory_basenames() {
    let duplicate = PairEvidenceFixture::new();
    let flow_path = duplicate.roots[1].join("flow.json");
    let mut flow: Value =
        serde_json::from_slice(&fs::read(&flow_path).expect("flow")).expect("flow JSON");
    flow["write_id"] = json!(2);
    flow["receipt_id"] = json!(2);
    fs::write(
        &flow_path,
        serde_json::to_vec_pretty(&flow).expect("flow bytes"),
    )
    .expect("flow mutation");
    duplicate.mutate(1, true, |manifest| {
        manifest["run_id"] = json!("run-0");
        manifest["write_id"] = json!("run-0:2");
        manifest["receipt_id"] = json!("run-0:2");
    });
    assert!(
        verify_fixture_pair(duplicate.root()).is_err(),
        "distinct write suffixes must not hide a duplicate, misbound run id"
    );
}

#[test]
fn pair_verifier_excludes_the_other_run_id_from_retained_artifacts() {
    let fixture = PairEvidenceFixture::new();
    fs::write(fixture.roots[0].join("cross-run-id.txt"), "run-1").expect("cross-run id mutation");
    fixture.mutate(0, true, |_| {});
    assert!(
        verify_fixture_pair(fixture.root()).is_err(),
        "a run must not retain the other run's exact identity"
    );
}

#[test]
fn pair_verifier_requires_external_fava_source_proof() {
    let fixture = PairEvidenceFixture::new();
    let source_path = fixture.roots[0].join("source/fava.json");
    let hostile_tree = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut source: Value = serde_json::from_slice(&fs::read(&source_path).expect("source proof"))
        .expect("source JSON");
    source["fava_source_tree_sha256"] = json!(hostile_tree);
    fs::write(
        &source_path,
        serde_json::to_vec_pretty(&source).expect("source bytes"),
    )
    .expect("source mutation");
    fixture.mutate(0, true, |manifest| {
        manifest["fava_source_tree_sha256"] = json!(hostile_tree);
    });
    assert!(
        verify_fixture_pair(fixture.root()).is_err(),
        "a self-consistent producer claim must not replace expected committed-source proof"
    );
}

#[test]
fn pair_verifier_requires_one_exact_croissant_identity_for_all_children() {
    for field in ["source_head", "executable_sha256"] {
        let fixture = PairEvidenceFixture::new();
        let processes_path = fixture.roots[1].join("children/processes.json");
        let mut processes: Value =
            serde_json::from_slice(&fs::read(&processes_path).expect("process proof"))
                .expect("process JSON");
        let hostile = if field == "source_head" {
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        } else {
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        };
        processes["ready"][1][field] = json!(hostile);
        fs::write(
            &processes_path,
            serde_json::to_vec_pretty(&processes).expect("process bytes"),
        )
        .expect("process mutation");
        fixture.mutate(1, true, |manifest| {
            manifest["ready"] = processes["ready"].clone();
        });
        assert!(
            verify_fixture_pair(fixture.root()).is_err(),
            "mixed child {field} must not prove the exact Croissant identity"
        );
    }
}
