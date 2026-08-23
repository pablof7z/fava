//! Exact signer attachment selection and completion admission.

use fava_signer::{SignerAvailability, SignerError};
use fava_write::{EventId, EventValue, MaterializationId, Receipt, ReceiptId, WriteId};
use tokio::sync::watch;

use crate::Publication;

/// Bound on retained stale signer-completion evidence.
const STALE_COMPLETION_CAPACITY: usize = 256;

impl Publication {
    /// Bounded late signer completions this owner rejected, newest last.
    ///
    /// Each fact names the receipt, write, materialization generation, event,
    /// and exact reason. A completion is stale when the signer attachment that
    /// produced it is no longer current, or when the write store no longer
    /// treats that materialization generation as current. Without this evidence
    /// a late completion is indistinguishable from a signer that never answered.
    #[must_use]
    #[allow(clippy::type_complexity)] // Existing identity values, not a new noun.
    pub fn stale_signer_completions(
        &self,
    ) -> Vec<(ReceiptId, WriteId, MaterializationId, EventId, String)> {
        self.stale_signer_completions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }

    fn reject_stale_completion(
        &self,
        receipt_id: ReceiptId,
        write_id: WriteId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) {
        let mut retained = self
            .stale_signer_completions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if retained.len() == STALE_COMPLETION_CAPACITY {
            retained.pop_front();
        }
        retained.push_back((receipt_id, write_id, materialization_id, event_id, reason));
    }

    pub(super) fn signer_generation(&self, receipt: &Receipt) -> Option<u64> {
        let EventValue::Unsigned(unsigned) = &receipt.current.event else {
            return None;
        };
        self.session
            .signer(unsigned.pubkey)
            .map(|(generation, _)| generation)
    }

    pub(super) fn start_signing(
        &self,
        receipt: &Receipt,
        cancel: watch::Receiver<bool>,
    ) -> Option<u64> {
        let EventValue::Unsigned(unsigned) = receipt.current.event.clone() else {
            return None;
        };
        let public_key = unsigned.pubkey;
        let (signer_generation, signer) = self.session.signer(public_key)?;
        if !matches!(signer.availability(), SignerAvailability::Available) {
            return Some(signer_generation);
        }
        let publication = self.clone();
        let write_id = receipt.write_id;
        let receipt_id = receipt.receipt_id;
        let materialization_id = receipt.current.publication.materialization_id;
        let event_id = receipt.current.id();
        tokio::spawn(async move {
            let completion = signer.sign_event(unsigned, cancel).await;
            if !publication
                .session
                .is_current(public_key, signer_generation)
            {
                publication.reject_stale_completion(
                    receipt_id,
                    write_id,
                    materialization_id,
                    event_id,
                    format!(
                        "signer attachment generation {signer_generation} for {public_key} \
                         was retired before its completion arrived"
                    ),
                );
                return;
            }
            let refusal = match completion {
                Ok(event) => {
                    match publication.store.install_signed(
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        event,
                    ) {
                        Ok(_) => return,
                        // The store owns currentness, so its exact refusal is the
                        // only accurate account of why the signature was rejected.
                        Err(error) => error.to_string(),
                    }
                }
                Err(SignerError::Cancelled) => return,
                Err(error) => error.to_string(),
            };
            if let Err(stale) = publication.store.record_signer_refusal(
                write_id,
                receipt_id,
                materialization_id,
                event_id,
                refusal.clone(),
            ) {
                publication.reject_stale_completion(
                    receipt_id,
                    write_id,
                    materialization_id,
                    event_id,
                    format!("signer completion rejected ({refusal}); evidence refused ({stale})"),
                );
            }
        });
        Some(signer_generation)
    }
}
