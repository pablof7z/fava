#[test]
fn verifier_refuses_resealed_build_attestation_for_another_executable() {
    let fixture = PairEvidenceFixture::new();
    let path = fixture.roots[0].join("source/fava-build.json");
    let mut build: Value = serde_json::from_slice(&fs::read(&path).expect("build attestation"))
        .expect("build attestation JSON");
    build["fava_canary_executable_sha256"] =
        json!("abababababababababababababababababababababababababababababababab");
    fs::write(
        path,
        serde_json::to_vec_pretty(&build).expect("mutated build attestation"),
    )
    .expect("write mutated build attestation");
    fixture.mutate(0, true, |_| {});
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_refuses_resealed_source_manifest_with_valid_arbitrary_bytes() {
    let fixture = PairEvidenceFixture::new();
    let path = fixture.roots[0].join("source/fava-build-source.manifest");
    let bytes = fs::read_to_string(&path).expect("source manifest");
    fs::write(&path, bytes.replace("apps/canary/src/main.rs", "apps/canary/src/fake.rs"))
        .expect("mutated source manifest");
    fixture.mutate(0, true, |_| {});
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_refuses_resealed_unproven_build_image_identity() {
    let fixture = PairEvidenceFixture::new();
    let zero = "0".repeat(64);
    for index in 0..2 {
        let build_path = fixture.roots[index].join("source/fava-build.json");
        let mut build: Value =
            serde_json::from_slice(&fs::read(&build_path).expect("build attestation"))
                .expect("build attestation JSON");
        build["fava_build_source_image_sha256"] = json!(zero);
        fs::write(
            build_path,
            serde_json::to_vec_pretty(&build).expect("mutated build attestation"),
        )
        .expect("write mutated build attestation");
        let source_path = fixture.roots[index].join("source/fava.json");
        let mut source: Value =
            serde_json::from_slice(&fs::read(&source_path).expect("source provenance"))
                .expect("source provenance JSON");
        source["fava_build_source_image_sha256"] = json!(zero);
        fs::write(
            source_path,
            serde_json::to_vec_pretty(&source).expect("mutated source provenance"),
        )
        .expect("write mutated source provenance");
        fixture.mutate(index, true, |manifest| {
            manifest["fava_build_source_image_sha256"] = json!(zero);
        });
    }
    assert!(
        verify_croissant_simple_groups_pair(
            fixture.root(),
            FIXTURE_FAVA_REVISION,
            FIXTURE_FAVA_TREE,
            FIXTURE_FAVA_BUILD_TREE,
            &zero,
            FIXTURE_FAVA_SOURCE_MANIFEST,
            FIXTURE_FAVA_RUST_BASE_IMAGE,
            FIXTURE_FAVA_EXECUTABLE,
            FIXTURE_CROISSANT_REVISION,
            FIXTURE_CROISSANT_EXECUTABLE,
        )
        .is_err(),
        "all-zero engine identity was accepted after consistent resealing"
    );
}

#[test]
fn verifier_refuses_resealed_host_bound_build_subject() {
    let fixture = PairEvidenceFixture::new();
    let build_path = fixture.roots[0].join("source/fava-build.json");
    let mut build: Value = serde_json::from_slice(
        &fs::read(&build_path).expect("build attestation"),
    )
    .expect("build attestation JSON");
    build["target_storage"] = json!("host-bind");
    build["subject_digest_origin"] = json!("host");
    fs::write(
        build_path,
        serde_json::to_vec_pretty(&build).expect("mutated build attestation"),
    )
    .expect("write mutated build attestation");
    fixture.mutate(0, true, |manifest| {
        manifest["fava_build_target_storage"] = json!("host-bind");
        manifest["fava_build_subject_digest_origin"] = json!("host");
    });
    assert!(
        verify_fixture_pair(fixture.root()).is_err(),
        "host-bound subject claim was accepted after consistent resealing"
    );
}
