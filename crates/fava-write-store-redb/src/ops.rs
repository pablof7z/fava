use std::collections::BTreeMap;

use fava_routing::RoutePlan;
use fava_state::{RelayAccess, RelaySessionKey};
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, SignatureState, WriteId, WriteIntent,
    WritePayload, WriteRouting,
};
use fava_write_store::{
    AcceptedWrite, WriteStore, WriteStoreError, apply_route_to_receipt,
    validate_current_materialization, validate_delivery_outcome, validate_receipt_text,
};
use tokio::sync::broadcast;

use crate::{RedbWriteStore, StoreState};

impl WriteStore for RedbWriteStore {
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
        if active == self.limits.active.get() {
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
            WritePayload::Edit(_) => {
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
        self.commit_accept(next_identity, &receipt)?;
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
        self.commit_update(None, &[receipt_id])?;
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

impl RedbWriteStore {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, StoreState>, WriteStoreError> {
        self.state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))
    }

    fn update(
        &self,
        receipt_id: ReceiptId,
        mutation: impl FnOnce(&mut Receipt) -> Result<(), WriteStoreError>,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock()?;
        let mut receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let original = receipt.clone();
        mutation(&mut receipt)?;
        if receipt == original {
            return Ok(receipt);
        }
        let removals = terminal_evictions(&state, &receipt, self.limits.terminal.get());
        let next_revision = next_revision(&state)?;
        self.commit_update(Some(&receipt), &removals)?;
        for id in removals {
            state.receipts.remove(&id);
        }
        state.receipts.insert(receipt_id, receipt.clone());
        state.revision = next_revision;
        self.publish_snapshot(&state);
        self.publish_receipt(Some(receipt.clone()), receipt_id);
        Ok(receipt)
    }
}

fn destinations(routing: &WriteRouting) -> BTreeMap<RelaySessionKey, RelayDeliveryOutcome> {
    match routing {
        WriteRouting::Automatic => BTreeMap::new(),
        WriteRouting::Explicit(relays) => relays
            .iter()
            .cloned()
            .map(|relay| {
                (
                    RelaySessionKey::new(relay, RelayAccess::public()),
                    RelayDeliveryOutcome::Pending,
                )
            })
            .collect(),
    }
}

fn terminal_evictions(state: &StoreState, updated: &Receipt, maximum: usize) -> Vec<ReceiptId> {
    let mut terminal: Vec<_> = state
        .receipts
        .values()
        .filter(|receipt| receipt.is_terminal() && receipt.receipt_id != updated.receipt_id)
        .map(|receipt| receipt.receipt_id)
        .collect();
    if updated.is_terminal() {
        terminal.push(updated.receipt_id);
    }
    terminal.sort_unstable();
    let excess = terminal.len().saturating_sub(maximum);
    terminal.into_iter().take(excess).collect()
}

fn next_revision(state: &StoreState) -> Result<u64, WriteStoreError> {
    state
        .revision
        .checked_add(1)
        .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))
}

pub(crate) fn settle(receipt: &mut Receipt) {
    if receipt.route_settled
        && receipt
            .destinations()
            .values()
            .all(RelayDeliveryOutcome::is_terminal)
    {
        receipt.outcome = if receipt.desired_destinations.is_empty() {
            ReceiptOutcome::NoDestination
        } else {
            ReceiptOutcome::Complete
        };
    }
}

#[derive(Eq, PartialEq)]
struct UnsignedEventView<'a> {
    id: Option<fava_write::EventId>,
    pubkey: fava_write::PublicKey,
    created_at: fava_write::Timestamp,
    kind: fava_write::Kind,
    tags: &'a [fava_write::Tag],
    content: &'a str,
}

impl<'a> From<&'a fava_write::UnsignedEvent> for UnsignedEventView<'a> {
    fn from(event: &'a fava_write::UnsignedEvent) -> Self {
        Self {
            id: event.id,
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.as_slice(),
            content: &event.content,
        }
    }
}

impl<'a> From<&'a Event> for UnsignedEventView<'a> {
    fn from(event: &'a Event) -> Self {
        Self {
            id: Some(event.id),
            pubkey: event.pubkey,
            created_at: event.created_at,
            kind: event.kind,
            tags: event.tags.as_slice(),
            content: &event.content,
        }
    }
}
