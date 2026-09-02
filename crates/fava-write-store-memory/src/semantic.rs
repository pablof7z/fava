use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;
use std::sync::Arc;

use fava_query::{SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus};
use fava_routing::RoutePlan;
use fava_state::{EventCoordinate, event_is_newer};
use fava_write::{
    EventEdit, EventId, EventValue, LocalWriteEvent, PublicKey, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, RevisionId, SignatureState, Timestamp,
    UnsignedEvent, WriteId, WriteIntent, WritePayload, WriteRouting,
};
use fava_write_store::{
    AcceptedWrite, WriteStoreError, apply_route_to_receipt, destination_evidence_capacity,
};

use super::MemoryWriteStore;
use super::model::destinations;
use super::semantic_acceptance::{require_current, validate_revision, validate_source};
use super::state::{
    attributed_failure, capacity_reached, edit_coordinate, next_identity, next_revision,
    require_failure_source, require_qualified_source,
};

/// The whole in-memory ledger behind one store: coordinate reservations,
/// retained receipts, coordinate ownership, pending successors, and queued edits.
#[derive(Clone, Debug)]
pub(super) struct WriteState {
    pub(super) revision: u64,
    pub(super) next_identity: NonZeroU64,
    pub(super) next_reservation: u64,
    pub(super) reservations: BTreeMap<u64, EventCoordinate>,
    pub(super) writes: BTreeMap<ReceiptId, Receipt>,
    pub(super) coordinates: BTreeMap<EventCoordinate, ReceiptId>,
    #[allow(clippy::type_complexity)] // Existing values avoid a second successor lifecycle type.
    pub(super) successors: BTreeMap<
        ReceiptId,
        (
            Option<EventEdit>,
            UnsignedEvent,
            Option<(EventId, Timestamp)>,
            Option<RoutePlan>,
        ),
    >,
    #[allow(clippy::type_complexity)] // Existing values deliberately avoid a state wrapper.
    pub(super) edits: BTreeMap<
        ReceiptId,
        (
            Vec<EventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        ),
    >,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            revision: 0,
            next_identity: NonZeroU64::MIN,
            next_reservation: 1,
            reservations: BTreeMap::new(),
            writes: BTreeMap::new(),
            coordinates: BTreeMap::new(),
            successors: BTreeMap::new(),
            edits: BTreeMap::new(),
        }
    }
}

impl MemoryWriteStore {
    pub(super) fn snapshot(state: &WriteState) -> SourceSnapshot {
        let mut seen_event_ids = BTreeSet::new();
        let mut events: Vec<_> = state
            .writes
            .values()
            .rev()
            .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            .filter(|receipt| seen_event_ids.insert(receipt.current.id()))
            .map(|receipt| SourceEvent::Local(receipt.current.clone()))
            .collect();
        events.reverse();
        SourceSnapshot {
            kind: SourceKind::WriteStore,
            revision: SourceRevision(state.revision),
            status: SourceStatus::Open,
            events,
            retractions: Vec::new(),
        }
    }

    pub(super) fn publish_snapshot(&self, state: &WriteState) {
        self.latest.send_replace(Arc::new(Self::snapshot(state)));
    }

    pub(super) fn accept_semantic(
        &self,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_semantic_inner(None, intent, event, source, None)
    }

