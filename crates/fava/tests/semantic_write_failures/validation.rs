use super::*;

#[tokio::test(flavor = "current_thread")]
async fn wrong_injected_timestamp_refuses_first_and_preserves_successor_current() {
    let first_keys = Keys::generate();
    let first_store = Arc::new(MemoryWriteStore::default());
    let first_materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    first_materializer.set(WRONG_TIMESTAMP);
    let first = assembly(
        &first_keys,
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&first_store),
        vec![Arc::clone(&first_materializer)],
    );
    assert!(
        first
            .publish(edit_intent(first_keys.public_key(), Kind::ContactList))
            .is_err()
    );
    assert_eq!(first_materializer.calls(), 1);
    assert!(first_store.is_empty().expect("first store remains empty"));

    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::clone(&cache),
        Arc::clone(&store),
        vec![Arc::clone(&materializer)],
    );
    let accepted = fava
        .publish(edit_intent(keys.public_key(), Kind::ContactList))
        .expect("valid first generation accepts");
    materializer.set(WRONG_TIMESTAMP);
    save_source(
        &cache,
        signed_source(&keys, Kind::ContactList, 10, "new source", &[]),
    );
    let failed = wait_failure(&fava, accepted.receipt_id).await;

    assert_eq!(failed.current.id(), accepted.current.id());
    assert_eq!(
        failed.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert!(
        failed
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|reason| reason.contains("injected timestamp"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_author_or_kind_refuses_before_custody() {
    for mode in [WRONG_ACTOR, WRONG_KIND] {
        let keys = Keys::generate();
        let store = Arc::new(MemoryWriteStore::default());
        let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
        materializer.set(mode);
        let fava = assembly(
            &keys,
            Arc::new(MemoryEventCache::default()),
            Arc::clone(&store),
            vec![Arc::clone(&materializer)],
        );

        assert!(
            fava.publish(edit_intent(keys.public_key(), Kind::ContactList))
                .is_err()
        );
        assert_eq!(materializer.calls(), 1);
        assert!(store.is_empty().expect("refusal leaves zero custody"));
    }
}
