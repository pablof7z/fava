//! Exact author-bound event-signing contract.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_write::{Event, PublicKey, UnsignedEvent};
use thiserror::Error;
use tokio::sync::watch;

/// Current availability of one signer provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignerAvailability {
    /// Signer can currently accept work.
    Available,
    /// Signer remains configured but cannot currently accept work.
    Unavailable,
}

/// Replaceable provider that signs exact events for one public key.
pub trait Signer: Send + Sync {
    /// Public key this provider can sign for.
    fn public_key(&self) -> PublicKey;

    /// Current provider availability.
    fn availability(&self) -> SignerAvailability;

    /// Sign one exact unsigned event unless its owner cancels the operation.
    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>>;
}

/// Exact signer refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SignerError {
    /// Signer is currently unavailable.
    #[error("signer unavailable: {0}")]
    Unavailable(String),
    /// User or provider rejected signing.
    #[error("signer rejected event: {0}")]
    Rejected(String),
    /// Owning write was cancelled.
    #[error("signing cancelled")]
    Cancelled,
    /// Provider returned an invalid event.
    #[error("signer returned invalid event: {0}")]
    InvalidOutput(String),
}
