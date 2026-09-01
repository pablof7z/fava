//! Durable exact-generation signing authorization and semantic successor promotion.

use std::collections::BTreeMap;

use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, PublicationEvidence, Receipt, ReceiptId,
    ReceiptOutcome, RevisionId, SignatureState, WriteId, WriteRouting,
};
use fava_write_store::{WriteStoreError, validate_current_revision, validate_receipt_text};

use crate::lifecycle::UnsignedEventView;
use crate::lifecycle::{destinations, next_revision};
use crate::{RedbWriteStore, SemanticCustody, StoreState};

impl RedbWriteStore {
    pub(super) fn recover_authorized_signing(&self) -> Result<(), WriteStoreError> {
        let authorized = {
            let state = self.lock()?;
            state
                .receipts
                .values()
                .filter(|receipt| {
                    matches!(
                        receipt.current.publication.signature,
                        SignatureState::Authorized
                    )
                })
                .map(|receipt| {
                    (
                        receipt.write_id,
                        receipt.receipt_id,
                        receipt.current.publication.revision_id,
                        receipt.current.id(),
                    )
                })
                .collect::<Vec<_>>()
        };
        for (write_id, receipt_id, revision_id, event_id) in authorized {
            self.record_signer_retryable_current(
                write_id,
                receipt_id,
                revision_id,
                event_id,
                format!(
                    "process ended after signing authorization for write {} receipt {} revision {} event {}; retry is permitted",
                    write_id.as_u64(),
                    receipt_id.as_u64(),
                    revision_id.as_u64(),
                    event_id
                ),
            )?;
        }
        Ok(())
    }

    pub(super) fn authorize_signing_current(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self.lock()?;
        let mut receipt = state
            .receipts
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
            .semantics
            .get(&receipt_id)
            .is_some_and(|(edits, author, _, _, _)| {
                edits.last().is_some_and(|edit| {
                    let coordinate = crate::semantic::edit_coordinate(edit, *author);
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
        receipt.current.publication.signature = signature;
        let next_revision = next_revision(&state)?;
        self.commit_update(Some(&receipt), state.semantics.get(&receipt_id), &[])?;
        state.receipts.insert(receipt_id, receipt.clone());
        state.revision = next_revision;
        self.publish_snapshot(&state);
        self.publish_receipt(Some(receipt.clone()), receipt_id);
        Ok(receipt)
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
        let mut state = self.lock()?;
        let mut receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(&receipt, write_id, revision_id, event_id)?;
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
        ) && state
            .semantics
            .get(&receipt_id)
            .is_some_and(|custody| custody.4.is_some())
        {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        receipt.current.publication.signature = SignatureState::Retryable(reason);
        self.commit_receipt(&mut state, receipt)
    }

    pub(super) fn has_signing_successor(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        revision_id: RevisionId,
        event_id: EventId,
    ) -> Result<bool, WriteStoreError> {
        let state = self.lock()?;
        let receipt = state
            .receipts
            .get(&receipt_id)
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(receipt, write_id, revision_id, event_id)?;
        Ok(state
            .semantics
            .get(&receipt_id)
            .is_some_and(|custody| custody.4.is_some()))
    }

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
        let mut state = self.lock()?;
        let mut receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(&receipt, write_id, revision_id, event_id)?;
        match &receipt.current.event {
            EventValue::Signed(current) if current == &event => return Ok(receipt),
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
            SignatureState::Authorized
        ) {
            return Err(WriteStoreError::Refused(
                "signature completion was not authorized".to_owned(),
            ));
        }
        if state
            .semantics
            .get(&receipt_id)
            .is_some_and(|custody| custody.4.is_some())
        {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        receipt.current.event = EventValue::Signed(event);
        receipt.current.publication.signature = SignatureState::Signed;
        self.commit_receipt(&mut state, receipt)
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
        let mut state = self.lock()?;
        let mut receipt = state
            .receipts
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        validate_current_revision(&receipt, write_id, revision_id, event_id)?;
        if matches!(&receipt.current.publication.signature, SignatureState::Refused(current) if current == &reason)
        {
            return Ok(receipt);
        }
        if !matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            return Err(WriteStoreError::Refused(
                "signer refusal was not authorized".to_owned(),
            ));
        }
        if state
            .semantics
            .get(&receipt_id)
            .is_some_and(|custody| custody.4.is_some())
        {
            return self.promote_authorized_successor(&mut state, receipt_id);
        }
        receipt.current.publication.signature = SignatureState::Refused(reason);
        self.commit_receipt(&mut state, receipt)
    }

    fn commit_receipt(
        &self,
        state: &mut StoreState,
        receipt: Receipt,
    ) -> Result<Receipt, WriteStoreError> {
        let receipt_id = receipt.receipt_id;
        let next_revision = next_revision(state)?;
        self.commit_update(Some(&receipt), state.semantics.get(&receipt_id), &[])?;
        state.receipts.insert(receipt_id, receipt.clone());
        state.revision = next_revision;
        self.publish_snapshot(state);
        self.publish_receipt(Some(receipt.clone()), receipt_id);
        Ok(receipt)
    }

    fn promote_authorized_successor(
        &self,
        state: &mut StoreState,
        receipt_id: ReceiptId,
    ) -> Result<Receipt, WriteStoreError> {
        let Some((
            mut edits,
            author,
            _current_source,
            _failed_source,
            Some((edit, event, successor_source, successor_route)),
        )) = state.semantics.get(&receipt_id).cloned()
        else {
            return Err(WriteStoreError::Refused(
                "durable semantic successor is missing".to_owned(),
            ));
        };
        let receipt =
            state.receipts.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
        if !matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            return Err(WriteStoreError::Refused(
                "durable successor predecessor is not authorized".to_owned(),
            ));
        }
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
            .ok_or_else(|| WriteStoreError::Refused("revision identity exhausted".to_owned()))?;
        let source_correction = edit.is_none();
        let successor_destinations = if source_correction {
            let mut sessions = receipt.desired_destinations.clone();
            sessions.extend(receipt.current.publication.destinations.keys().cloned());
            sessions
                .iter()
                .cloned()
                .map(|session| (session, fava_write::RelayDeliveryOutcome::Pending))
                .collect()
        } else {
            destinations(&receipt.routing, &receipt.access)
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
            attempts: BTreeMap::new(),
            ..receipt
        };
        if let Some(plan) = successor_route.as_ref() {
            fava_write_store::apply_route_to_receipt(&mut updated, plan)?;
        }
        if let Some(edit) = edit {
            edits.push(edit);
        }
        let custody: SemanticCustody = (edits, author, successor_source, None, None);
        let next_revision = next_revision(state)?;
        self.commit_update(Some(&updated), Some(&custody), &[])?;
        state.semantics.insert(receipt_id, custody);
        state.receipts.insert(receipt_id, updated.clone());
        state.revision = next_revision;
        self.publish_snapshot(state);
        self.publish_receipt(Some(updated.clone()), receipt_id);
        Ok(updated)
    }
}
