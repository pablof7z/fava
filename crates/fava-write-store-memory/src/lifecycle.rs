use fava_routing::RoutePlan;
use fava_state::RelaySessionKey;
use fava_write::{
    Event, EventId, EventValue, MaterializationId, Receipt, ReceiptId, RelayDeliveryOutcome,
    SignatureState, WriteId,
};
use fava_write_store::{
    WriteStoreError, apply_route_to_receipt, validate_current_materialization,
    validate_delivery_outcome, validate_receipt_text,
};

use super::MemoryWriteStore;
use super::model::{UnsignedEventView, settle};
use super::state::{next_revision, release_semantic};

impl MemoryWriteStore {
    pub(super) fn install_signed_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
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
        validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
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
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.event = EventValue::Signed(event);
        receipt.current.publication.signature = SignatureState::Signed;
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    pub(super) fn record_signer_refusal_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        validate_receipt_text(&reason)?;
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
        match &receipt.current.publication.signature {
            SignatureState::Refused(current) if current == &reason => return Ok(receipt.clone()),
            SignatureState::Unsigned => {}
            SignatureState::Signed | SignatureState::Refused(_) => {
                return Err(WriteStoreError::Refused(
                    "signer refusal is not current".to_owned(),
                ));
            }
        }
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        receipt.current.publication.signature = SignatureState::Refused(reason);
        let current = receipt.clone();
        state.revision = next_revision;
        self.publish_receipt(&state, &current);
        Ok(current)
    }

    pub(super) fn apply_route_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
        let plan_destinations: std::collections::BTreeSet<_> =
            plan.destinations.keys().cloned().collect();
        if plan.revision == receipt.route_revision
            && plan_destinations == receipt.desired_destinations
            && plan.shortfalls == receipt.route_shortfalls
            && plan.settled == receipt.route_settled
        {
            return Ok(receipt.clone());
        }
        let next_revision = next_revision(&state)?;
        let receipt = state.writes.get_mut(&receipt_id).expect("checked above");
        apply_route_to_receipt(receipt, plan)?;
        let updated = receipt.clone();
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
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock_state()?;
        let receipt = state
            .writes
            .get_mut(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
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
    pub(super) fn record_outcome_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
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
        validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
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
