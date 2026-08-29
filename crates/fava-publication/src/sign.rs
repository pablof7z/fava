//! Exact signer attachment selection and completion admission.

use fava_signer::{SignerAvailability, SignerError};
use fava_write::{
    Event, EventId, EventValue, RevisionId, Receipt, ReceiptId, SignatureState, WriteId,
};
use tokio::sync::watch;

use crate::Publication;

/// Bound on retained stale signer-completion evidence.
const STALE_COMPLETION_CAPACITY: usize = 256;

impl Publication {
    /// Bounded late signer completions this owner rejected, newest last.
    ///
    /// Each fact names the receipt, write, revision generation, event,
    /// and exact reason. A completion is stale when the signer attachment that
    /// produced it is no longer current, or when the write store no longer
    /// treats that revision generation as current. Without this evidence
    /// a late completion is indistinguishable from a signer that never answered.
    #[must_use]
    #[allow(clippy::type_complexity)] // Existing identity values, not a new noun.
    pub fn stale_signer_completions(
        &self,
    ) -> Vec<(ReceiptId, WriteId, RevisionId, EventId, String)> {
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
        revision_id: RevisionId,
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
        retained.push_back((receipt_id, write_id, revision_id, event_id, reason));
    }

    pub(super) fn signer_generation(&self, receipt: &Receipt) -> Option<u64> {
        let EventValue::Unsigned(unsigned) = &receipt.current.event else {
            return None;
        };
        self.session
            .signer(unsigned.pubkey)
            .map(|(generation, _availability)| generation)
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
        let (signer_generation, availability) = self.session.signer(public_key)?;
        if !matches!(availability, SignerAvailability::Available) {
            return Some(signer_generation);
        }
        let write_id = receipt.write_id;
        let receipt_id = receipt.receipt_id;
        let revision_id = receipt.current.publication.revision_id;
        let event_id = receipt.current.id();
        let authorized = self
            .store
            .authorize_signing(write_id, receipt_id, revision_id, event_id)
            .ok()?;
        if !matches!(
            authorized.current.publication.signature,
            SignatureState::Authorized
        ) {
            return None;
        }
        let Some(signing) =
            self.session
                .invoke_signer(public_key, signer_generation, unsigned, cancel)
        else {
            if let Err(error) = self.store.record_signer_retryable(
                authorized.write_id,
                authorized.receipt_id,
                authorized.current.publication.revision_id,
                authorized.current.id(),
                format!(
                    "authorized signer attachment generation {signer_generation} for {public_key} was retired before provider invocation; retry is permitted"
                ),
            ) {
                self.reject_stale_completion(
                    authorized.receipt_id,
                    authorized.write_id,
                    authorized.current.publication.revision_id,
                    authorized.current.id(),
                    format!("retired pre-invocation transition was rejected: {error}"),
                );
            }
            return Some(signer_generation);
        };
        let publication = self.clone();
        tokio::spawn(async move {
            let completion = signing.await;
            publication.finish_signing(&authorized, public_key, signer_generation, completion);
        });
        Some(signer_generation)
    }

    pub(super) fn cancel_authorized_signing(
        &self,
        receipt: &Receipt,
        signer_generation: Option<u64>,
        cancel: &watch::Sender<bool>,
        cause: &str,
    ) {
        if matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            let write_id = receipt.write_id;
            let receipt_id = receipt.receipt_id;
            let revision_id = receipt.current.publication.revision_id;
            let event_id = receipt.current.id();
            let generation = signer_generation
                .map_or_else(|| "unknown".to_owned(), |generation| generation.to_string());
            let reason = format!(
                "authorized signer operation for write {} receipt {} revision {} event {} attachment generation {generation} cancelled before effect because {cause}; retry is permitted",
                write_id.as_u64(),
                receipt_id.as_u64(),
                revision_id.as_u64(),
                event_id,
            );
            if let Err(error) = self.store.record_signer_retryable(
                write_id,
                receipt_id,
                revision_id,
                event_id,
                reason,
            ) {
                self.reject_stale_completion(
                    receipt_id,
                    write_id,
                    revision_id,
                    event_id,
                    format!("authorized cancellation transition was rejected: {error}"),
                );
            }
        }
        cancel.send_replace(true);
    }

    fn finish_signing(
        &self,
        receipt: &Receipt,
        public_key: fava_write::PublicKey,
        signer_generation: u64,
        completion: Result<Event, SignerError>,
    ) {
        let write_id = receipt.write_id;
        let receipt_id = receipt.receipt_id;
        let revision_id = receipt.current.publication.revision_id;
        let event_id = receipt.current.id();
        if !self.session.is_current(public_key, signer_generation) {
            self.reject_stale_completion(
                receipt_id,
                write_id,
                revision_id,
                event_id,
                format!(
                    "signer attachment generation {signer_generation} for {public_key} \
                     was retired before its completion arrived"
                ),
            );
            return;
        }
        let refusal = match completion {
            Ok(event) => match self.store.install_signed(
                write_id,
                receipt_id,
                revision_id,
                event_id,
                event,
            ) {
                Ok(_) => return,
                // The store owns currentness, so its exact refusal is the only
                // accurate account of why the signature was rejected.
                Err(error) => error.to_string(),
            },
            Err(SignerError::Cancelled) => {
                let _ = self.store.record_signer_retryable(
                    write_id,
                    receipt_id,
                    revision_id,
                    event_id,
                    format!(
                        "authorized signer operation for write {} receipt {} revision {} event {} attachment generation {signer_generation} cancelled before effect; retry is permitted",
                        write_id.as_u64(),
                        receipt_id.as_u64(),
                        revision_id.as_u64(),
                        event_id,
                    ),
                );
                return;
            }
            Err(SignerError::Unavailable(reason)) => {
                let _ = self.store.record_signer_retryable(
                    write_id,
                    receipt_id,
                    revision_id,
                    event_id,
                    format!("signer unavailable before effect: {reason}; retry is permitted"),
                );
                return;
            }
            Err(error) => error.to_string(),
        };
        if let Err(stale) = self.store.record_signer_refusal(
            write_id,
            receipt_id,
            revision_id,
            event_id,
            refusal.clone(),
        ) {
            self.reject_stale_completion(
                receipt_id,
                write_id,
                revision_id,
                event_id,
                format!("signer completion rejected ({refusal}); evidence refused ({stale})"),
            );
        }
    }
}
