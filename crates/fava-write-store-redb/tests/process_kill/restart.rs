use std::sync::Arc;

use super::*;

fn restart_builder(
    cache: Arc<MemoryEventCache>,
    store: Arc<RedbWriteStore>,
    materializer: Arc<TestMaterializer>,
) -> FavaBuilder {
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .publisher(Arc::new(PendingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .materializer(materializer)
}

fn content(receipt: &Receipt) -> &str {
    match &receipt.current.event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

fn publish_change(fava: &Fava, change: u8) -> fava::Write {
    fava.by(keys().public_key())
        .to([relay()])
        .unwrap()
        .publish(ReplaceableEventEdit::new(Kind::ContactList, None, vec![change]).unwrap())
        .expect("immediate same-coordinate edit accepts after recovery reconciliation")
}

async fn assert_restart_then_immediate_edit(
    path: PathBuf,
    persisted_edits: u64,
    immediate_change: u8,
) {
    let store = Arc::new(RedbWriteStore::open(path).expect("semantic store reopens"));
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            signed_source(20, "restart source"),
            relay_evidence(),
        ))])
        .unwrap();
    let materializer = Arc::new(TestMaterializer::new(Kind::ContactList));
    let fava = restart_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&materializer),
    )
    .build()
    .expect("recovery reconciles before exposing the redb facade");

    let reconciled = fava.receipt(ReceiptId::from_u64(1)).unwrap().unwrap();
    assert_eq!(
        reconciled.current.publication.materialization_id,
        MaterializationId::from_u64(persisted_edits + 1)
    );
    let expected_reconciled = (1..=persisted_edits)
        .fold("restart source".to_owned(), |body, change| {
            format!("{body}|{change}")
        });
    assert_eq!(content(&reconciled), expected_reconciled);

    // No await separates build from admission on this current-thread runtime.
    // The recovery task cannot have initialized, so the returned facade is the
    // deterministic admission barrier and must already expose reconciled state.
    let immediate = publish_change(&fava, immediate_change);
    assert_eq!(immediate.write_id().as_u64(), 1);
    assert_eq!(immediate.receipt_id().as_u64(), 1);
    assert_eq!(
        immediate
            .receipt()
            .unwrap()
            .current
            .publication
            .materialization_id,
        MaterializationId::from_u64(persisted_edits + 2)
    );

    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            signed_source(40, "late source"),
            relay_evidence(),
        ))])
        .unwrap();
    let replayed = wait_for_generation(&fava, ReceiptId::from_u64(1), persisted_edits + 3).await;
    let expected_late = (1..=persisted_edits)
        .chain(std::iter::once(u64::from(immediate_change)))
        .fold("late source".to_owned(), |body, change| {
            format!("{body}|{change}")
        });
    assert_eq!(content(&replayed), expected_late);
}

#[tokio::test(flavor = "current_thread")]
async fn redb_restart_reconciles_before_immediate_edit_and_late_source() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let root = unique_root("semantic-clean-restart-admission");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("writes.redb");
    let store = RedbWriteStore::open(&path).unwrap();
    store
        .accept_materialized_edit(edit_intent(), materialization(1, "1"), None)
        .unwrap();
    drop(store);

    assert_restart_then_immediate_edit(path, 1, 2).await;
}

#[tokio::test(flavor = "current_thread")]
async fn sigkill_restart_reconciles_before_immediate_edit_and_late_source() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    assert_restart_then_immediate_edit(kill_at("composed"), 2, 3).await;
}
