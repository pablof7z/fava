use super::*;

#[tokio::test(flavor = "current_thread")]
async fn equal_timestamp_lower_id_wins_while_higher_id_and_unqualified_sources_are_inert() {
    let keys = Keys::generate();
    let other = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let left = signed_source(&keys, Kind::ContactList, u64::MAX - 2, "left", &[]);
    let right = signed_source(&keys, Kind::ContactList, u64::MAX - 2, "right", &[]);
    let (base, equal) = if left.id > right.id {
        (left, right)
    } else {
        (right, left)
    };
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            base.clone(),
            relay_occurrence(),
        ))])
        .expect("base source enters cache");
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .applier(Arc::clone(&applier))
    .build()
    .expect("semantic publication assembly");
    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;

    let older = signed_source(&keys, Kind::ContactList, u64::MAX - 3, "older", &[]);
    let wrong_actor = signed_source(&other, Kind::ContactList, u64::MAX - 1, "wrong actor", &[]);
    let wrong_kind = signed_source(&keys, Kind::TextNote, u64::MAX - 1, "wrong kind", &[]);
    let equal_id = equal.id;
    cache
        .commit(vec![
            EventStateMutation::Upsert(relay_event(equal, relay_occurrence())),
            EventStateMutation::Upsert(relay_event(older, relay_occurrence())),
            EventStateMutation::Upsert(relay_event(wrong_actor, relay_occurrence())),
            EventStateMutation::Upsert(relay_event(wrong_kind, relay_occurrence())),
            EventStateMutation::Upsert(relay_event(base.clone(), relay_occurrence())),
        ])
        .expect("winner and inert source facts enter cache");
    let receipt = wait_for_revision(&fava, write.receipt_id(), 2).await;
    wait_for_signer(&signer, 2).await;
    assert_eq!(receipt.current.publication.revision_source, Some(equal_id));
    assert_eq!(applier.calls().len(), 2);

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            base,
            relay_occurrence(),
        ))])
        .expect("losing equal-time source repeats");
    assert_no_receipt_change(&store).await;
    let receipt = write.receipt().unwrap();
    assert_eq!(
        receipt.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(applier.calls().len(), 2);
}
