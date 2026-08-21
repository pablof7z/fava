//! Bounded volatile write-store provider for tests and explicit ephemeral profiles.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QuerySource, QuerySourceClosed, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_routing::RoutePlan;
use fava_state::RelaySessionKey;
use fava_write::{
    Event, EventValue, LocalWriteEvent, PublicationEvidence, Receipt, ReceiptId, ReceiptOutcome,
    RelayDeliveryOutcome, SignatureState, WriteId, WriteIntent, WritePayload,
};
use fava_write_store::{
    AcceptedWrite, WriteStore, WriteStoreError, apply_route_to_receipt, validate_delivery_outcome,
    validate_receipt_text,
};
use tokio::sync::{broadcast, watch};

mod model;

use model::{UnsignedEventView, destinations, settle};

const RECEIPT_CHANGE_CAPACITY: usize = 256;

/// Bounded current-process write store.
pub struct MemoryWriteStore {
    capacity: NonZeroUsize,
    state: Mutex<WriteState>,
    latest: watch::Sender<Arc<SourceSnapshot>>,
    receipt_changes: broadcast::Sender<(ReceiptId, Option<Receipt>)>,
}

#[derive(Clone, Debug)]
struct WriteState {
    revision: u64,
    next_identity: u64,
    writes: BTreeMap<ReceiptId, Receipt>,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            revision: 0,
            next_identity: 1,
            writes: BTreeMap::new(),
        }
    }
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

    fn snapshot(state: &WriteState) -> SourceSnapshot {
        SourceSnapshot {
            kind: SourceKind::WriteStore,
            revision: SourceRevision(state.revision),
            status: SourceStatus::Open,
            events: state
                .writes
                .values()
                .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
                .map(|receipt| SourceEvent::Local(receipt.current.clone()))
                .collect(),
        }
    }

    fn publish_snapshot(&self, state: &WriteState) {
        self.latest.send_replace(Arc::new(Self::snapshot(state)));
    }
}

