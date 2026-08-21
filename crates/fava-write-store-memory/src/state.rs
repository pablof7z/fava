use fava_write::{EventId, MaterializationId, ReceiptId};
use fava_write_store::WriteStoreError;

use crate::semantic::WriteState;

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

pub(super) fn release_semantic(state: &mut WriteState, receipt_id: ReceiptId) {
    if let Some((edit, _, _)) = state.edits.remove(&receipt_id) {
        state.coordinates.remove(edit.coordinate());
    }
}
