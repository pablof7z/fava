//! Private write-store witness for acknowledged signing completion processing.

use std::sync::Arc;

use fava_query::{OpenedQuerySource, Query, QuerySource, QuerySourceError};
use fava_routing::RoutePlan;
use fava_state::RelaySessionKey;
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, MaterializationId, Receipt, ReceiptId,
    RelayDeliveryOutcome, ReplaceableEventEdit, Timestamp, UnsignedEvent, WriteId, WriteIntent,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use fava_write_store_memory::MemoryWriteStore;
use tokio::sync::broadcast;

const COMPLETION_ACK_CAPACITY: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompletionAck {
    pub(super) write_id: WriteId,
    pub(super) receipt_id: ReceiptId,
    pub(super) materialization_id: MaterializationId,
    pub(super) event_id: EventId,
    pub(super) installed: bool,
}

pub(super) struct CompletionStore {
    inner: MemoryWriteStore,
    completions: broadcast::Sender<CompletionAck>,
}

impl CompletionStore {
    pub(super) fn new() -> (Arc<Self>, broadcast::Receiver<CompletionAck>) {
        let (completions, receiver) = broadcast::channel(COMPLETION_ACK_CAPACITY);
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
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.inner
            .accept_reserved_materialized_edit(reservation, intent, event, source, initial_route)
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
            fava::PublicKey,
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
