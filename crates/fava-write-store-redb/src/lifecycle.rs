use std::collections::BTreeMap;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_write::{Event, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, WriteRouting};
use fava_write_store::WriteStoreError;

use crate::{RedbWriteStore, StoreState};

impl RedbWriteStore {
    pub(super) fn lock(&self) -> Result<std::sync::MutexGuard<'_, StoreState>, WriteStoreError> {
        self.state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))
    }

    pub(super) fn update(
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
        let semantic = state.semantics.get(&receipt_id);
        self.commit_update(Some(&receipt), semantic, &removals)?;
        for id in &removals {
            crate::release_semantic(&mut state, *id);
            state.receipts.remove(id);
        }
        if receipt.is_terminal() {
            crate::release_semantic_coordinate(&mut state, receipt_id);
        }
        state.receipts.insert(receipt_id, receipt.clone());
        state.revision = next_revision;
        self.publish_snapshot(&state);
        for id in removals {
            self.publish_receipt(None, id);
        }
        self.publish_receipt(Some(receipt.clone()), receipt_id);
        Ok(receipt)
    }
}

pub(super) fn destinations(
    routing: &WriteRouting,
) -> BTreeMap<RelaySessionKey, RelayDeliveryOutcome> {
    match routing {
        WriteRouting::Automatic => BTreeMap::new(),
        WriteRouting::Explicit(relays) => relays
            .iter()
            .cloned()
            .map(|relay| {
                (
                    RelaySessionKey {
                        relay,
                        access: RelayAccess::Public,
                    },
                    RelayDeliveryOutcome::Pending,
                )
            })
            .collect(),
    }
}

pub(super) fn terminal_evictions(
    state: &StoreState,
    updated: &Receipt,
    maximum: usize,
) -> Vec<ReceiptId> {
    let mut terminal: Vec<_> = state
        .receipts
        .values()
        .filter(|receipt| receipt.is_terminal() && receipt.receipt_id != updated.receipt_id)
        .map(|receipt| receipt.receipt_id)
        .collect();
    terminal.sort_unstable();
    let projected = terminal.len() + usize::from(updated.is_terminal());
    let excess = projected.saturating_sub(maximum);
    terminal.into_iter().take(excess).collect()
}

pub(super) fn validate_recovered_bounds(
    state: &StoreState,
    active: usize,
    terminal: usize,
) -> Result<(), WriteStoreError> {
    let active_count = active_count(state);
    if active_count > active {
        return Err(WriteStoreError::Refused(format!(
            "recovered active write count exceeds bound: {active_count} > {active}"
        )));
    }
    let terminal_count = state
        .receipts
        .values()
        .filter(|receipt| receipt.is_terminal())
        .count();
    if terminal_count > terminal {
        return Err(WriteStoreError::Refused(format!(
            "recovered terminal receipt count exceeds bound: {terminal_count} > {terminal}"
        )));
    }
    Ok(())
}

pub(super) fn next_revision(state: &StoreState) -> Result<u64, WriteStoreError> {
    state
        .revision
        .checked_add(1)
        .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))
}

pub(super) fn active_count(state: &StoreState) -> usize {
    state
        .receipts
        .values()
        .filter(|receipt| !receipt.is_terminal())
        .count()
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
pub(super) struct UnsignedEventView<'a> {
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
