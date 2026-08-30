//! Exact semantic custody recovery reads.

use fava_write::{EventEdit, EventId, PublicKey, Receipt, ReceiptId, RevisionId, Timestamp};
use fava_write_store::WriteStoreError;

use crate::MemoryWriteStore;

impl MemoryWriteStore {
    #[allow(clippy::type_complexity)] // Existing values deliberately avoid a recovery wrapper.
    pub(super) fn recover_semantic(
        &self,
    ) -> Result<
        Vec<(
            Receipt,
            Vec<EventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        let state = self.lock_state()?;
        Ok(state
            .edits
            .iter()
            .filter_map(|(receipt_id, (edit, author, source, failed_source))| {
                state.writes.get(receipt_id).and_then(|receipt| {
                    (!receipt.is_terminal()).then(|| {
                        (
                            receipt.clone(),
                            edit.clone(),
                            *author,
                            *source,
                            *failed_source,
                        )
                    })
                })
            })
            .collect())
    }

    #[allow(clippy::type_complexity)]
    pub(super) fn semantic_custody(
        &self,
        receipt_id: ReceiptId,
        expected: RevisionId,
    ) -> Result<
        Option<(
            Vec<EventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        let state = self.lock_state()?;
        let Some(receipt) = state.writes.get(&receipt_id) else {
            return Ok(None);
        };
        if receipt.current.publication.revision_id != expected {
            return Err(WriteStoreError::Refused(
                "semantic custody generation is not current".to_owned(),
            ));
        }
        Ok(state.edits.get(&receipt_id).cloned())
    }
}
