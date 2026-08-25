//! Generation-qualified durable semantic custody refresh.

use fava_write::Receipt;
use tokio::sync::watch;

use super::materialization::SemanticState;
use super::{Publication, STORE_READ_RETRY_DELAY};

impl Publication {
    pub(super) async fn refresh_semantic(
        &self,
        mut receipt: Receipt,
        state: &mut SemanticState,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<Receipt> {
        loop {
            if *cancel.borrow() {
                return None;
            }
            match self.store.materialized_edits(
                receipt.receipt_id,
                receipt.current.publication.materialization_id,
            ) {
                Ok(Some((edits, author, selected, failed_id))) if !edits.is_empty() => {
                    state.edits = edits;
                    state.author = author;
                    state.selected_id = selected.map(|(id, _)| id);
                    if let Some((_, timestamp)) = selected {
                        state.source_floor = Some(timestamp);
                    }
                    state.failed_id = failed_id;
                    return Some(receipt);
                }
                Ok(Some(_) | None) => return None,
                Err(_) => {
                    tokio::select! {
                        changed = cancel.changed() => {
                            if changed.is_err() || *cancel.borrow_and_update() {
                                return None;
                            }
                        }
                        () = tokio::time::sleep(STORE_READ_RETRY_DELAY) => {}
                    }
                    receipt = self.read_receipt(receipt.receipt_id, cancel).await?;
                    if receipt.is_terminal() {
                        return None;
                    }
                }
            }
        }
    }
}
