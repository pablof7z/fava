//! Bounded volatile write-store provider for tests and explicit ephemeral profiles.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceKind, SourceSnapshot,
};
use fava_relay::RelaySessionKey;
use fava_routing::RoutePlan;
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, RevisionId, PublicKey, PublicationEvidence,
    Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, EventEdit, SignatureState,
    Timestamp, UnsignedEvent, WriteId, WriteIntent, WritePayload,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use tokio::sync::{broadcast, watch};

mod lifecycle;
mod model;
mod recovery;
mod semantic;
mod semantic_acceptance;
mod semantic_composition;
mod semantic_reservation;
mod state;
#[cfg(test)]
mod tests;

use model::destinations;
use semantic::WriteState;
use state::{capacity_reached, next_identity, next_revision, release_semantic};

const RECEIPT_CHANGE_CAPACITY: usize = 256;

/// Bounded current-process write store.
pub struct MemoryWriteStore {
    capacity: NonZeroUsize,
    state: Mutex<WriteState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
}

impl Default for MemoryWriteStore {
    fn default() -> Self {
        Self::bounded(NonZeroUsize::new(10_000).expect("constant is non-zero"))
    }
}

impl MemoryWriteStore {
    /// Create an empty store with an exact maximum active-write count.
    #[must_use]
    pub fn bounded(capacity: NonZeroUsize) -> Self {
        let (latest, _) = watch::channel(Arc::new(SourceSnapshot::empty(SourceKind::WriteStore)));
        let (receipt_changes, _) = broadcast::channel(RECEIPT_CHANGE_CAPACITY);
        Self {
            capacity,
            state: Mutex::new(WriteState::default()),
            latest,
            receipt_changes,
        }
    }

    /// Take the ledger lock, refusing rather than panicking on a poisoned mutex.
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, WriteState>, WriteStoreError> {
        self.state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))
    }

    fn publish_receipt(&self, state: &WriteState, receipt: &Receipt) {
        self.publish_snapshot(state);
        self.publish_receipt_only(receipt);
    }

    fn publish_receipt_only(&self, receipt: &Receipt) {
        let _ = self
            .receipt_changes
            .send((receipt.receipt_id, Some(receipt.clone())));
    }
}

impl WriteStore for MemoryWriteStore {
    fn active_capacity(&self) -> usize {
        self.capacity.get()
    }

    fn reserve_active(
        &self,
        edit: &EventEdit,
        author: PublicKey,
    ) -> Result<u64, WriteStoreError> {
        self.reserve_active_slot(edit, author)
    }

