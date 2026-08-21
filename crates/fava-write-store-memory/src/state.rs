use fava_state::EventCoordinate;
use fava_write::{
    EventId, MaterializationId, PublicKey, ReceiptId, ReplaceableEventEdit, Timestamp,
};
use fava_write_store::WriteStoreError;

use crate::semantic::WriteState;

pub(super) fn edit_coordinate(edit: &ReplaceableEventEdit, author: PublicKey) -> EventCoordinate {
    EventCoordinate::Replaceable {
        author,
        kind: edit.kind(),
        identifier: edit.identifier().map(str::to_owned),
    }
}

pub(super) fn attributed_failure(
    materialization_id: MaterializationId,
    source: Option<EventId>,
    reason: String,
) -> String {
    let source = source.map_or_else(|| "empty state".to_owned(), |id| id.to_string());
    let prefix = format!(
        "materialization {} from source {source} failed",
        materialization_id.as_u64()
    );
    let attributed = format!("{prefix}: {reason}");
    drop(reason);
    if fava_write_store::validate_receipt_text(&attributed).is_ok() {
        attributed
    } else {
        prefix
    }
}

pub(super) fn next_revision(state: &WriteState) -> Result<u64, WriteStoreError> {
    state
        .revision
        .checked_add(1)
        .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))
}

pub(super) fn active_count(state: &WriteState) -> usize {
    state
        .writes
        .values()
        .filter(|receipt| !receipt.is_terminal())
        .count()
}

pub(super) fn capacity_reached(state: &WriteState, capacity: usize) -> bool {
    active_count(state)
        .checked_add(state.reservations.len())
        .is_none_or(|used| used >= capacity)
}

pub(super) fn release_semantic(state: &mut WriteState, receipt_id: ReceiptId) {
    if let Some((edit, author, _, _)) = state.edits.remove(&receipt_id) {
        state.coordinates.remove(&edit_coordinate(&edit, author));
    }
}

pub(super) fn require_qualified_source(
    current: Option<(EventId, Timestamp)>,
    candidate: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    let qualified = match (current, candidate) {
        (None, Some(_)) | (Some(_), None) => true,
        (Some((current_id, current_time)), Some((candidate_id, candidate_time))) => {
            candidate_time > current_time
                || (candidate_time == current_time && candidate_id < current_id)
        }
        (None, None) => false,
    };
    if qualified {
        Ok(())
    } else {
        Err(WriteStoreError::Refused(
            "source event is equal, older, or already consumed".to_owned(),
        ))
    }
}

pub(super) fn require_failure_source(
    current: Option<(EventId, Timestamp)>,
    failed: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    if current == failed {
        Ok(())
    } else {
        require_qualified_source(current, failed)
    }
}
