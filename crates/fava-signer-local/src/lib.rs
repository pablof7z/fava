//! In-process Nostr secret-key signer.

use std::future::Future;
use std::pin::Pin;

use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, PublicKey, UnsignedEvent};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use tokio::sync::watch;

/// Local exact-event signer backed by one Nostr keypair.
pub struct LocalSigner {
    keys: Keys,
}

impl LocalSigner {
    /// Take custody of one in-process keypair.
    #[must_use]
    pub const fn new(keys: Keys) -> Self {
        Self { keys }
    }
}

impl Signer for LocalSigner {
    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        &self,
        event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        Box::pin(async move {
            if event.pubkey != self.keys.public_key() {
                return Err(SignerError::InvalidOutput(
                    "unsigned event author does not match signer".to_owned(),
                ));
            }
            tokio::select! {
                biased;
                changed = cancel.changed() => {
                    let _ = changed;
                    Err(SignerError::Cancelled)
                }
                result = async { event.finalize(&self.keys) } => {
                    result.map_err(|error| SignerError::InvalidOutput(error.to_string()))
                }
            }
        })
    }
}
