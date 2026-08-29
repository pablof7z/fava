use fava_relay::RelaySessionKey;
use fava_routing::RoutePlan;
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, RevisionId, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, SignatureState, WriteId, WriteRouting,
};
use fava_write_store::{
    WriteStoreError, apply_route_to_receipt, validate_current_revision,
    validate_delivery_outcome, validate_receipt_text,
};

use super::MemoryWriteStore;
use super::model::{UnsignedEventView, destinations, settle};
use super::state::{next_revision, release_semantic};

impl MemoryWriteStore {
    pub(super) fn install_signed_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        event: Event,
    ) -> Result<Receipt, WriteStoreError> {
        event
            .verify()
            .map_err(|error| WriteStoreError::Refused(error.to_string()))?;
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        match &receipt.current.event {
            EventValue::Signed(current) if current == &event => return Ok(receipt.clone()),
            EventValue::Signed(_) => {
                return Err(WriteStoreError::Refused(
                    "event is already signed differently".to_owned(),
                ));
            }
            EventValue::Unsigned(unsigned)
                if UnsignedEventView::from(unsigned) == UnsignedEventView::from(&event) => {}
            EventValue::Unsigned(_) => {
                return Err(WriteStoreError::Refused(
                    "signature does not match current unsigned event".to_owned(),
                ));
            }
        }
        if !matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized | SignatureState::Signed
        ) {
            return Err(WriteStoreError::Refused(
                "signature completion was not authorized".to_owned(),
            ));
        }
        let next_revision = next_revision(&state)?;
        if state.successors.contains_key(&receipt_id) {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.event = EventValue::Signed(event);
        receipt.current.publication.signature = SignatureState::Signed;
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    /// Commit a signer refusal, only from a generation the store durably
    /// authorized.
    pub(super) fn record_signer_refusal_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        validate_receipt_text(&reason)?;
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        match &receipt.current.publication.signature {
            SignatureState::Refused(current) if current == &reason => return Ok(receipt.clone()),
            SignatureState::Authorized => {}
            SignatureState::Unsigned | SignatureState::Retryable(_) => {
                return Err(WriteStoreError::Refused(
                    "signer refusal was not authorized".to_owned(),
                ));
            }
            SignatureState::Signed | SignatureState::Refused(_) => {
                return Err(WriteStoreError::Refused(
                    "signer refusal is not current".to_owned(),
                ));
            }
        }
        let next_revision = next_revision(&state)?;
        if state.successors.contains_key(&receipt_id) {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.publication.signature = SignatureState::Refused(reason);
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    pub(super) fn authorize_signing_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(&receipt, write_id, revision_id, event_id)?;
        if !matches!(receipt.current.event, EventValue::Unsigned(_)) {
            return Err(WriteStoreError::Refused(
                "event is already signed".to_owned(),
            ));
        }
        if matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            return Ok(receipt);
        }
        if !matches!(
            receipt.current.publication.signature,
            SignatureState::Unsigned | SignatureState::Retryable(_)
        ) {
            return Err(WriteStoreError::Refused(
                "signing authorization is not current".to_owned(),
            ));
        }
        let reserved = state
            .edits
            .get(&receipt_id)
            .is_some_and(|(edits, author, _, _)| {
                edits.last().is_some_and(|edit| {
                    let coordinate = super::state::edit_coordinate(edit, *author);
                    state
                        .reservations
                        .values()
                        .any(|value| value == &coordinate)
                })
            });
        let signature = if reserved {
            SignatureState::Retryable(format!(
                "signing authorization for write {} receipt {} revision {} event {} deferred until its coordinate reservation resolves",
                write_id.as_u64(),
                receipt_id.as_u64(),
                revision_id.as_u64(),
                event_id
            ))
        } else {
            SignatureState::Authorized
        };
        if receipt.current.publication.signature == signature {
            return Ok(receipt);
        }
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.publication.signature = signature;
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    /// Mark signing retryable, promoting a pending successor when one is waiting.
    pub(super) fn record_signer_retryable_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        validate_receipt_text(&reason)?;
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        if !matches!(
            receipt.current.publication.signature,
            SignatureState::Unsigned | SignatureState::Authorized | SignatureState::Retryable(_)
        ) {
            return Err(WriteStoreError::Refused(
                "retryable signer failure is not current".to_owned(),
            ));
        }
        if matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) && state.successors.contains_key(&receipt_id)
        {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.publication.signature = SignatureState::Retryable(reason);
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    pub(super) fn has_signing_successor(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<bool, WriteStoreError> {
        let state = self.lock_state()?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        Ok(state.successors.contains_key(&receipt_id))
    }

    fn promote_authorized_successor(
        &self,
        state: &mut super::semantic::WriteState,
        receipt_id: ReceiptId,
    ) -> Result<Receipt, WriteStoreError> {
        let next_revision = next_revision(state)?;
        let (edit, event, successor_source, successor_route) =
            state.successors.remove(&receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("durable semantic successor is missing".to_owned())
            })?;
        let receipt =
            state.writes.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
        let (mut edits, author, _current_source, _failed_source) =
            state.edits.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("semantic custody is missing".to_owned())
            })?;
        let mut retired = receipt.current.publication.retired_revisions.clone();
        retired.push((
            receipt.current.publication.revision_id,
            receipt.current.id(),
            receipt.current.publication.revision_source,
            receipt.current.publication.revision_failure.clone(),
        ));
        let revision_id = receipt
            .current
            .publication
            .revision_id
            .checked_next()
            .ok_or_else(|| {
                WriteStoreError::Refused("revision identity exhausted".to_owned())
            })?;
        let source_correction = edit.is_none();
        let successor_destinations = if source_correction {
            let mut sessions = receipt.desired_destinations.clone();
            sessions.extend(receipt.current.publication.destinations.keys().cloned());
            sessions
                .iter()
                .cloned()
                .map(|session| (session, RelayDeliveryOutcome::Pending))
                .collect()
        } else {
            destinations(&receipt.routing)
        };
        let publication = PublicationEvidence {
            receipt_id,
            write_id: receipt.write_id,
            revision_id,
            revision_source: successor_source.map(|(id, _)| id),
            revision_failure: None,
            retired_revisions: retired,
            signature: SignatureState::Unsigned,
            destinations: successor_destinations,
        };
        let current = LocalWriteEvent::new(EventValue::Unsigned(event), publication)?;
        let explicit = matches!(receipt.routing, WriteRouting::Explicit(_));
        let desired_destinations = current.publication.destinations.keys().cloned().collect();
        let mut updated = Receipt {
            current,
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: std::collections::BTreeMap::new(),
            ..receipt
        };
        if let Some(plan) = successor_route.as_ref() {
            apply_route_to_receipt(&mut updated, plan)?;
        }
        if let Some(edit) = edit {
            edits.push(edit);
        }
        state
            .edits
            .insert(receipt_id, (edits, author, successor_source, None));
        state.writes.insert(receipt_id, updated.clone());
        state.revision = next_revision;
        self.publish_receipt(state, &updated);
        Ok(updated)
    }

    pub(super) fn apply_route_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        let mut updated = receipt.clone();
        apply_route_to_receipt(&mut updated, plan)?;
        if updated == *receipt {
            return Ok(updated);
        }
        let next_revision = next_revision(&state)?;
        state.writes.insert(receipt_id, updated.clone());
        state.revision = next_revision;
        if updated.is_terminal() {
            release_semantic(&mut state, receipt_id);
        }
        self.publish_receipt(&state, &updated);
        Ok(updated)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn begin_attempt_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        let current_attempt = receipt.attempts.get(session).copied().unwrap_or(0);
        if current_attempt == attempt
            && matches!(
                receipt.current.publication.destinations.get(session),
                Some(RelayDeliveryOutcome::Attempting)
            )
        {
            return Ok(receipt.clone());
        }
        let expected_attempt = current_attempt
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("attempt count exhausted".to_owned()))?;
        if attempt != expected_attempt {
            return Err(WriteStoreError::Refused(
                "attempt is not current".to_owned(),
            ));
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
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        *receipt
            .current
            .publication
            .destinations
            .get_mut(session)
            .expect("checked above") = RelayDeliveryOutcome::Attempting;
        receipt.attempts.insert(session.clone(), attempt);
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    #[allow(clippy::too_many_arguments)]
    /// Commit one relay's delivery outcome while that attempt is still current.
    pub(super) fn record_outcome_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
        outcome: RelayDeliveryOutcome,
    ) -> Result<Receipt, WriteStoreError> {
        validate_delivery_outcome(&outcome)?;
        if !outcome.is_terminal() && !matches!(outcome, RelayDeliveryOutcome::Retryable { .. }) {
            return Err(WriteStoreError::Refused(
                "recorded delivery outcome is not terminal or retryable".to_owned(),
            ));
        }
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        if receipt.attempts.get(session).copied() != Some(attempt) {
            return Err(WriteStoreError::Refused(
                "attempt is not current".to_owned(),
            ));
        }
        let current = receipt
            .current
            .publication
            .destinations
            .get_mut(session)
            .ok_or_else(|| WriteStoreError::Refused("destination does not exist".to_owned()))?;
        if current == &outcome {
            return Ok(receipt.clone());
        }
        let may_transition = matches!(current, RelayDeliveryOutcome::Attempting)
            || (matches!(current, RelayDeliveryOutcome::Retryable { .. })
                && matches!(outcome, RelayDeliveryOutcome::GivenUp { .. }));
        if !may_transition {
            return Err(WriteStoreError::Refused(
                "attempt is not current".to_owned(),
            ));
        }
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        *receipt
            .current
            .publication
            .destinations
            .get_mut(session)
            .expect("checked above") = outcome;
        settle(receipt);
        let current = receipt.clone();
        state.revision = next_revision;
        if current.is_terminal() {
            release_semantic(&mut state, receipt_id);
        }
        self.publish_receipt(&state, &current);
        Ok(current)
    }
}
