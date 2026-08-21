use super::*;

#[test]
fn memory_exact_current_guard_precedes_idempotence() {
    let keys = Keys::generate();
    let store = MemoryWriteStore::default();
    let base = source(&keys, 10, "base");
    let accepted = accept(
        &store,
        edit(keys.public_key()),
        materialization(keys.public_key(), 11, "generation one"),
        Some(&base),
    );
    let successor_source = source(&keys, 20, "successor source");
    let successor_event = materialization(keys.public_key(), 21, "generation two");
    let successor = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(1),
            Some(base.id),
            successor_event.clone(),
            Some(&successor_source),
        )
        .expect("successor installs");
    let mut changes = store.receipt_changes();

    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(1),
                Some(successor_source.id),
                successor_event.clone(),
                Some(&successor_source),
            )
            .is_err(),
        "an identical body cannot bypass a stale generation"
    );
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(2),
                Some(base.id),
                successor_event.clone(),
                Some(&successor_source),
            )
            .is_err(),
        "an identical body cannot bypass a stale source identity"
    );
    assert_eq!(
        store.receipt(accepted.receipt_id).unwrap(),
        Some(successor.clone())
    );
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    let replay = store
        .install_materialization(
            accepted.write_id,
            accepted.receipt_id,
            MaterializationId::from_u64(2),
            Some(successor_source.id),
            successor_event.clone(),
            Some(&successor_source),
        )
        .expect("exact current replay remains idempotent");
    assert_eq!(replay, successor);
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    store.cancel(accepted.receipt_id).expect("receipt cancels");
    let mut changes = store.receipt_changes();
    assert!(
        store
            .install_materialization(
                accepted.write_id,
                accepted.receipt_id,
                MaterializationId::from_u64(2),
                Some(successor_source.id),
                successor_event,
                Some(&successor_source),
            )
            .is_err(),
        "terminal custody cannot report idempotent success"
    );
    assert!(matches!(
        changes.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}
