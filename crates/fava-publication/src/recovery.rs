//! Bounded semantic reconciliation before facade admission and runner effects.

use fava_write::{Receipt, ReceiptId};
use fava_write_store::{WriteStoreError, destination_evidence_capacity};
use tokio::sync::watch;

use super::edit_application::SemanticState;
use super::{Publication, PublicationError};

impl Publication {
    pub(super) async fn initialize_semantic(
        &self,
        mut receipt: Receipt,
        state: &mut SemanticState,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<Receipt> {
        for _ in 0..=destination_evidence_capacity() {
            receipt = self.refresh_semantic(receipt, state, cancel).await?;
            self.reapply(&receipt, state);
            let current = self.read_receipt(receipt.receipt_id, cancel).await?;
            if current.current.publication.revision_id == state.revision_id {
                return Some(current);
            }
            receipt = current;
        }
        self.record_activation_exhaustion(&receipt);
        None
    }

    pub(super) fn reconcile_recovered(
        &self,
        receipt_id: ReceiptId,
        state: &mut SemanticState,
    ) -> Result<(), PublicationError> {
        for _ in 0..=destination_evidence_capacity() {
            let receipt = self.store.receipt(receipt_id)?.ok_or_else(|| {
                WriteStoreError::Refused("recovered semantic receipt is missing".to_owned())
            })?;
            if receipt.is_terminal() {
                return Ok(());
            }
            if let Err(error) = self.refresh_recovered_custody(&receipt, state) {
                let current = self.store.receipt(receipt_id)?.ok_or_else(|| {
                    WriteStoreError::Refused("revalidated semantic receipt is missing".to_owned())
                })?;
                if current != receipt {
                    continue;
                }
                return Err(error);
            }
            let current = self.store.receipt(receipt_id)?.ok_or_else(|| {
                WriteStoreError::Refused("revalidated semantic receipt is missing".to_owned())
            })?;
            if current != receipt {
                continue;
            }
            self.reapply(&current, state);
            let applied = self.store.receipt(receipt_id)?.ok_or_else(|| {
                WriteStoreError::Refused("reconciled semantic receipt is missing".to_owned())
            })?;
            if applied.current.publication.revision_id == state.revision_id {
                return Ok(());
            }
        }
        Err(WriteStoreError::Refused(format!(
            "semantic recovery reconciliation exceeds bound {}",
            destination_evidence_capacity() + 1
        ))
        .into())
    }

    fn refresh_recovered_custody(
        &self,
        receipt: &Receipt,
        state: &mut SemanticState,
    ) -> Result<(), PublicationError> {
        let retry_persisted_failure = receipt.current.publication.revision_id
            == state.revision_id
            && state.failed_id.is_none();
        let Some((edits, author, selected, failed_id)) = self.store.applied_edits(
            receipt.receipt_id,
            receipt.current.publication.revision_id,
        )?
        else {
            return Err(WriteStoreError::Refused(
                "recovered semantic custody is missing".to_owned(),
            )
            .into());
        };
        if edits.is_empty() {
            return Err(WriteStoreError::Refused(
                "recovered semantic edit sequence is empty".to_owned(),
            )
            .into());
        }
        state.refresh_custody(
            receipt.current.publication.revision_id,
            edits,
            author,
            selected,
            failed_id,
        );
        // Assembly deliberately authorizes one retry of the failure already
        // persisted for the recovered generation. Exact custody refresh must
        // not accidentally suppress that existing recovery contract.
        if retry_persisted_failure {
            state.failed_id = None;
        }
        Ok(())
    }
}
