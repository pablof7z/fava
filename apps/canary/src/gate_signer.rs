//! An application-owned signer that asks a human before it signs.
//!
//! This is a real provider role, not a stub: a hardware wallet, a NIP-07
//! extension, or a NIP-46 bunker all behave this way. It exists so a scenario
//! can observe the optimistic local state Fava exposes between acceptance and
//! signature.
//!
//! Implementing it requires naming `fava_signer::Signer`, which the `fava`
//! facade does not re-export. That is recorded as a wall, not routed around.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fava::{Event, PublicKey, UnsignedEvent};
use fava_signer::{Signer, SignerAvailability, SignerError};
use nostr::event::UnsignedEvent as NostrUnsignedEvent;
use nostr::key::Keys;
use secp256k1::Secp256k1;
use tokio::sync::{mpsc, oneshot, watch};

use crate::{CanaryError, CanaryResult};

const SIGN_REQUEST_CAPACITY: usize = 2;

type SignResponse = oneshot::Sender<Result<Event, SignerError>>;

/// One signing request awaiting an out-of-band decision.
pub(crate) struct PendingSign {
    pub(crate) event: UnsignedEvent,
    response: SignResponse,
}

impl PendingSign {
    pub(crate) fn complete(self, event: Event) -> CanaryResult<()> {
        self.response
            .send(Ok(event))
            .map_err(|_| CanaryError::new("signing generation was no longer awaiting completion"))
    }
}

/// A signer whose signatures are released by the application, not immediately.
pub(crate) struct GateSigner {
    public_key: PublicKey,
    requests: mpsc::Sender<PendingSign>,
}

impl GateSigner {
    pub(crate) fn new(public_key: PublicKey) -> (Self, mpsc::Receiver<PendingSign>) {
        let (requests, receiver) = mpsc::channel(SIGN_REQUEST_CAPACITY);
        (
            Self {
                public_key,
                requests,
            },
            receiver,
        )
    }
}

impl Signer for GateSigner {
    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(async move {
            let (response, completion) = oneshot::channel();
            self.requests
                .try_send(PendingSign { event, response })
                .map_err(|error| {
                    SignerError::Unavailable(format!("signing request refused: {error}"))
                })?;
            completion
                .await
                .map_err(|_| SignerError::Unavailable("completion channel closed".to_owned()))?
        })
    }
}

/// Await the next signing request within a bounded deadline.
pub(crate) async fn next_sign(
    requests: &mut mpsc::Receiver<PendingSign>,
) -> CanaryResult<PendingSign> {
    tokio::time::timeout(Duration::from_secs(5), requests.recv())
        .await
        .map_err(|_| CanaryError::new("timed out awaiting materialization signing"))?
        .ok_or_else(|| CanaryError::new("signing request channel closed"))
}

/// Produce a byte-stable signature so a scenario can be run twice and compared.
pub(crate) fn deterministic_finalize(
    mut event: NostrUnsignedEvent,
    keys: &Keys,
) -> Result<Event, SignerError> {
    if event.pubkey != keys.public_key() {
        return Err(SignerError::InvalidOutput(
            "unsigned event author does not match signer".to_owned(),
        ));
    }
    let id = event.id.unwrap_or_else(|| event.compute_id());
    event.id = Some(id);
    let signature = keys.sign_schnorr_with_aux_rand(&Secp256k1::new(), id.as_bytes(), &[0; 32]);
    event
        .add_signature(signature)
        .map_err(|error| SignerError::InvalidOutput(error.to_string()))
}
