//! Replaceable relay-session transport contracts.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_state::RelaySessionKey;
use thiserror::Error;

/// Correlated result of attempting to hand one exact frame to a relay session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffOutcome {
    /// Bytes definitely did not leave Fava.
    NotHandedOff {
        /// Exact local refusal reason.
        reason: String,
    },
    /// The transport accepted the complete frame for the session.
    HandedOff,
    /// The transport cannot prove whether the relay received the frame.
    Ambiguous {
        /// Exact ambiguity reason.
        reason: String,
    },
}

/// Replaceable owner of relay-session connection resources.
pub trait Transport: Send + Sync {
    /// Open a fresh session generation for one relay-access identity.
    #[allow(clippy::type_complexity)] // Inline future keeps this contract object-safe without another public noun.
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>;
}

/// One exact live connection generation to a relay.
pub trait RelaySession: Send + Sync {
    /// Relay and access identity owned by this session.
    fn key(&self) -> &RelaySessionKey;

    /// Monotonic transport-owned connection generation.
    fn generation(&self) -> u64;

    /// Attempt to hand off one complete text frame.
    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>>;

    /// Await the next complete relay text frame.
    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>>;

    /// Close this exact session generation. Repeated close is harmless.
    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>>;
}

/// Scoped transport operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TransportError {
    /// A session could not be established before any handoff.
    #[error("relay session open refused: {0}")]
    ConnectionRefused(String),
    /// A previously open session disconnected.
    #[error("relay session disconnected: {0}")]
    Disconnected(String),
    /// The session is already closed.
    #[error("relay session is closed")]
    Closed,
    /// An inbound frame was not a valid NIP-01 text frame.
    #[error("relay supplied an invalid frame: {0}")]
    InvalidFrame(String),
}
