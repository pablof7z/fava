use super::*;

#[tokio::test(flavor = "current_thread")]
async fn wrong_injected_timestamp_refuses_first_and_preserves_successor_current() {
    let first_keys = Keys::generate();
    let first_store = Arc::new(MemoryWriteStore::default());
    let first_applier = Arc::new(ControlledApplier::new(Kind::ContactList));
    first_applier.set(WRONG_TIMESTAMP);
    let first = assembly(
        &first_keys,
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&first_store),
        vec![Arc::clone(&first_applier)],
    );
    assert!(
        first
            .by(first_keys.public_key())
            .to([support::relay_url()])
            .expect("explicit route validates")
            .publish(failure_support::edit(Kind::ContactList))
            .is_err()
    );
    assert_eq!(first_applier.calls(), 1);
    assert!(first_store.is_empty().expect("first store remains empty"));

    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let applier = Arc::new(ControlledApplier::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::clone(&store),
        vec![Arc::clone(&applier)],
    );
    let accepted = publish_edit(&fava, keys.public_key(), Kind::ContactList);
    let accepted_event_id = accepted
        .receipt()
        .expect("accepted receipt reads")
        .current
        .id();
    applier.set(WRONG_TIMESTAMP);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "new source", &[]),
    );
    let failed = wait_failure(&fava, accepted.receipt_id()).await;

    assert_eq!(failed.current.id(), accepted_event_id);
    assert_eq!(
        failed.current.publication.revision_id,
        RevisionId::FIRST
    );
    assert!(
        failed
            .current
            .publication
            .revision_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("injected timestamp"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_author_or_kind_refuses_before_custody() {
    for mode in [WRONG_ACTOR, WRONG_KIND] {
        let keys = Keys::generate();
        let store = Arc::new(MemoryWriteStore::default());
        let applier = Arc::new(ControlledApplier::new(Kind::ContactList));
        applier.set(mode);
        let fava = assembly(
            &keys,
            Arc::new(MemoryEventCache::default()),
            Arc::clone(&store),
            vec![Arc::clone(&applier)],
        );

        assert!(
            fava.by(keys.public_key())
                .to([support::relay_url()])
                .expect("explicit route validates")
                .publish(failure_support::edit(Kind::ContactList))
                .is_err()
        );
        assert_eq!(applier.calls(), 1);
        assert!(store.is_empty().expect("refusal leaves zero custody"));
    }
}
