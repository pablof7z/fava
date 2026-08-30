//! Exact durable semantic custody recovery reads.

use fava_write::{EventEdit, EventId, PublicKey, Receipt, ReceiptId, RevisionId, Timestamp};
use fava_write_store::WriteStoreError;

use crate::RedbWriteStore;

impl RedbWriteStore {
    #[allow(clippy::type_complexity)]
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
        let state = self.lock()?;
        Ok(state
            .semantics
            .iter()
            .filter_map(|(receipt_id, (edit, author, source, failed_source, _))| {
                state.receipts.get(receipt_id).and_then(|receipt| {
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
        let state = self.lock()?;
        let Some(receipt) = state.receipts.get(&receipt_id) else {
            return Ok(None);
        };
        if receipt.current.publication.revision_id != expected {
            return Err(WriteStoreError::Refused(
                "semantic custody generation is not current".to_owned(),
            ));
        }
        Ok(state
            .semantics
            .get(&receipt_id)
            .map(|(edits, author, source, failed, _)| (edits.clone(), *author, *source, *failed)))
    }
}
