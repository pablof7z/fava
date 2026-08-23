//! Exact signer attachment selection and completion admission.

use fava_signer::{SignerAvailability, SignerError};
use fava_write::{EventValue, Receipt};
use tokio::sync::watch;

use crate::Publication;

impl Publication {
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
                return;
            }
            match completion {
                Ok(event) => {
                    if publication
                        .store
                        .install_signed(write_id, receipt_id, materialization_id, event_id, event)
                        .is_err()
                    {
                        let _ = publication.store.record_signer_refusal(
                            write_id,
                            receipt_id,
                            materialization_id,
                            event_id,
                            "signer returned an event that did not match the accepted body"
                                .to_owned(),
                        );
                    }
                }
                Err(SignerError::Cancelled) => {}
                Err(error) => {
                    let _ = publication.store.record_signer_refusal(
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        error.to_string(),
                    );
                }
            }
        });
        Some(signer_generation)
    }
}