    fn release_active(&self, reservation: u64) -> Result<(), WriteStoreError> {
        self.release_active_slot(reservation)
    }

    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.receipt_changes.subscribe()
    }

    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError> {
        let mut guard = self.lock_state()?;
        if capacity_reached(&guard, self.capacity.get()) {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }

        let identity = guard.next_identity;
        let next_identity = next_identity(identity)?;
        let write_id = WriteId::from_nonzero(identity);
        let receipt_id = ReceiptId::from_nonzero(identity);
        let (payload, routing) = intent.into_parts();
        let (event, signature) = match payload {
            WritePayload::Event(event) => (EventValue::Unsigned(event), SignatureState::Unsigned),
            WritePayload::Edit { .. } => {
                return Err(WriteStoreError::Refused(
                    "replaceable-event edit requires revision before acceptance".to_owned(),
                ));
            }
            WritePayload::Presigned(event) => (EventValue::Signed(event), SignatureState::Signed),
        };
        let destinations = destinations(&routing);
        let desired_destinations = destinations.keys().cloned().collect();
        let explicit = matches!(routing, fava_write::WriteRouting::Explicit(_));
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
            revision_id: fava_write::RevisionId::FIRST,
            revision_source: None,
            revision_failure: None,
            retired_revisions: Vec::new(),
            signature,
            destinations,
        };
        let current = LocalWriteEvent::new(event, publication)?;
        let receipt = Receipt {
            write_id,
            receipt_id,
            current: current.clone(),
            routing,
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: BTreeMap::new(),
        };

        let next_revision = next_revision(&guard)?;
        guard.next_identity = next_identity;
        guard.revision = next_revision;
        guard.writes.insert(receipt_id, receipt.clone());
        self.publish_receipt(&guard, &receipt);

        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current,
        })
    }

    fn accept_applied_edit(
        &self,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_semantic(intent, event, source)
    }

    fn accept_reserved_applied_edit(
        &self,
        reservation: u64,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_reserved_semantic(reservation, intent, event, source, initial_route)
    }

    fn install_revision(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: RevisionId,
        expected_source: Option<EventId>,
        applied_edits: &[EventEdit],
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<Receipt, WriteStoreError> {
        self.install_semantic(
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

    fn record_revision_failure(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: RevisionId,
        expected_source: Option<EventId>,
        source: Option<&EventValue>,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.record_semantic_failure(
            write_id,
            receipt_id,
            expected,
            expected_source,
            source,
            reason,
        )
    }

    #[allow(clippy::type_complexity)] // The neutral contract forbids a recovery wrapper.
    fn recover_applied_edits(
        &self,
    ) -> Result<
        Vec<(
            Receipt,
            Vec<EventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.recover_semantic()
    }

    #[allow(clippy::type_complexity)]
    fn applied_edits(
        &self,
        receipt_id: ReceiptId,
        expected: RevisionId,
    ) -> Result<
        Option<(
            Vec<EventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.semantic_custody(receipt_id, expected)
    }

    fn install_signed(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        event: Event,
    ) -> Result<Receipt, WriteStoreError> {
        self.install_signed_current(write_id, receipt_id, revision_id, event_id, event)
    }

    fn authorize_signing(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<Receipt, WriteStoreError> {
        self.authorize_signing_current(write_id, receipt_id, revision_id, event_id)
    }

    fn record_signer_retryable(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.record_signer_retryable_current(
            write_id,
            receipt_id,
            revision_id,
            event_id,
            reason,
        )
    }

    fn signing_successor(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<bool, WriteStoreError> {
        self.has_signing_successor(write_id, receipt_id, revision_id, event_id)
    }

    fn record_signer_refusal(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        self.record_signer_refusal_current(
            write_id,
            receipt_id,
            revision_id,
            event_id,
            reason,
        )
    }

    fn apply_route(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        self.apply_route_current(write_id, receipt_id, revision_id, event_id, plan)
    }

    fn begin_attempt(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
    ) -> Result<Receipt, WriteStoreError> {
        self.begin_attempt_current(
            write_id,
            receipt_id,
            revision_id,
            event_id,
            session,
            attempt,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_outcome(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
        outcome: RelayDeliveryOutcome,
    ) -> Result<Receipt, WriteStoreError> {
        self.record_outcome_current(
            write_id,
            receipt_id,
            revision_id,
            event_id,
            session,
            attempt,
            outcome,
        )
    }

    fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        let mut guard = self.lock_state()?;
        let next_revision = next_revision(&guard)?;
        let Some(receipt) = guard.writes.get_mut(&receipt_id) else {
            return Ok(None);
        };
        if matches!(receipt.outcome, ReceiptOutcome::Cancelled) {
            return Ok(Some(receipt.clone()));
        }
        if receipt.is_terminal()
            || receipt.destinations().values().any(|outcome| {
                !matches!(
                    outcome,
                    RelayDeliveryOutcome::Pending | RelayDeliveryOutcome::Retryable { .. }
                )
            })
        {
            return Err(WriteStoreError::Refused(
                "receipt can no longer be cancelled before handoff".to_owned(),
            ));
        }
        for outcome in receipt.current.publication.destinations.values_mut() {
            *outcome = RelayDeliveryOutcome::CancelledBeforeHandoff;
        }
        receipt.outcome = ReceiptOutcome::Cancelled;
        let current = receipt.clone();
        guard.revision = next_revision;
        release_semantic(&mut guard, receipt_id);
        self.publish_receipt(&guard, &current);
        Ok(Some(current))
    }

    fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        let guard = self.lock_state()?;
        Ok(guard.writes.get(&receipt_id).cloned())
    }

    fn recover_open(&self) -> Result<Vec<Receipt>, WriteStoreError> {
        let guard = self.lock_state()?;
        Ok(guard
            .writes
            .values()
            .filter(|receipt| !receipt.is_terminal())
            .cloned()
            .collect())
    }

    fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        let mut guard = self.lock_state()?;
        if guard
            .writes
            .get(&receipt_id)
            .is_some_and(|receipt| !receipt.is_terminal())
        {
            return Err(WriteStoreError::Refused(
                "active receipt cannot be removed".to_owned(),
            ));
        }
        if !guard.writes.contains_key(&receipt_id) {
            return Ok(false);
        }
        let next_revision = next_revision(&guard)?;
        release_semantic(&mut guard, receipt_id);
        guard.writes.remove(&receipt_id);
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self.receipt_changes.send((receipt_id, None));
        Ok(true)
    }

    fn len(&self) -> Result<usize, WriteStoreError> {
        let guard = self.lock_state()?;
        Ok(guard
            .writes
            .values()
            .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            .count())
    }
}

impl QuerySource for MemoryWriteStore {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        let receiver = self.latest.subscribe();
        let initial = receiver.borrow().as_ref().clone();
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(WatchChanges {
                receiver,
                closed: false,
            }),
        })
    }
}

struct WatchChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    closed: bool,
}

impl SourceChanges for WatchChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed {
                return Err(QuerySourceClosed::local_close());
            }
            if self.receiver.changed().await.is_err() {
                self.closed = true;
                return Err(QuerySourceClosed::provider_closed());
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
