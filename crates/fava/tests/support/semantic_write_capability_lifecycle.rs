use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use fava::{
    Event, EventValue, Kind, MaterializationId, PublicKey, Receipt, ReceiptId, ReceiptOutcome,
    RelayDeliveryOutcome, ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp,
    UnsignedEvent, Write, WriteId, WriteIntentError, all,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{OpenedQuerySource, Query, QuerySource, QuerySourceError};
use fava_relay::RelaySessionKey;
use fava_routing::RoutePlan;
use fava_state::{EventStateMutation, RetractionCause};
use fava_write::{EventId, LocalWriteEvent, WriteIntent};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use tokio::sync::broadcast;

use super::capability_protocol::assert_source_removal;
use super::capability_signer::GatedSigner;
use super::support::{
    RecordingPublisher, UnavailableSigner, publication_builder, relay_url,
    relay_event, relay_occurrence, relay_session, wait_for_materialization,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;

pub async fn exercise<Add, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    adjacent: Adjacent,
    target: (&str, &str),
) where
    Add: Fn() -> EditResult,
    Adjacent: Fn() -> EditResult,
{
    prove_source_removal(kind, Arc::clone(&materializer), &add, target).await;
    prove_processed_stale_success(kind, materializer, add, adjacent).await;
}

async fn prove_source_removal<Add>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: &Add,
    target: (&str, &str),
) where
    Add: Fn() -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let older = NostrEventBuilder::new(kind, "older")
        .custom_created_at(Timestamp::from(5))
        .finalize(&keys)
        .unwrap();
    let current = NostrEventBuilder::new(kind, "opaque")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![
            EventStateMutation::Upsert(relay_event(older, relay_occurrence())),
            EventStateMutation::Upsert(relay_event(current.clone(), relay_occurrence())),
        ])
        .unwrap();
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(UnavailableSigner::new(actor));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        store,
        Arc::clone(&signer),
        Arc::clone(&publisher),
    )
    .materializers([materializer])
    .build()
    .unwrap();
    let accepted = publish_edit(&fava, add().unwrap(), actor);
    let accepted_receipt = accepted
        .receipt()
        .expect("accepted receipt remains readable");
    cache
        .commit(vec![EventStateMutation::Retract {
            event_id: current.id,
            session: relay_session(),
            cause: RetractionCause::Evicted,
        }])
        .unwrap();
    let removed = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;
    assert_source_removal(
        &accepted,
        &accepted_receipt,
        &removed,
        current.id,
        kind,
        actor,
        target,
    );
    assert!(publisher.attempts().is_empty());
}

#[allow(clippy::too_many_lines)]
async fn prove_processed_stale_success<Add, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    adjacent: Adjacent,
) where
    Add: Fn() -> EditResult,
    Adjacent: Fn() -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let initial = materializer
        .materialize(
            &add().unwrap(),
            actor,
            None,
            Timestamp::from(u64::MAX - 100),
        )
        .unwrap()
        .finalize(&keys)
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            relay_event(initial.clone(), relay_occurrence()),
            Timestamp::from(1),
        )
        .unwrap();
    let (store, mut completions) = CompletionStore::new();
    let (signer, mut signatures) = GatedSigner::new(actor);
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(signer),
        Arc::clone(&publisher),
    )
    .materializers([Arc::clone(&materializer)])
    .build()
    .unwrap();
    let accepted = publish_edit(&fava, add().unwrap(), actor);
    let (first, first_completion) = signatures.recv().await.unwrap();
    let successor = materializer
        .materialize(
            &adjacent().unwrap(),
            actor,
            Some(&EventValue::Signed(initial.clone())),
            Timestamp::from(u64::MAX - 50),
        )
        .unwrap()
        .finalize(&keys)
        .unwrap();
    admit_twice(&cache, &successor);
    let authorized = accepted.receipt().unwrap();
    assert_eq!(
        authorized.current.publication.materialization_id,
        MaterializationId::from_u64(1),
        "source-driven successor superseded an authorized generation"
    );
    let first = first.finalize(&keys).unwrap();
    let first_id = first.id;
    first_completion.send(Ok(first)).unwrap();
    assert_completion(
        &completions.recv().await.unwrap(),
        &accepted,
        MaterializationId::from_u64(1),
        first_id,
        true,
    );
    let current = wait_for_materialization(&fava, accepted.receipt_id(), 2).await;
    let (second, second_completion) = signatures.recv().await.unwrap();
    let after_predecessor = accepted.receipt().unwrap();
    assert_eq!(after_predecessor.current.id(), current.current.id());
    assert_eq!(
        after_predecessor.current.publication.materialization_source,
        Some(successor.id)
    );
    assert!(matches!(
        after_predecessor.current.event,
        EventValue::Unsigned(_)
    ));
    assert!(publisher.attempts().is_empty());
    let second = second.finalize(&keys).unwrap();
    let second_id = second.id;
    second_completion.send(Ok(second)).unwrap();
    assert_completion(
        &completions.recv().await.unwrap(),
        &accepted,
        MaterializationId::from_u64(2),
        second_id,
        true,
    );
    let terminal = tokio::time::timeout(Duration::from_secs(1), accepted.settled(all()))
        .await
        .expect("terminal receipt wait is bounded")
        .unwrap();
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.current.event.kind(), kind);
    assert_eq!(terminal.write_id, accepted.write_id());
    assert_eq!(terminal.receipt_id, accepted.receipt_id());
    let attempts = publisher.attempts();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].materialization_id,
        MaterializationId::from_u64(2)
    );
    assert_eq!(attempts[0].event.id, second_id);
    assert!(terminal.destinations().values().all(|outcome| {
        *outcome
            == RelayDeliveryOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
    }));
}

