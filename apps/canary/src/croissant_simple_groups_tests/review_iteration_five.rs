#[test]
fn verifier_refuses_renamed_executable_residue_during_snapshot() {
    let fixture = PairEvidenceFixture::new();
    let directory = fixture.roots[0].join("relays/a/executable-old");
    fs::create_dir_all(&directory).expect("residual executable directory");
    fs::write(directory.join("croissant"), b"residual executable")
        .expect("residual executable");
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_refuses_resealed_missing_executable_cleanup_fact() {
    let fixture = PairEvidenceFixture::new();
    let processes_path = fixture.roots[0].join("children/processes.json");
    let mut processes: Value =
        serde_json::from_slice(&fs::read(&processes_path).expect("processes read"))
            .expect("processes json");
    processes["teardown"][0]["executable_removed"] = json!(false);
    fs::write(
        &processes_path,
        serde_json::to_vec_pretty(&processes).expect("processes bytes"),
    )
    .expect("processes mutation");
    fixture.mutate(0, true, |manifest| {
        manifest["teardown"][0]["executable_removed"] = json!(false);
    });
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_hashes_resealed_retained_fava_image_itself() {
    let fixture = PairEvidenceFixture::new();
    fs::write(
        fixture.roots[0].join("source/fava-canary"),
        b"hostile pinned fava image!\n",
    )
    .expect("replace retained image");
    fixture.mutate(0, true, |_| {});
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_refuses_unfalsifiable_reproducible_build_claim() {
    let fixture = PairEvidenceFixture::new();
    let source_path = fixture.roots[0].join("source/fava.json");
    let mut source: Value =
        serde_json::from_slice(&fs::read(&source_path).expect("source proof"))
            .expect("source proof json");
    source["fava_binary_reproduced_from_source"] = json!(true);
    fs::write(
        source_path,
        serde_json::to_vec_pretty(&source).expect("source proof bytes"),
    )
    .expect("unfalsifiable claim");
    fixture.mutate(0, true, |_| {});
    assert!(verify_fixture_pair(fixture.root()).is_err());
}

#[test]
fn verifier_refuses_darwin_test_fallback_as_live_proof() {
    let fixture = PairEvidenceFixture::new();
    fixture.mutate(0, true, |manifest| {
        manifest["ready"][0]["execution_platform"] = json!("darwin-test-only-path");
    });
    assert!(verify_fixture_pair(fixture.root()).is_err());
}
