use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava::{Event, PublicKey, UnsignedEvent};
use fava_signer::{Signer, SignerAvailability, SignerError};
use tokio::sync::{mpsc, oneshot, watch};

pub(crate) type PendingSignature = (UnsignedEvent, oneshot::Sender<Result<Event, SignerError>>);

pub(crate) struct GatedSigner {
    public_key: PublicKey,
    pending: mpsc::Sender<PendingSignature>,
}

impl GatedSigner {
    pub(crate) fn new(public_key: PublicKey) -> (Self, mpsc::Receiver<PendingSignature>) {
        let (pending, requests) = mpsc::channel(2);
        (
            Self {
                public_key,
                pending,
            },
            requests,
        )
    }
}

impl Signer for GatedSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(async move {
            let (complete, response) = oneshot::channel();
            self.pending
                .send((event, complete))
                .await
                .map_err(|error| {
                    SignerError::Unavailable(format!("signature gate closed: {error}"))
                })?;
            response
                .await
                .map_err(|_| SignerError::Unavailable("completion dropped".to_owned()))?
        })
    }
}
