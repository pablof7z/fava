use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Mutex};

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
use tokio::sync::{broadcast, watch};

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
                _ = self.closed.recv() => Err(QuerySourceClosed::provider_closed()),
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
    fn transact(
        &self,
        decide: &dyn Fn(&[CachedEvent]) -> Vec<CacheMutation>,
    ) -> Result<usize, EventCacheError> {
        self.inner.transact(decide)
    }
    fn len(&self) -> Result<usize, EventCacheError> {
        self.inner.len()
    }
}

pub(super) struct FaultingWriteStore {
    inner: MemoryWriteStore,
    closed: broadcast::Sender<()>,
    drop_receipt_changes: Arc<AtomicBool>,
    failing_reads: AtomicUsize,
    fail_materialized_reads: AtomicBool,
    materialized_read_failures: AtomicU64,
    materialized_read_barrier: watch::Sender<u64>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
    reads_after_route: AtomicUsize,
    reads_after_signature: AtomicUsize,
    receipt_barrier: Mutex<Option<Arc<Barrier>>>,
    route_barrier: Mutex<Option<Arc<Barrier>>>,
    route_commits: AtomicU64,
    fail_release: AtomicBool,
    fail_initial_route_accept: AtomicBool,
    refuse_routes: AtomicBool,
}

impl FaultingWriteStore {
    pub(super) fn new() -> Self {
        let (closed, _) = broadcast::channel(4);
        let inner = MemoryWriteStore::default();
        let mut inner_changes = inner.receipt_changes();
        let (receipt_changes, _) = broadcast::channel(64);
        let (materialized_read_barrier, _) = watch::channel(0);
        let forwarded_changes = receipt_changes.clone();
        let drop_receipt_changes = Arc::new(AtomicBool::new(false));
        let drop_changes = Arc::clone(&drop_receipt_changes);
        tokio::spawn(async move {
            while let Ok(change) = inner_changes.recv().await {
                if !drop_changes.load(Ordering::SeqCst) {
                    let _ = forwarded_changes.send(change);
                }
            }
        });
        Self {
            inner,
            closed,
            drop_receipt_changes,
            failing_reads: AtomicUsize::new(0),
            fail_materialized_reads: AtomicBool::new(false),
            materialized_read_failures: AtomicU64::new(0),
            materialized_read_barrier,
            receipt_changes,
            reads_after_route: AtomicUsize::new(0),
            reads_after_signature: AtomicUsize::new(0),
            receipt_barrier: Mutex::new(None),
            route_barrier: Mutex::new(None),
            route_commits: AtomicU64::new(0),
            fail_release: AtomicBool::new(false),
            fail_initial_route_accept: AtomicBool::new(false),
            refuse_routes: AtomicBool::new(false),
        }
    }

    pub(super) fn close_observations(&self) {
        let _ = self.closed.send(());
    }

    pub(super) fn fail_receipt_reads(&self, count: usize) {
        self.failing_reads.store(count, Ordering::SeqCst);
    }

    pub(super) fn remaining_receipt_read_failures(&self) -> usize {
        self.failing_reads.load(Ordering::SeqCst)
    }

    pub(super) fn fail_materialized_reads(&self, fail: bool) {
        self.fail_materialized_reads.store(fail, Ordering::SeqCst);
    }

    pub(super) fn materialized_read_failures(&self) -> u64 {
        self.materialized_read_failures.load(Ordering::SeqCst)
    }

    pub(super) fn materialized_read_barrier(&self) -> watch::Receiver<u64> {
        self.materialized_read_barrier.subscribe()
    }

    pub(super) fn fail_receipt_reads_after_signature(&self, count: usize) {
        self.reads_after_signature.store(count, Ordering::SeqCst);
    }

    pub(super) fn fail_receipt_reads_after_route(&self, count: usize) {
        self.reads_after_route.store(count, Ordering::SeqCst);
    }

