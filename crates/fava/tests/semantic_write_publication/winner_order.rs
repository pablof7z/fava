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
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            base.clone(),
            relay_evidence(),
        ))])
        .expect("base source enters cache");
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .materializer(Arc::clone(&materializer))
    .build()
    .expect("semantic publication assembly");
    let accepted = fava
        .publish(intent(keys.public_key(), Kind::ContactList))
        .expect("edit accepts");
    wait_for_signer(&signer, 1).await;

    let older = signed_source(&keys, Kind::ContactList, u64::MAX - 3, "older", &[]);
    let wrong_actor = signed_source(&other, Kind::ContactList, u64::MAX - 1, "wrong actor", &[]);
    let wrong_kind = signed_source(&keys, Kind::TextNote, u64::MAX - 1, "wrong kind", &[]);
    let equal_id = equal.id;
    cache
        .commit(vec![
            CacheMutation::Upsert(CachedEvent::new(equal, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(older, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(wrong_actor, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(wrong_kind, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(base.clone(), relay_evidence())),
        ])
        .expect("winner and inert source facts enter cache");
    let receipt = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    wait_for_signer(&signer, 2).await;
    assert_eq!(
        receipt.current.publication.materialization_source,
        Some(equal_id)
    );
    assert_eq!(materializer.calls().len(), 2);

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            base,
            relay_evidence(),
        ))])
        .expect("losing equal-time source repeats");
    assert_no_receipt_change(&store).await;
    let receipt = fava.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(
        receipt.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(materializer.calls().len(), 2);
}
