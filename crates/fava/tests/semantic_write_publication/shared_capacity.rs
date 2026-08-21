use super::*;

#[tokio::test(flavor = "current_thread")]
async fn shared_store_capacity_refuses_before_second_publication_provider_effect() {
    let store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let first_keys = Keys::generate();
    let second_keys = Keys::generate();
    let first_materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let second_materializer = Arc::new(TestMaterializer::new(Kind::ContactList, EDIT_FORMAT));
    let (first, _, _, _, _) = assembly(
        Arc::clone(&store),
        first_keys.clone(),
        vec![Arc::clone(&first_materializer)],
    );
    let (second, _, _, second_signer, second_publisher) = assembly(
        Arc::clone(&store),
        second_keys.clone(),
        vec![Arc::clone(&second_materializer)],
    );

    first
        .publish(intent(
            first_keys.public_key(),
            Kind::ContactList,
            EDIT_FORMAT,
        ))
        .expect("first publication owns the only slot");
    assert!(
        second
            .publish(intent(
                second_keys.public_key(),
                Kind::ContactList,
                EDIT_FORMAT,
            ))
            .is_err()
    );

    assert_eq!(first_materializer.calls().len(), 1);
    assert_eq!(second_materializer.calls().len(), 0);
    assert_no_effects(&store, &second_signer, &second_publisher, 1);
}
