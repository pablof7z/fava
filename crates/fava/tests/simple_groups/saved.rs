#[tokio::test(flavor = "current_thread")]
async fn simple_group_saved_edit_uses_ordinary_semantic_lifecycle() {
    let first_keys = Keys::generate();
    let second_keys = Keys::generate();
    let first_signer = Arc::new(BlockingSigner::new(first_keys.public_key()));
    let second_signer = Arc::new(BlockingSigner::new(second_keys.public_key()));
    let harness = Harness::new_with_signers_and_materializers(
        [
            Arc::clone(&first_signer) as Arc<dyn Signer>,
            Arc::clone(&second_signer) as Arc<dyn Signer>,
        ],
        [SimpleGroups::materializer()],
    );
    let group = Group::on([host("saved")], "photos").expect("group");
    let first_edit = SimpleGroups::save_group(&group, Some("Photos")).expect("save edit");
    let second_edit = SimpleGroups::save_relay(host("inert-saved")).expect("relay edit");

    let first = harness
        .fava
        .by(first_keys.public_key())
        .to(group.hosts())
        .expect("first exact route")
        .publish(first_edit)
        .expect("first semantic custody");
    let second = harness
        .fava
        .by(second_keys.public_key())
        .to(group.hosts())
        .expect("second exact route")
        .publish(second_edit)
        .expect("second semantic custody");
    wait_until(|| first_signer.calls() == 1 && second_signer.calls() == 1).await;

    let first_receipt = first.receipt().expect("first receipt");
    let second_receipt = second.receipt().expect("second receipt");
    assert_ne!(first.write_id(), second.write_id());
    assert_ne!(first.receipt_id(), second.receipt_id());
    assert_eq!(first_receipt.current.event.kind(), Kind::from_u16(10_009));
    assert_eq!(second_receipt.current.event.kind(), Kind::from_u16(10_009));
    assert_eq!(
        first_receipt.routing,
        WriteRouting::Explicit(vec![host("saved")])
    );
    assert_eq!(second_receipt.routing, first_receipt.routing);
    assert_eq!(
        BTreeSet::from([
            operation_generation(&first_receipt),
            operation_generation(&second_receipt),
        ])
        .len(),
        2,
        "write, receipt, and per-write generation form one isolated operation identity",
    );
    assert_eq!(harness.store.len().expect("store readable"), 2);
    assert!(harness.publisher.attempts().is_empty());
    assert_eq!(harness.router.calls.load(Ordering::SeqCst), 0);
    assert_eq!(harness.transport.opens.load(Ordering::SeqCst), 0);

    let before = harness.store.len().expect("store readable");
    assert!(SimpleGroups::save_group(&group, Some(&"x".repeat(4_097))).is_err());
    let saved = NostrEventBuilder::new(Kind::from_u16(10_009), "opaque")
        .tags([tag(&["r", "wss://parsed-only.example"])])
        .custom_created_at(Timestamp::from(90))
        .finalize(&first_keys)
        .expect("saved relay event signs");
    let parsed = SavedRelay::from_event(&EventValue::Signed(saved)).expect("saved relay parses");
    assert_eq!(parsed.len(), 1);
    assert_eq!(harness.store.len().expect("store readable"), before);
    assert_eq!(harness.router.calls.load(Ordering::SeqCst), 0);
    assert_eq!(harness.transport.opens.load(Ordering::SeqCst), 0);

    harness
        .fava
        .cancel_publication(first.receipt_id())
        .expect("first cancels");
    harness
        .fava
        .cancel_publication(second.receipt_id())
        .expect("second cancels");
}

#[tokio::test(flavor = "current_thread")]
async fn simple_group_management_events_are_author_bearing() {
    let keys = Keys::generate();
    let signer = Arc::new(ExactSigner::new(keys.clone()));
    let harness = Harness::new(Arc::clone(&signer) as Arc<dyn Signer>);
    let group = Group::on([host("management")], "photos").expect("group");
    let metadata = group
        .edit_metadata(
            EventBuilder::new(keys.public_key(), Kind::from_u16(9_002))
                .created_at(Timestamp::from(9_002))
                .tag(tag(&["name", "Photos"]))
                .build()
                .expect("metadata draft"),
        )
        .expect("metadata context");
    let pins = group
        .set_pins(
            EventBuilder::new(keys.public_key(), Kind::from_u16(9_010))
                .created_at(Timestamp::from(9_010))
                .tag(tag(&["e", &"11".repeat(32)]))
                .build()
                .expect("pin draft"),
        )
        .expect("pin context");

    assert_eq!(metadata.pubkey, keys.public_key());
    assert_eq!(metadata.kind, Kind::from_u16(9_002));
    assert_eq!(pins.pubkey, keys.public_key());
    assert_eq!(pins.kind, Kind::from_u16(9_010));
    assert!(
        metadata
            .tags
            .iter()
            .any(|row| row.as_slice() == ["h", "photos"])
    );
    assert!(
        pins.tags
            .iter()
            .any(|row| row.as_slice() == ["h", "photos"])
    );

    let metadata_write = harness
        .fava
        .to(group.hosts())
        .expect("metadata route")
        .publish(metadata)
        .expect("ordinary metadata custody");
    let pins_write = harness
        .fava
        .to(group.hosts())
        .expect("pins route")
        .publish(pins)
        .expect("ordinary pins custody");
    assert_ordinary_write(&metadata_write);
    assert_ordinary_write(&pins_write);
    assert_ne!(metadata_write.write_id(), pins_write.write_id());
    wait_until(|| signer.calls() == 2).await;
    wait_until(|| harness.publisher.attempts().len() == 2).await;
    assert!(
        harness
            .publisher
            .attempts()
            .iter()
            .any(|attempt| attempt.event.kind == Kind::from_u16(9_002))
    );
    assert!(
        harness
            .publisher
            .attempts()
            .iter()
            .any(|attempt| attempt.event.kind == Kind::from_u16(9_010))
    );
}

fn operation_generation(
    receipt: &fava::Receipt,
) -> (fava::WriteId, fava::ReceiptId, MaterializationId) {
    (
        receipt.write_id,
        receipt.receipt_id,
        receipt.current.publication.materialization_id,
    )
}

fn assert_ordinary_write(_write: &fava::Write) {}

fn group() -> Group {
    Group::on(
        [host("a"), host("b"), host("contacted-but-not-serving")],
        "group-29",
    )
    .expect("group is valid")
}

fn host(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn tag(cells: &[&str]) -> Tag {
    Tag::parse(cells.iter().copied()).expect("test tag")
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline");
}
