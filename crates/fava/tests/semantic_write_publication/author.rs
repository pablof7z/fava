use super::*;

#[tokio::test(flavor = "current_thread")]
async fn accepted_author_scopes_sources_signing_and_every_generation() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let alice_signer = Arc::new(BlockingSigner::new(alice.public_key()));
    let bob_signer = Arc::new(CountingSigner::new(bob.clone()));
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&alice_signer),
        Arc::new(RecordingPublisher::default()),
    )
    .signer(Arc::clone(&bob_signer))
    .applier(Arc::clone(&applier))
    .build()
    .expect("two-signer semantic assembly");

    let write = fava
        .by(alice.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit(Kind::ContactList))
        .expect("Alice's edit accepts");
    wait_for_signer(&alice_signer, 1).await;
    assert_eq!(
        write.receipt().unwrap().current.event.author(),
        alice.public_key()
    );
    assert_eq!(applier.calls()[0].author, alice.public_key());
    assert_eq!(bob_signer.calls(), 0);

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(&bob, Kind::ContactList, 50, "Bob", &[]),
            relay_occurrence(),
        ))])
        .expect("Bob's unrelated coordinate enters cache");
    assert_no_receipt_change(&store).await;
    assert_eq!(applier.calls().len(), 1);

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(&alice, Kind::ContactList, 10, "Alice", &[]),
            relay_occurrence(),
        ))])
        .expect("Alice's successor enters cache");
    let successor = wait_for_revision(&fava, write.receipt_id(), 2).await;
    wait_for_signer(&alice_signer, 2).await;
    assert_eq!(successor.current.event.author(), alice.public_key());
    assert!(
        applier
            .calls()
            .iter()
            .all(|call| call.author == alice.public_key())
    );
    assert_eq!(bob_signer.calls(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn recovery_uses_persisted_author_when_only_bob_signer_is_selected() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(MemoryWriteStore::default());
    let accepted = store
        .accept_applied_edit(
            intent(alice.public_key(), Kind::ContactList),
            EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("accepted as Alice")
                .by(alice.public_key())
                .build()
                .unwrap(),
            None,
        )
        .expect("Alice's accepted edit is durable");
    let bob_signer = Arc::new(CountingSigner::new(bob));
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&bob_signer),
        Arc::new(RecordingPublisher::default()),
    )
    .applier(Arc::clone(&applier))
    .build()
    .expect("recovery starts with only Bob's signer selected");

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(&alice, Kind::ContactList, 10, "Alice source", &[]),
            relay_occurrence(),
        ))])
        .expect("Alice's source enters after recovery");
    let recovered = wait_for_revision(&fava, accepted.receipt_id, 2).await;

    assert_eq!(recovered.current.event.author(), alice.public_key());
    assert_eq!(applier.calls().len(), 1);
    assert_eq!(applier.calls()[0].author, alice.public_key());
    assert_eq!(bob_signer.calls(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn addressable_edit_selects_only_its_exact_identifier() {
    let keys = Keys::generate();
    let kind = Kind::Custom(30_023);
    let wanted = NostrEventBuilder::new(kind, "wanted")
        .tag(Tag::identifier("wanted"))
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .unwrap();
    let unrelated = NostrEventBuilder::new(kind, "unrelated")
        .tag(Tag::identifier("other"))
        .custom_created_at(Timestamp::from(20))
        .finalize(&keys)
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![
            EventStateMutation::Upsert(relay_event(wanted.clone(), relay_occurrence())),
            EventStateMutation::Upsert(relay_event(unrelated, relay_occurrence())),
        ])
        .unwrap();
    let applier = Arc::new(TestApplier::new(kind));
    let (fava, _, _, signer, _) = assembly_with_cache(
        cache,
        Arc::new(MemoryWriteStore::default()),
        keys.clone(),
        vec![Arc::clone(&applier)],
    );
    let edit = EventEdit::new(kind, Some("wanted".to_owned()), vec![1]).unwrap();
    let write = fava
        .by(keys.public_key())
        .to([relay_url()])
        .expect("route validates")
        .publish(edit)
        .expect("addressable edit accepts");
    let receipt = write.settled(all_terminal()).await.unwrap();

    assert_eq!(receipt.current.event.author(), keys.public_key());
    assert_eq!(applier.calls().len(), 1);
    assert_eq!(applier.calls()[0].identifier.as_deref(), Some("wanted"));
    assert_eq!(
        applier.calls()[0].source.as_ref().and_then(EventValue::id),
        Some(wanted.id)
    );
    assert_eq!(signer.calls(), 1);
}