    pub(super) fn accept_reserved_semantic(
        &self,
        reservation: u64,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_semantic_inner(Some(reservation), intent, event, source, initial_route)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one lock and one volatile commit keep semantic admission atomic"
    )]
    fn accept_semantic_inner(
        &self,
        reservation: Option<u64>,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        let mut state = self.lock_state()?;
        let (payload, routing, accepted_access) = intent.into_parts();
        let WritePayload::Edit { edit, author } = payload else {
            return Err(WriteStoreError::Refused(
                "semantic acceptance requires a replaceable-event edit".to_owned(),
            ));
        };
        let coordinate = edit_coordinate(&edit, author);
        let reserved = if let Some(reservation) = reservation {
            let Some(reserved_coordinate) = state.reservations.get(&reservation) else {
                return Err(WriteStoreError::Refused(
                    "active reservation is not current".to_owned(),
                ));
            };
            if reserved_coordinate != &coordinate {
                return Err(WriteStoreError::Refused(
                    "active reservation belongs to a different replaceable coordinate".to_owned(),
                ));
            }
            state.reservations.remove(&reservation);
            true
        } else {
            false
        };
        let selected_source = validate_revision(&edit, author, &event, source, &routing)?;

        if let Some(receipt_id) = state.coordinates.get(&coordinate).copied() {
            let receipt = state.writes.get(&receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
            let stored = state.edits.get(&receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("semantic custody is missing".to_owned())
            })?;
            if stored.0.as_slice() == [edit.clone()]
                && stored.1 == author
                && stored.2 == selected_source
                && receipt.routing == routing
                && receipt.current.event == EventValue::Unsigned(event.clone())
                && initial_route.is_none_or(|plan| {
                    plan.revision == receipt.route_revision
                        && apply_route_to_receipt(&mut receipt.clone(), plan).is_ok()
                })
            {
                return Ok(AcceptedWrite {
                    write_id: receipt.write_id,
                    receipt_id: receipt.receipt_id,
                    current: receipt.current.clone(),
                });
            }
            return self.compose_semantic(
                &mut state,
                receipt_id,
                edit,
                author,
                &routing,
                event,
                source,
                initial_route,
            );
        }

        if !reserved && capacity_reached(&state, self.capacity.get()) {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }
        let identity = state.next_identity;
        let next_identity = next_identity(identity)?;
        let write_id = WriteId::from_nonzero(identity);
        let receipt_id = ReceiptId::from_nonzero(identity);
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
            revision_id: RevisionId::FIRST,
            revision_source: selected_source.map(|(id, _)| id),
            revision_failure: None,
            retired_revisions: Vec::new(),
            signature: SignatureState::Unsigned,
            destinations: destinations(&routing),
        };
        let current = LocalWriteEvent::new(EventValue::Unsigned(event), publication)?;
        let explicit = matches!(routing, WriteRouting::Explicit(_));
        let desired_destinations = current.publication.destinations.keys().cloned().collect();
        let mut receipt = Receipt {
            write_id,
            receipt_id,
            current: current.clone(),
            routing,
            access: accepted_access,
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: BTreeMap::new(),
        };
        if let Some(plan) = initial_route {
            apply_route_to_receipt(&mut receipt, plan)?;
        }
        let next_revision = next_revision(&state)?;

        state.next_identity = next_identity;
        state.revision = next_revision;
        if !receipt.is_terminal() {
            state.coordinates.insert(coordinate, receipt_id);
            state
                .edits
                .insert(receipt_id, (vec![edit], author, selected_source, None));
        }
        state.writes.insert(receipt_id, receipt.clone());
        self.publish_receipt(&state, &receipt);
        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current: receipt.current,
        })
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn install_semantic(
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
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let (edits, author, current_source, _) =
            state.edits.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("semantic custody does not exist".to_owned())
            })?;
        require_current(
            &receipt,
            write_id,
            expected,
            expected_source,
            current_source,
        )?;
        if edits.as_slice() != applied_edits {
            return Err(WriteStoreError::Refused(
                "successor did not apply the complete durable semantic edit sequence".to_owned(),
            ));
        }
        let edit = edits.last().ok_or_else(|| {
            WriteStoreError::Refused("semantic edit sequence is empty".to_owned())
        })?;
        let selected_source = validate_revision(edit, author, &event, source, &receipt.routing)?;
        if receipt.current.event == EventValue::Unsigned(event.clone())
            && receipt.current.publication.revision_source == selected_source.map(|(id, _)| id)
        {
            return Ok(receipt);
        }
        require_qualified_source(current_source, selected_source)?;
        let event_id = event.id.ok_or_else(|| {
            WriteStoreError::Refused("successor revision has no stable id".to_owned())
        })?;
        if !event_is_newer(
            (event.created_at, event_id),
            (receipt.current.event.created_at(), receipt.current.id()),
        ) {
            return Err(WriteStoreError::Refused(
                "successor revision is not newer than current event".to_owned(),
            ));
        }
        if receipt.current.publication.retired_revisions.len() >= destination_evidence_capacity() {
            return Err(WriteStoreError::Refused(
                "retired revision evidence capacity reached".to_owned(),
            ));
        }
        if let Some(plan) = initial_route {
            let mut routed = receipt.clone();
            apply_route_to_receipt(&mut routed, plan)?;
        }
        if matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            if state.successors.contains_key(&receipt_id) {
                return Err(WriteStoreError::Refused(
                    "replaceable coordinate already has a durable successor".to_owned(),
                ));
            }
            state.successors.insert(
                receipt_id,
                (None, event, selected_source, initial_route.cloned()),
            );
            self.publish_receipt_only(&receipt);
            return Ok(receipt);
        }

        let mut retired = receipt.current.publication.retired_revisions.clone();
        retired.push((
            receipt.current.publication.revision_id,
            receipt.current.id(),
            receipt.current.publication.revision_source,
            receipt.current.publication.revision_failure.clone(),
        ));
        let mut correction_destinations: BTreeSet<_> = receipt.desired_destinations.clone();
        correction_destinations.extend(receipt.current.publication.destinations.keys().cloned());
        if correction_destinations.len() > destination_evidence_capacity() {
            return Err(WriteStoreError::Refused(
                "correction destination capacity reached".to_owned(),
            ));
        }
        let destinations = correction_destinations
            .iter()
            .cloned()
            .map(|session| (session, RelayDeliveryOutcome::Pending))
            .collect();
        let revision_id = receipt
            .current
            .publication
            .revision_id
            .checked_next()
            .ok_or_else(|| WriteStoreError::Refused("revision identity exhausted".to_owned()))?;
        let current = LocalWriteEvent::new(
            EventValue::Unsigned(event),
            PublicationEvidence {
                receipt_id,
                write_id,
                revision_id,
                revision_source: selected_source.map(|(id, _)| id),
                revision_failure: None,
                retired_revisions: retired,
                signature: SignatureState::Unsigned,
                destinations,
            },
        )?;
        let mut updated = receipt;
        updated.current = current;
        updated.outcome = ReceiptOutcome::Open;
        updated.desired_destinations = correction_destinations;
        updated.attempts.clear();
        if let Some(plan) = initial_route {
            apply_route_to_receipt(&mut updated, plan)?;
        }
        let next_revision = next_revision(&state)?;

        state.revision = next_revision;
        state
            .edits
            .insert(receipt_id, (edits, author, selected_source, None));
        state.writes.insert(receipt_id, updated.clone());
        self.publish_receipt(&state, &updated);
        Ok(updated)
    }

    #[allow(clippy::too_many_arguments)]
    /// Attribute one revision failure to the exact generation and source
    /// that produced it.
    pub(super) fn record_semantic_failure(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: RevisionId,
        expected_source: Option<EventId>,
        source: Option<&EventValue>,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let (edits, author, current_source, current_failed_source) =
            state.edits.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("semantic custody does not exist".to_owned())
            })?;
        require_current(
            &receipt,
            write_id,
            expected,
            expected_source,
            current_source,
        )?;
        let edit = edits.last().ok_or_else(|| {
            WriteStoreError::Refused("semantic edit sequence is empty".to_owned())
        })?;
        let failed_source = validate_source(edit, author, source)?;
        require_failure_source(current_source, failed_source)?;
        let failed_source_id = failed_source.map(|(id, _)| id);
        let failure = attributed_failure(expected, failed_source_id, reason);
        if current_failed_source == failed_source_id
            && receipt.current.publication.revision_failure.as_deref() == Some(failure.as_str())
        {
            return Ok(receipt);
        }

        let mut updated = receipt;
        updated.current.publication.revision_failure = Some(failure);
        let next_revision = next_revision(&state)?;
        state.revision = next_revision;
        state.edits.insert(
            receipt_id,
            (edits, author, current_source, failed_source_id),
        );
        state.writes.insert(receipt_id, updated.clone());
        self.publish_receipt(&state, &updated);
        Ok(updated)
    }
}
