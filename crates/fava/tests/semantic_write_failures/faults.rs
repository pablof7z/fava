use std::sync::atomic::{AtomicUsize, Ordering};

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges,
};
use fava_routing::RoutePlan;
use fava_state::{CacheMutation, CachedEvent, RelaySessionKey};
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, MaterializationId, Receipt, ReceiptId,
    RelayDeliveryOutcome, ReplaceableEventEdit, Timestamp, UnsignedEvent, WriteId, WriteIntent,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::EventId as NostrEventId;
use tokio::sync::broadcast;

fn controlled_open(
    source: &dyn QuerySource,
    query: &Query,
    closed: broadcast::Receiver<()>,
) -> Result<OpenedQuerySource, QuerySourceError> {
    let opened = source.open(query)?;
    Ok(OpenedQuerySource {
        initial: opened.initial,
        changes: Box::new(ControlledChanges {
            inner: opened.changes,
            closed,
        }),
    })
}

struct ControlledChanges {
    inner: Box<dyn SourceChanges>,
    closed: broadcast::Receiver<()>,
}

impl SourceChanges for ControlledChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            tokio::select! {
                biased;
                _ = self.closed.recv() => Err(QuerySourceClosed),
                changed = self.inner.next_change() => changed,
            }
        })
    }

    fn close(&mut self) {
        self.inner.close();
    }
}

pub(super) struct ClosingEventCache {
    inner: fava_event_cache_memory::MemoryEventCache,
    closed: broadcast::Sender<()>,
}

impl ClosingEventCache {
    pub(super) fn new() -> Self {
        let (closed, _) = broadcast::channel(4);
        Self {
            inner: fava_event_cache_memory::MemoryEventCache::default(),
            closed,
        }
    }

    pub(super) fn close_observations(&self) {
        let _ = self.closed.send(());
    }
}

impl QuerySource for ClosingEventCache {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        controlled_open(&self.inner, query, self.closed.subscribe())
    }
}

impl EventCache for ClosingEventCache {
    fn commit(&self, mutations: Vec<CacheMutation>) -> Result<(), EventCacheError> {
        self.inner.commit(mutations)
    }
    fn event(&self, id: NostrEventId) -> Result<Option<CachedEvent>, EventCacheError> {
        self.inner.event(id)
    }
    fn events(&self) -> Result<Vec<CachedEvent>, EventCacheError> {
        self.inner.events()
    }
    fn len(&self) -> Result<usize, EventCacheError> {
        self.inner.len()
    }
}

pub(super) struct FaultingWriteStore {
    inner: MemoryWriteStore,
    closed: broadcast::Sender<()>,
    failing_reads: AtomicUsize,
    reads_after_signature: AtomicUsize,
}

impl FaultingWriteStore {
    pub(super) fn new() -> Self {
        let (closed, _) = broadcast::channel(4);
        Self {
            inner: MemoryWriteStore::default(),
            closed,
            failing_reads: AtomicUsize::new(0),
            reads_after_signature: AtomicUsize::new(0),
        }
    }

    pub(super) fn close_observations(&self) {
        let _ = self.closed.send(());
    }

    pub(super) fn fail_receipt_reads(&self, count: usize) {
        self.failing_reads.store(count, Ordering::SeqCst);
    }

    pub(super) fn fail_receipt_reads_after_signature(&self, count: usize) {
        self.reads_after_signature.store(count, Ordering::SeqCst);
    }

    fn should_fail_read(&self) -> bool {
        self.failing_reads
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                count.checked_sub(1)
            })
            .is_ok()
    }
}

impl QuerySource for FaultingWriteStore {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        controlled_open(&self.inner, query, self.closed.subscribe())
    }
}

impl WriteStore for FaultingWriteStore {
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
        let installed =
            self.inner
                .install_signed(write_id, receipt_id, materialization_id, event_id, event);
        if installed.is_ok() {
            let failures = self.reads_after_signature.swap(0, Ordering::SeqCst);
            self.failing_reads.store(failures, Ordering::SeqCst);
        }
        installed
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
        if self.should_fail_read() {
            Err(WriteStoreError::Refused(
                "injected transient receipt read failure".to_owned(),
            ))
        } else {
            self.inner.receipt(receipt_id)
        }
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
