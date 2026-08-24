#[test]
fn verifier_refuses_extra_metadata_and_admin_command_effects() {
    for metadata in [true, false] {
        let fixture = PairEvidenceFixture::new();
        let root = &fixture.roots[0];
        let manifest = read_manifest(root);
        let simple_group = manifest["simple_group_id"].as_str().expect("group");
        let event = if metadata {
            EventBuilder::new(Kind::from(9002), "")
                .tags([
                    Tag::parse(["name", manifest["metadata_names"][0].as_str().unwrap()]).unwrap(),
                    Tag::parse(["about", "A-only metadata"]).unwrap(),
                    Tag::parse(["picture", "https://example.invalid/extra.png"]).unwrap(),
                    Tag::parse(["h", simple_group]).unwrap(),
                ])
                .custom_created_at(Timestamp::from(9_003))
                .finalize(&fixture.authors[0])
                .unwrap()
        } else {
            EventBuilder::new(Kind::from(9000), "")
                .tags([
                    Tag::parse([
                        "p",
                        manifest["admin_targets"][0].as_str().unwrap(),
                        "admin",
                    ])
                    .unwrap(),
                    Tag::parse(["p", &Keys::generate().public_key().to_hex(), "admin"]).unwrap(),
                    Tag::parse(["h", simple_group]).unwrap(),
                ])
                .custom_created_at(Timestamp::from(9_001))
                .finalize(&fixture.authors[0])
                .unwrap()
        };
        let kind = if metadata { 9002 } else { 9000 };
        let connection = if metadata { 3 } else { 4 };
        assert_eq!(event.kind.as_u16(), kind);
        fixture.rewrite_wire_values(0, "a", |frames| {
            for frame in frames {
                if frame["connection"] == connection && decoded_kind(frame) == Some("EVENT") {
                    frame["decoded"][1] = serde_json::to_value(&event).unwrap();
                    frame["payload"] =
                        json!(serde_json::to_string(&frame["decoded"]).expect("payload"));
                }
                if frame["connection"] == connection && decoded_kind(frame) == Some("OK") {
                    frame["decoded"][1] = json!(event.id.to_hex());
                    frame["payload"] =
                        json!(serde_json::to_string(&frame["decoded"]).expect("payload"));
                }
            }
        });
        assert!(verify_fixture_pair(fixture.root()).is_err());
    }
}

#[test]
fn verifier_refuses_resealed_wrong_fava_canary_executable() {
    let fixture = PairEvidenceFixture::new();
    let hostile = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let source_path = fixture.roots[0].join("source/fava.json");
    let mut source: Value = serde_json::from_slice(&fs::read(&source_path).unwrap()).unwrap();
    source["fava_canary_executable_sha256"] = json!(hostile);
    fs::write(&source_path, serde_json::to_vec_pretty(&source).unwrap()).unwrap();
    fixture.mutate(0, true, |manifest| {
        manifest["fava_canary_executable_sha256"] = json!(hostile);
    });
    assert!(verify_fixture_pair(fixture.root()).is_err());
}
