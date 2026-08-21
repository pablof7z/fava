use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Barrier, Mutex};

use fava::{
    Event, EventValue, Kind, MaterializationId, PublicKey, Receipt, ReceiptId, ReceiptOutcome,
    RelayDeliveryOutcome, ReplaceableEventEdit, ReplaceableEventMaterializer, Timestamp,
    UnsignedEvent, WriteId, WriteIntent, WriteIntentError,
};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{OpenedQuerySource, Query, QuerySource, QuerySourceError};
use fava_routing::RoutePlan;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_state::{CacheMutation, CachedEvent, RelaySessionKey};
use fava_write::{EventId, LocalWriteEvent};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use super::capability_protocol::assert_source_removal;
use super::explicit_intent;
use super::support::{
    BlockingSigner, RecordingPublisher, publication_builder, relay_evidence,
    wait_for_materialization, wait_for_signer,
};

type EditResult = Result<ReplaceableEventEdit, WriteIntentError>;
type PendingSignature = (UnsignedEvent, oneshot::Sender<Result<Event, SignerError>>);

pub async fn exercise<Add, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    adjacent: Adjacent,
    target: (&str, &str),
) where
    Add: Fn(PublicKey) -> EditResult,
    Adjacent: Fn(PublicKey) -> EditResult,
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
    Add: Fn(PublicKey) -> EditResult,
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
            CacheMutation::Upsert(CachedEvent::new(older, relay_evidence())),
            CacheMutation::Upsert(CachedEvent::new(current.clone(), relay_evidence())),
        ])
        .unwrap();
    let store = Arc::new(MemoryWriteStore::default());
    let signer = Arc::new(BlockingSigner::new(actor));
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
    let accepted = fava.publish(explicit_intent(add(actor).unwrap())).unwrap();
    wait_for_signer(&signer, 1).await;
    cache
        .commit(vec![CacheMutation::Retract(current.id)])
        .unwrap();
    let removed = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    assert_source_removal(&accepted, &removed, current.id, kind, actor, target);
    assert!(publisher.attempts().is_empty());
}

async fn prove_processed_stale_success<Add, Adjacent>(
    kind: Kind,
    materializer: Arc<dyn ReplaceableEventMaterializer>,
    add: Add,
    adjacent: Adjacent,
) where
    Add: Fn(PublicKey) -> EditResult,
    Adjacent: Fn(PublicKey) -> EditResult,
{
    let keys = Keys::generate();
    let actor = keys.public_key();
    let initial = materializer
        .materialize(&add(actor).unwrap(), None, Timestamp::from(u64::MAX - 100))
        .unwrap()
        .finalize(&keys)
        .unwrap();
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .admit(
            CachedEvent::new(initial.clone(), relay_evidence()),
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
    let accepted = fava.publish(explicit_intent(add(actor).unwrap())).unwrap();
    let (first, first_completion) = signatures.recv().await.unwrap();
    let successor = materializer
        .materialize(
            &adjacent(actor).unwrap(),
            Some(&initial),
            Timestamp::from(u64::MAX - 50),
        )
        .unwrap()
        .finalize(&keys)
        .unwrap();
    admit_twice(&cache, &successor);
    let current = wait_for_materialization(&fava, accepted.receipt_id, 2).await;
    let (second, second_completion) = signatures.recv().await.unwrap();
    let first = first.finalize(&keys).unwrap();
    let first_id = first.id;
    first_completion.send(Ok(first)).unwrap();
    assert_completion(
        &completions.recv().await.unwrap(),
        &accepted,
        MaterializationId::from_u64(1),
        first_id,
        false,
    );
    let after_stale = fava.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(after_stale, current);
    assert_eq!(
        after_stale.current.publication.materialization_source,
        Some(successor.id)
    );
    assert!(matches!(after_stale.current.event, EventValue::Unsigned(_)));
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
    let terminal = fava.wait_terminal(accepted.receipt_id).await.unwrap();
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.current.event.kind(), kind);
    assert_eq!(terminal.write_id, accepted.write_id);
    assert_eq!(terminal.receipt_id, accepted.receipt_id);
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
                        CachedEvent::new(successor, relay_evidence()),
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

struct GatedSigner {
    public_key: PublicKey,
    pending: mpsc::Sender<PendingSignature>,
}

impl GatedSigner {
    fn new(public_key: PublicKey) -> (Self, mpsc::Receiver<PendingSignature>) {
        let (pending, requests) = mpsc::channel(2);
        (
            Self {
                public_key,
                pending,
            },
            requests,
        )
    }
}

impl Signer for GatedSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }
    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }
    fn sign_event(
        &self,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        Box::pin(async move {
            let (complete, response) = oneshot::channel();
            self.pending
                .send((event, complete))
                .await
                .map_err(|error| {
                    SignerError::Unavailable(format!("signature gate closed: {error}"))
                })?;
            response
                .await
                .map_err(|_| SignerError::Unavailable("completion dropped".to_owned()))?
        })
    }
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
    accepted: &AcceptedWrite,
    materialization_id: MaterializationId,
    event_id: EventId,
    installed: bool,
) {
    assert_eq!(ack.write_id, accepted.write_id);
    assert_eq!(ack.receipt_id, accepted.receipt_id);
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
    fn reserve_active(&self) -> Result<u64, WriteStoreError> {
        self.inner.reserve_active()
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
        source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner.accept_materialized_edit(intent, event, source)
    }
    fn accept_reserved_materialized_edit(
        &self,
        reservation: u64,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner
            .accept_reserved_materialized_edit(reservation, intent, event, source)
    }
    fn install_materialization(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<Receipt, WriteStoreError> {
        self.inner.install_materialization(
            write_id,
            receipt_id,
            expected,
            expected_source,
            event,
            source,
        )
    }
    fn record_materialization_failure(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        source: Option<&Event>,
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
            ReplaceableEventEdit,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.inner.recover_materialized_edits()
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