    pub(super) fn drop_receipt_changes(&self) {
        self.drop_receipt_changes.store(true, Ordering::SeqCst);
    }

    pub(super) fn pause_after_next_route(&self, barrier: Arc<Barrier>) {
        *self.route_barrier.lock().unwrap() = Some(barrier);
    }

    pub(super) fn pause_after_next_receipt_read(&self, barrier: Arc<Barrier>) {
        *self.receipt_barrier.lock().unwrap() = Some(barrier);
    }

    pub(super) fn route_commits(&self) -> u64 {
        self.route_commits.load(Ordering::SeqCst)
    }

    pub(super) fn fail_reservation_release(&self, fail: bool) {
        self.fail_release.store(fail, Ordering::SeqCst);
    }

    pub(super) fn fail_initial_route_acceptance(&self, fail: bool) {
        self.fail_initial_route_accept.store(fail, Ordering::SeqCst);
    }

    pub(super) fn refuse_routes(&self, refuse: bool) {
        self.refuse_routes.store(refuse, Ordering::SeqCst);
    }

    fn should_fail_read(&self) -> bool {
        self.failing_reads
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                count.checked_sub(1)
            })
            .is_ok()
    }

    fn after_route_commit(&self) {
        let failures = self.reads_after_route.swap(0, Ordering::SeqCst);
        self.failing_reads.store(failures, Ordering::SeqCst);
        self.route_commits.fetch_add(1, Ordering::SeqCst);
        let barrier = self.route_barrier.lock().unwrap().take();
        if let Some(barrier) = barrier {
            barrier.wait();
            barrier.wait();
        }
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
    fn reserve_active(
        &self,
        edit: &ReplaceableEventEdit,
        author: fava::PublicKey,
    ) -> Result<u64, WriteStoreError> {
        self.inner.reserve_active(edit, author)
    }
    fn release_active(&self, reservation: u64) -> Result<(), WriteStoreError> {
        self.inner.release_active(reservation)?;
        if self.fail_release.load(Ordering::SeqCst) {
            Err(WriteStoreError::Refused(
                "injected reservation release failure".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.receipt_changes.subscribe()
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
        if initial_route.is_some() && self.fail_initial_route_accept.load(Ordering::SeqCst) {
            self.inner.release_active(reservation)?;
            return Err(WriteStoreError::Refused(
                "injected atomic initial-route acceptance failure".to_owned(),
            ));
        }
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
        let installed = self.inner.install_materialization(
            write_id,
            receipt_id,
            expected,
            expected_source,
            applied_edits,
            event,
            source,
            initial_route,
        );
        if installed.is_ok() && initial_route.is_some() {
            self.after_route_commit();
        }
        installed
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
            fava::PublicKey,
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
            fava::PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        if self.fail_materialized_reads.load(Ordering::SeqCst) {
            let failures = self
                .materialized_read_failures
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1);
            self.materialized_read_barrier.send_replace(failures);
            return Err(WriteStoreError::Refused(
                "injected durable semantic custody read failure".to_owned(),
            ));
        }
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
        let installed =
            self.inner
                .install_signed(write_id, receipt_id, materialization_id, event_id, event);
        if installed.is_ok() {
            let failures = self.reads_after_signature.swap(0, Ordering::SeqCst);
            self.failing_reads.store(failures, Ordering::SeqCst);
        }
        installed
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
        if self.refuse_routes.load(Ordering::SeqCst) {
            return Err(WriteStoreError::Refused(
                "injected exact-generation route refusal".to_owned(),
            ));
        }
        let applied =
            self.inner
                .apply_route(write_id, receipt_id, materialization_id, event_id, plan);
        if applied.is_ok() {
            self.after_route_commit();
        }
        applied
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
            let receipt = self.inner.receipt(receipt_id);
            let barrier = self.receipt_barrier.lock().unwrap().take();
            if let Some(barrier) = barrier {
                barrier.wait();
                barrier.wait();
            }
            receipt
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
