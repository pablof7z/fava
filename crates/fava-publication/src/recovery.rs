//! Bounded semantic reconciliation before facade admission and runner effects.

use fava_write::{Receipt, ReceiptId};
use fava_write_store::{WriteStoreError, destination_evidence_capacity};
use tokio::sync::watch;

use super::materialization::SemanticState;
use super::{Publication, PublicationError};

impl Publication {
    pub(super) async fn initialize_semantic(
        &self,
        mut receipt: Receipt,
        state: &mut SemanticState,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<Receipt> {
        for _ in 0..=destination_evidence_capacity() {
            if receipt.current.publication.materialization_id != state.materialization_id {
                receipt = self.refresh_semantic(receipt, state, cancel).await?;
            }
            self.rematerialize(&receipt, state);
            let current = self.read_receipt(receipt.receipt_id, cancel).await?;
            if current.current.publication.materialization_id == state.materialization_id {
                return Some(current);
            }
            receipt = current;
        }
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
            if receipt.current.publication.materialization_id != state.materialization_id {
                self.refresh_recovered_custody(&receipt, state)?;
            }
            self.rematerialize(&receipt, state);
            let current = self.store.receipt(receipt_id)?.ok_or_else(|| {
                WriteStoreError::Refused("reconciled semantic receipt is missing".to_owned())
            })?;
            if current.current.publication.materialization_id == state.materialization_id {
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
        let Some((edits, author, selected, failed_id)) = self.store.materialized_edits(
            receipt.receipt_id,
            receipt.current.publication.materialization_id,
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
            receipt.current.publication.materialization_id,
            edits,
            author,
            selected,
            failed_id,
        );
        Ok(())
    }
}
