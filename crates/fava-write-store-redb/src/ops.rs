use fava_routing::RoutePlan;
use fava_state::RelaySessionKey;
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, ReplaceableEventEdit, SignatureState,
    UnsignedEvent, WriteId, WriteIntent, WritePayload, WriteRouting,
};
use fava_write_store::{
    AcceptedWrite, WriteStore, WriteStoreError, apply_route_to_receipt,
    validate_current_materialization, validate_delivery_outcome, validate_receipt_text,
};
use tokio::sync::broadcast;

use crate::RedbWriteStore;
use crate::lifecycle::{UnsignedEventView, destinations, next_revision, settle};

impl WriteStore for RedbWriteStore {
    fn active_capacity(&self) -> usize {
        self.limits.active.get()
    }

    fn reserve_active(&self) -> Result<u64, WriteStoreError> {
        self.reserve_active_slot()
    }

    fn release_active(&self, reservation: u64) -> Result<(), WriteStoreError> {
        self.release_active_slot(reservation)
    }

    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)> {
        self.receipt_changes.subscribe()
    }

    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError> {
        let mut state = self.lock()?;
        let active = state
            .receipts
            .values()
            .filter(|receipt| !receipt.is_terminal())
            .count();
        if active
            .checked_add(state.reservations.len())
            .is_none_or(|used| used >= self.limits.active.get())
        {
            return Err(WriteStoreError::Refused(format!(
                "active write bound {} reached",
                self.limits.active
            )));
        }
        let identity = state.next_identity;
        let next_identity = identity
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("write identity exhausted".to_owned()))?;
        let write_id = WriteId::from_u64(identity);
        let receipt_id = ReceiptId::from_u64(identity);
        let (payload, routing) = intent.into_parts();
        let (event, signature) = match payload {
            WritePayload::Event(event) => (EventValue::Unsigned(event), SignatureState::Unsigned),
            WritePayload::Edit { .. } => {
                return Err(WriteStoreError::Refused(
                    "replaceable-event edit requires materialization before acceptance".to_owned(),
                ));
            }
            WritePayload::Presigned(event) => (EventValue::Signed(event), SignatureState::Signed),
        };
        let destinations = destinations(&routing);
        let desired_destinations = destinations.keys().cloned().collect();
        let explicit = matches!(routing, WriteRouting::Explicit(_));
        let current = LocalWriteEvent::new(
            event,
            PublicationEvidence {
                receipt_id,
                write_id,
                materialization_id: fava_write::MaterializationId::from_u64(1),
                materialization_source: None,
                materialization_failure: None,
                retired_materializations: Vec::new(),
                signature,
                destinations,
            },
        )?;
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
        let next_revision = next_revision(&state)?;
        self.commit_accept(next_identity, &receipt, None)?;
        state.next_identity = next_identity;
        state.revision = next_revision;
        state.receipts.insert(receipt_id, receipt.clone());
        self.publish_snapshot(&state);
        self.publish_receipt(Some(receipt), receipt_id);
        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current,
        })
    }

    fn accept_materialized_edit(
        &self,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_semantic(intent, event, source)
    }

    fn accept_reserved_materialized_edit(
        &self,
        reservation: u64,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        self.accept_reserved_semantic(reservation, intent, event, source)
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
        self.install_semantic(
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
        self.record_semantic_failure(
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
            fava_write::PublicKey,
            Option<(EventId, fava_write::Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        self.recover_semantic()
    }

    fn install_signed(
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
        self.update(receipt_id, |receipt| {
            validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
            match &receipt.current.event {
                EventValue::Signed(current) if current == &event => return Ok(()),
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
            receipt.current.event = EventValue::Signed(event);
            receipt.current.publication.signature = SignatureState::Signed;
            Ok(())
        })
    }

    fn record_signer_refusal(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        validate_receipt_text(&reason)?;
        self.update(receipt_id, |receipt| {
            validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
            match &receipt.current.publication.signature {
                SignatureState::Refused(current) if current == &reason => return Ok(()),
                SignatureState::Unsigned => {}
                SignatureState::Signed | SignatureState::Refused(_) => {
                    return Err(WriteStoreError::Refused(
                        "signer refusal is not current".to_owned(),
                    ));
                }
            }
            receipt.current.publication.signature = SignatureState::Refused(reason);
            Ok(())
        })
    }

    fn apply_route(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError> {
        self.update(receipt_id, |receipt| {
            validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
            let destinations: std::collections::BTreeSet<_> =
                plan.destinations.keys().cloned().collect();
            if plan.revision == receipt.route_revision
                && destinations == receipt.desired_destinations
                && plan.shortfalls == receipt.route_shortfalls
                && plan.settled == receipt.route_settled
            {
                return Ok(());
            }
            apply_route_to_receipt(receipt, plan)
        })
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
        self.update(receipt_id, |receipt| {
            validate_current_materialization(receipt, write_id, materialization_id, event_id)?;
            if !matches!(receipt.current.event, EventValue::Signed(_)) {
                return Err(WriteStoreError::Refused("event is not signed".to_owned()));
            }
            let current_attempt = receipt.attempts.get(session).copied().unwrap_or(0);
            if current_attempt == attempt
                && matches!(
                    receipt.current.publication.destinations.get(session),
                    Some(RelayDeliveryOutcome::Attempting)
                )
            {
                return Ok(());
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
            *outcome = RelayDeliveryOutcome::Attempting;
            receipt.attempts.insert(session.clone(), attempt);
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
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
        validate_delivery_outcome(&outcome)?;
        if !outcome.is_terminal() && !matches!(outcome, RelayDeliveryOutcome::Retryable { .. }) {
            return Err(WriteStoreError::Refused(
                "recorded delivery outcome is not terminal or retryable".to_owned(),
            ));
        }
        self.update(receipt_id, |receipt| {
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
                return Ok(());
            }
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
            Ok(())
        })
    }

    fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        let Some(current) = self.receipt(receipt_id)? else {
            return Ok(None);
        };
        if matches!(current.outcome, ReceiptOutcome::Cancelled) {
            return Ok(Some(current));
        }
        self.update(receipt_id, |receipt| {
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
            Ok(())
        })
        .map(Some)
    }

    fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError> {
        Ok(self.lock()?.receipts.get(&receipt_id).cloned())
    }

    fn recover_open(&self) -> Result<Vec<Receipt>, WriteStoreError> {
        Ok(self
            .lock()?
            .receipts
            .values()
            .filter(|receipt| !receipt.is_terminal())
            .cloned()
            .collect())
    }

    fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        let mut state = self.lock()?;
        if state
            .receipts
            .get(&receipt_id)
            .is_some_and(|receipt| !receipt.is_terminal())
        {
            return Err(WriteStoreError::Refused(
                "active receipt cannot be removed".to_owned(),
            ));
        }
        if !state.receipts.contains_key(&receipt_id) {
            return Ok(false);
        }
        let next_revision = next_revision(&state)?;
        self.commit_update(None, None, &[receipt_id])?;
        crate::release_semantic(&mut state, receipt_id);
        state.receipts.remove(&receipt_id);
        state.revision = next_revision;
        self.publish_snapshot(&state);
        self.publish_receipt(None, receipt_id);
        Ok(true)
    }

    fn len(&self) -> Result<usize, WriteStoreError> {
        Ok(self
            .lock()?
            .receipts
            .values()
            .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            .count())
    }
}
use std::collections::BTreeMap;
