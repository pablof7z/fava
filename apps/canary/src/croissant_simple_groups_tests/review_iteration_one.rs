#[test]
fn pair_verifier_rejects_unsafe_evidence() {
    let control = PairEvidenceFixture::new();
    verify_croissant_simple_groups_pair(control.root(), "fixture-revision")
        .expect("narrow safe pair verifies");

    for case in [
        UnsafePairCase::PersistentParentSecret,
        UnsafePairCase::IncompleteCleanup,
        UnsafePairCase::UnsignedClaim,
        UnsafePairCase::ReusedIdentity,
        UnsafePairCase::ReusedUniqueIdentity,
        UnsafePairCase::CrossRunData,
        UnsafePairCase::CrossRunUniqueData,
        UnsafePairCase::ExtraManifest,
        UnsafePairCase::MissingManifest,
        UnsafePairCase::StagingResidue,
        UnsafePairCase::UnderivedFlowClaim,
        UnsafePairCase::UnderivedProcessClaim,
        UnsafePairCase::MissingExactClose,
        UnsafePairCase::ExtraSignedHandoff,
        UnsafePairCase::RetainedSecretMarker,
    ] {
        let fixture = PairEvidenceFixture::new();
        fixture.apply(case);
        assert!(
            verify_croissant_simple_groups_pair(fixture.root(), "fixture-revision").is_err(),
            "pair verifier accepted unsafe fixture {case:?}"
        );
    }
}

#[test]
fn pair_verifier_requires_shared_evidence_route_order() {
    let fixture = PairEvidenceFixture::new();
    fixture.reverse_shared_evidence(0);
    assert!(
        verify_croissant_simple_groups_pair(fixture.root(), "fixture-revision").is_err(),
        "shared evidence must stay bound to each exact relay route"
    );
}

#[derive(Clone, Copy, Debug)]
enum UnsafePairCase {
    PersistentParentSecret,
    IncompleteCleanup,
    UnsignedClaim,
    ReusedIdentity,
    ReusedUniqueIdentity,
    CrossRunData,
    CrossRunUniqueData,
    ExtraManifest,
    MissingManifest,
    StagingResidue,
    UnderivedFlowClaim,
    UnderivedProcessClaim,
    MissingExactClose,
    ExtraSignedHandoff,
    RetainedSecretMarker,
}

impl PairEvidenceFixture {
    fn root(&self) -> &Path {
        self.temporary.path()
    }
}

fn wire_line(sequence: u64, connection: u64, direction: &str, payload: &Value) -> String {
    serde_json::to_string(&json!({
        "sequence": sequence,
        "connection": connection,
        "unix_ms": sequence,
        "direction": direction,
        "frame_type": "text",
        "payload": serde_json::to_string(&payload).expect("payload json"),
    }))
    .expect("wire line")
}

fn read_manifest(root: &Path) -> Value {
    serde_json::from_slice(&fs::read(root.join("manifest.json")).expect("manifest read"))
        .expect("manifest json")
}