fn publish_edit(fava: &fava::Fava, edit: ReplaceableEventEdit, actor: PublicKey) -> Write {
    fava.by(actor)
        .to([relay_url()])
        .expect("route validates")
        .publish(edit)
        .expect("semantic edit accepts")
}

fn admit_twice(cache: &Arc<MemoryEventCache>, successor: &Event) {
    let barrier = Arc::new(Barrier::new(3));
    let admissions = Arc::new(Mutex::new(0));
    std::thread::scope(|scope| {
        for _ in 0..2 {
            let (cache, successor, barrier, admissions) = (
                Arc::clone(cache),
                successor.clone(),
                Arc::clone(&barrier),
                Arc::clone(&admissions),
            );
            scope.spawn(move || {
                barrier.wait();
                cache
                    .admit(
                        relay_event(successor, relay_occurrence()),
                        Timestamp::from(2),
                    )
                    .unwrap();
                *admissions.lock().unwrap() += 1;
            });
        }
        barrier.wait();
    });
    assert_eq!(*admissions.lock().unwrap(), 2);
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompletionAck {
    write_id: WriteId,
    receipt_id: ReceiptId,
    materialization_id: MaterializationId,
    event_id: EventId,
    installed: bool,
}

fn assert_completion(
    ack: &CompletionAck,
    accepted: &Write,
    materialization_id: MaterializationId,
    event_id: EventId,
    installed: bool,
) {
    assert_eq!(ack.write_id, accepted.write_id());
    assert_eq!(ack.receipt_id, accepted.receipt_id());
    assert_eq!(ack.materialization_id, materialization_id);
    assert_eq!(ack.event_id, event_id);
    assert_eq!(ack.installed, installed);
}

struct CompletionStore {
    inner: MemoryWriteStore,
    completions: broadcast::Sender<CompletionAck>,
}

impl CompletionStore {
    fn new() -> (Arc<Self>, broadcast::Receiver<CompletionAck>) {
        let (completions, receiver) = broadcast::channel(4);
        (
            Arc::new(Self {
                inner: MemoryWriteStore::default(),
                completions,
            }),
            receiver,
        )
    }
}

impl QuerySource for CompletionStore {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        self.inner.open(query)
    }
}

impl WriteStore for CompletionStore {
    fn active_capacity(&self) -> usize {
        self.inner.active_capacity()
    }
    fn reserve_active(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
    ) -> Result<u64, WriteStoreError> {
        self.inner.reserve_active(edit, author)
    }
    fn release_active(&self, reservation: u64) -> Result<(), WriteStoreError> {
        self.inner.release_active(reservation)
    }
    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.inner.receipt_changes()
    }
    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner.accept(intent)
    }
    fn accept_materialized_edit(
        &self,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner.accept_materialized_edit(intent, event, source)
    }
    fn accept_reserved_materialized_edit(
        &self,
        reservation: u64,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner.accept_reserved_materialized_edit(
            reservation,
            intent,
            event,
            source,
            initial_route,
        )
    }
    fn install_materialization(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        applied_edits: &[ReplaceableEventEdit],
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.install_materialization(
            write_id,
            receipt_id,
            expected,
            expected_source,
            applied_edits,
            event,
            source,
            initial_route,
        )
    }
    fn record_materialization_failure(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        source: Option<&EventValue>,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.record_materialization_failure(
            write_id,
            receipt_id,
            expected,
            expected_source,
            source,
            reason,
        )
    }
    #[allow(clippy::type_complexity)]
    fn recover_materialized_edits(
        &self,
    ) -> Result<
        Vec<(
            Receipt,
            Vec<ReplaceableEventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.inner.recover_materialized_edits()
    }
    #[allow(clippy::type_complexity)]
    fn materialized_edits(
        &self,
        receipt_id: ReceiptId,
        expected: MaterializationId,
    ) -> Result<
        Option<(
            Vec<ReplaceableEventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.inner.materialized_edits(receipt_id, expected)
    }
    fn install_signed(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        event: Event,
    ) -> Result<Receipt, WriteStoreError> {
        let result =
            self.inner
                .install_signed(write_id, receipt_id, materialization_id, event_id, event);
        let _ = self.completions.send(CompletionAck {
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            installed: result.is_ok(),
        });
        result
    }
    fn authorize_signing(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner
            .authorize_signing(write_id, receipt_id, materialization_id, event_id)
    }
    fn record_signer_retryable(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.record_signer_retryable(
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            reason,
        )
    }
    fn signing_successor(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
    ) -> Result<bool, WriteStoreError> {
        self.inner
            .signing_successor(write_id, receipt_id, materialization_id, event_id)
    }
    fn record_signer_refusal(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner
            .record_signer_refusal(write_id, receipt_id, materialization_id, event_id, reason)
    }
    fn apply_route(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner
            .apply_route(write_id, receipt_id, materialization_id, event_id, plan)
    }
    fn begin_attempt(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.begin_attempt(
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            session,
            attempt,
        )
    }
    fn record_outcome(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
        outcome: RelayDeliveryOutcome,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.record_outcome(
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            session,
            attempt,
            outcome,
        )
    }
    fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        self.inner.cancel(receipt_id)
    }
    fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        self.inner.receipt(receipt_id)
    }
    fn recover_open(&self) -> Result<Vec<Receipt>, WriteStoreError> {
        self.inner.recover_open()
    }
    fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        self.inner.remove_receipt(receipt_id)
    }
    fn receipt_event(
        &self,
        receipt_id: ReceiptId,
    ) -> Result<Option<LocalWriteEvent>, WriteStoreError> {
        self.inner.receipt_event(receipt_id)
    }
    fn len(&self) -> Result<usize, WriteStoreError> {
        self.inner.len()
    }
    fn accept_materialized(&self, event: EventValue) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner.accept_materialized(event)
    }
}
