use std::collections::BTreeSet;

use fava_routing::RoutePlan;
use fava_state::event_is_newer;
use fava_write::{
    EventId, EventValue, LocalWriteEvent, MaterializationId, PublicKey, PublicationEvidence,
    Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, ReplaceableEventEdit, SignatureState,
    UnsignedEvent, WriteId, WriteIntent, WritePayload, WriteRouting,
};
use fava_write_store::{
    AcceptedWrite, WriteStoreError, apply_route_to_receipt, destination_evidence_capacity,
};

use crate::lifecycle::{
    capacity_reached, destinations, next_identity, next_revision, terminal_evictions,
};
use crate::semantic_acceptance::{
    attributed_failure, require_current, require_failure_source, require_qualified_source,
    validate_materialization, validate_source,
};
use crate::{RedbWriteStore, SemanticCustody};

pub(super) use crate::semantic_acceptance::edit_coordinate;

impl RedbWriteStore {
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
        reason = "one lock and one durable commit keep semantic admission atomic"
    )]
    fn accept_semantic_inner(
        &self,
        reservation: Option<u64>,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        let mut state = self.lock()?;
        let (payload, routing) = intent.into_parts();
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
        let selected_source = validate_materialization(&edit, author, &event, source, &routing)?;
        if let Some(receipt_id) = state.coordinates.get(&coordinate).copied() {
            let receipt = state.receipts.get(&receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
            let stored = state.semantics.get(&receipt_id).ok_or_else(|| {
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
        if !reserved && capacity_reached(&state, self.limits.active.get()) {
            return Err(WriteStoreError::Refused(format!(
                "active write bound {} reached",
                self.limits.active
            )));
        }
        let identity = state.next_identity;
        let next_identity = next_identity(identity)?;
        let write_id = WriteId::from_nonzero(identity);
        let receipt_id = ReceiptId::from_nonzero(identity);
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
            materialization_id: MaterializationId::FIRST,
            materialization_source: selected_source.map(|(id, _)| id),
            materialization_failure: None,
            retired_materializations: Vec::new(),
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
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: std::collections::BTreeMap::new(),
        };
        if let Some(plan) = initial_route {
            apply_route_to_receipt(&mut receipt, plan)?;
        }
        let custody = (vec![edit], author, selected_source, None, None);
        let terminal = receipt.is_terminal();
        let removals = terminal_evictions(&state, &receipt, self.limits.terminal.get());
        let next_revision = next_revision(&state)?;
        self.commit_accept(next_identity, &receipt, Some(&custody), &removals)?;
        for id in &removals {
            crate::release_semantic(&mut state, *id);
            state.receipts.remove(id);
        }
        state.next_identity = next_identity;
        state.revision = next_revision;
        state.semantics.insert(receipt_id, custody);
        if !terminal {
            state.coordinates.insert(coordinate, receipt_id);
        }
        state.receipts.insert(receipt_id, receipt.clone());
        self.publish_snapshot(&state);
        for id in removals {
            self.publish_receipt(None, id);
        }
        self.publish_receipt(Some(receipt.clone()), receipt_id);
        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current: receipt.current,
        })
    }

    pub(super) fn reserve_active_slot(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
    ) -> Result<u64, WriteStoreError> {
        let mut state = self.lock()?;
        let coordinate = edit_coordinate(edit, author);
        if state
            .reservations
            .values()
            .any(|reserved| reserved == &coordinate)
        {
            return Err(WriteStoreError::Refused(
                "replaceable coordinate already has an active reservation".to_owned(),
            ));
        }
        if state
            .coordinates
            .get(&coordinate)
            .and_then(|receipt_id| state.semantics.get(receipt_id))
            .is_some_and(|custody| custody.4.is_some())
        {
            return Err(WriteStoreError::Refused(
                "replaceable coordinate already has a durable successor".to_owned(),
            ));
        }
        if !state.coordinates.contains_key(&coordinate)
            && capacity_reached(&state, self.limits.active.get())
        {
            return Err(WriteStoreError::Refused(format!(
                "active write bound {} reached",
                self.limits.active
            )));
        }
        let reservation = state.next_reservation;
        state.next_reservation = reservation
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("active reservation exhausted".to_owned()))?;
        state.reservations.insert(reservation, coordinate);
        Ok(reservation)
    }

    pub(super) fn release_active_slot(&self, reservation: u64) -> Result<(), WriteStoreError> {
        let mut state = self.lock()?;
        if let Some(coordinate) = state.reservations.remove(&reservation) {
            if let Some(receipt_id) = state.coordinates.get(&coordinate).copied()
                && let Some(receipt) = state.receipts.get(&receipt_id).cloned()
            {
                self.publish_receipt(Some(receipt), receipt_id);
            }
            Ok(())
        } else {
            Err(WriteStoreError::Refused(
                "active reservation is not current".to_owned(),
            ))
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One transaction owns the swap.
    pub(super) fn install_semantic(
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
        let mut state = self.lock()?;
        let receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let (edits, author, current_source, _, _) =
            state.semantics.get(&receipt_id).cloned().ok_or_else(|| {
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
        let selected_source =
            validate_materialization(edit, author, &event, source, &receipt.routing)?;
        if receipt.write_id == write_id
            && receipt.current.event == EventValue::Unsigned(event.clone())
            && receipt.current.publication.materialization_source
                == selected_source.map(|(id, _)| id)
        {
            return Ok(receipt);
        }
        require_qualified_source(current_source, selected_source)?;
        let event_id = event.id.ok_or_else(|| {
            WriteStoreError::Refused("successor materialization has no stable id".to_owned())
        })?;
        if !event_is_newer(
            (event.created_at, event_id),
            (receipt.current.event.created_at(), receipt.current.id()),
        ) {
            return Err(WriteStoreError::Refused(
                "successor materialization is not newer than current event".to_owned(),
            ));
        }
        if receipt.current.publication.retired_materializations.len()
            >= destination_evidence_capacity()
        {
            return Err(WriteStoreError::Refused(
                "retired materialization evidence capacity reached".to_owned(),
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
            let current_custody = state.semantics.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("semantic custody does not exist".to_owned())
            })?;
            if current_custody.4.is_some() {
                return Err(WriteStoreError::Refused(
                    "replaceable coordinate already has a durable successor".to_owned(),
                ));
            }
            let custody: SemanticCustody = (
                edits,
                author,
                current_source,
                None,
                Some((None, event, selected_source, initial_route.cloned())),
            );
            self.commit_update(Some(&receipt), Some(&custody), &[])?;
            state.semantics.insert(receipt_id, custody);
            self.publish_receipt(Some(receipt.clone()), receipt_id);
            return Ok(receipt);
        }
        let mut retired = receipt.current.publication.retired_materializations.clone();
        retired.push((
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            receipt.current.publication.materialization_source,
            receipt.current.publication.materialization_failure.clone(),
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
        let materialization_id = expected.checked_next().ok_or_else(|| {
            WriteStoreError::Refused("materialization identity exhausted".to_owned())
        })?;
        let current = LocalWriteEvent::new(
            EventValue::Unsigned(event),
            PublicationEvidence {
                receipt_id,
                write_id,
                materialization_id,
                materialization_source: selected_source.map(|(id, _)| id),
                materialization_failure: None,
                retired_materializations: retired,
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
        let custody: SemanticCustody = (edits, author, selected_source, None, None);
        let next_revision = next_revision(&state)?;
        self.commit_update(Some(&updated), Some(&custody), &[])?;
        state.revision = next_revision;
        state.semantics.insert(receipt_id, custody);
        state.receipts.insert(receipt_id, updated.clone());
        self.publish_snapshot(&state);
        self.publish_receipt(Some(updated.clone()), receipt_id);
        Ok(updated)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_semantic_failure(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        source: Option<&EventValue>,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock()?;
        let receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let (edits, author, current_source, current_failed_source, successor) =
            state.semantics.get(&receipt_id).cloned().ok_or_else(|| {
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
            && receipt
                .current
                .publication
                .materialization_failure
                .as_deref()
                == Some(failure.as_str())
        {
            return Ok(receipt);
        }
        let mut updated = receipt;
        updated.current.publication.materialization_failure = Some(failure);
        let custody: SemanticCustody = (edits, author, current_source, failed_source_id, successor);
        let next_revision = next_revision(&state)?;
        self.commit_update(Some(&updated), Some(&custody), &[])?;
        state.revision = next_revision;
        state.semantics.insert(receipt_id, custody);
        state.receipts.insert(receipt_id, updated.clone());
        self.publish_snapshot(&state);
        self.publish_receipt(Some(updated.clone()), receipt_id);
        Ok(updated)
    }
}