impl WriteStore for MemoryWriteStore {
    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.receipt_changes.subscribe()
    }

    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        if guard.writes.len() == self.capacity.get() {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }

        let identity = guard.next_identity;
        let next_identity = identity
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("write identity exhausted".to_owned()))?;
        let write_id = WriteId::from_u64(identity);
        let receipt_id = ReceiptId::from_u64(identity);
        let (payload, routing) = intent.into_parts();
        let (event, signature) = match payload {
            WritePayload::Event(event) => (EventValue::Unsigned(event), SignatureState::Unsigned),
            WritePayload::Presigned(event) => (EventValue::Signed(event), SignatureState::Signed),
        };
        let destinations = destinations(&routing);
        let desired_destinations = destinations.keys().cloned().collect();
        let explicit = matches!(routing, fava_write::WriteRouting::Explicit(_));
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
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
        self.publish_snapshot(&guard);
        let _ = self.receipt_changes.send((receipt_id, Some(receipt)));

        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current,
        })
    }

    fn install_signed(
        &self,
        receipt_id: ReceiptId,
        event: Event,
    ) -> Result<Receipt, WriteStoreError> {
        event
            .verify()
            .map_err(|error| WriteStoreError::Refused(error.to_string()))?;
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let next_revision = next_revision(&guard)?;
        let receipt = guard
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        if receipt.is_terminal() {
            return Err(WriteStoreError::Refused("receipt is terminal".to_owned()));
        }
        let EventValue::Unsigned(unsigned) = &receipt.current.event else {
            return Err(WriteStoreError::Refused(
                "event is already signed".to_owned(),
            ));
        };
        if UnsignedEventView::from(unsigned) != UnsignedEventView::from(&event) {
            return Err(WriteStoreError::Refused(
                "signature does not match current unsigned event".to_owned(),
            ));
        }
        receipt.current.event = EventValue::Signed(event);
        receipt.current.publication.signature = SignatureState::Signed;
        let current = receipt.clone();
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(current.clone())));
        Ok(current)
    }

    fn record_signer_refusal(
        &self,
        receipt_id: ReceiptId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        validate_receipt_text(&reason)?;
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let next_revision = next_revision(&guard)?;
        let receipt = guard
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        if receipt.is_terminal() || !matches!(receipt.current.event, EventValue::Unsigned(_)) {
            return Err(WriteStoreError::Refused(
                "signer refusal is not current".to_owned(),
            ));
        }
        receipt.current.publication.signature = SignatureState::Refused(reason);
        let current = receipt.clone();
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(current.clone())));
        Ok(current)
    }

    fn apply_route(
        &self,
        receipt_id: ReceiptId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let next_revision = next_revision(&guard)?;
        let receipt = guard
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        if receipt.is_terminal() {
            return Err(WriteStoreError::Refused("receipt is terminal".to_owned()));
        }
        apply_route_to_receipt(receipt, plan)?;
        let updated = receipt.clone();
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(updated.clone())));
        Ok(updated)
    }

    fn begin_attempt(
        &self,
        receipt_id: ReceiptId,
        session: &RelaySessionKey,
    ) -> Result<Receipt, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let next_revision = next_revision(&guard)?;
        let receipt = guard
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        if !matches!(receipt.current.event, EventValue::Signed(_)) {
            return Err(WriteStoreError::Refused("event is not signed".to_owned()));
        }
        let outcome = receipt
            .current
            .publication
            .destinations
            .get_mut(session)
            .ok_or_else(|| WriteStoreError::Refused("destination does not exist".to_owned()))?;
        if !matches!(
            outcome,
            RelayDeliveryOutcome::Pending | RelayDeliveryOutcome::Retryable { .. }
        ) {
            return Err(WriteStoreError::Refused(
                "destination is not pending".to_owned(),
            ));
        }
        *outcome = RelayDeliveryOutcome::Attempting;
        let attempts = receipt.attempts.entry(session.clone()).or_default();
        *attempts = attempts
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("attempt count exhausted".to_owned()))?;
        let current = receipt.clone();
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(current.clone())));
        Ok(current)
    }

    fn record_outcome(
        &self,
        receipt_id: ReceiptId,
        session: &RelaySessionKey,
        outcome: RelayDeliveryOutcome,
    ) -> Result<Receipt, WriteStoreError> {
        validate_delivery_outcome(&outcome)?;
        if !outcome.is_terminal() && !matches!(outcome, RelayDeliveryOutcome::Retryable { .. }) {
            return Err(WriteStoreError::Refused(
                "recorded delivery outcome is not terminal".to_owned(),
            ));
        }
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let next_revision = next_revision(&guard)?;
        let receipt = guard
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let current = receipt
            .current
            .publication
            .destinations
            .get_mut(session)
            .ok_or_else(|| WriteStoreError::Refused("destination does not exist".to_owned()))?;
        let may_transition = matches!(current, RelayDeliveryOutcome::Attempting)
            || (matches!(current, RelayDeliveryOutcome::Retryable { .. })
                && matches!(outcome, RelayDeliveryOutcome::GivenUp { .. }));
        if !may_transition {
            return Err(WriteStoreError::Refused(
                "attempt is not current".to_owned(),
            ));
        }
        *current = outcome;
        settle(receipt);
        let current = receipt.clone();
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(current.clone())));
        Ok(current)
    }

    fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
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
        self.publish_snapshot(&guard);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(current.clone())));
        Ok(Some(current))
    }

    fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(guard.writes.get(&receipt_id).cloned())
    }

    fn recover_open(&self) -> Result<Vec<Receipt>, WriteStoreError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(guard
            .writes
            .values()
            .filter(|receipt| !receipt.is_terminal())
            .cloned()
            .collect())
    }

    fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
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
        guard.writes.remove(&receipt_id);
        guard.revision = next_revision;
        self.publish_snapshot(&guard);
        let _ = self.receipt_changes.send((receipt_id, None));
        Ok(true)
    }

    fn len(&self) -> Result<usize, WriteStoreError> {
        let guard = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(guard
            .writes
            .values()
            .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            .count())
    }
}

fn next_revision(state: &WriteState) -> Result<u64, WriteStoreError> {
    state
        .revision
        .checked_add(1)
        .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))
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
            if self.closed || self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed);
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
