//! One-attempt event publication contract.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use fava_state::RelaySessionKey;
use fava_transport::Transport;
use fava_write::{Event, MaterializationId, ReceiptId, WriteId};

/// One exact publication attempt at one exact destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishAttempt {
    /// Stable accepted-write identity.
    pub write_id: WriteId,
    /// Stable receipt identity.
    pub receipt_id: ReceiptId,
    /// Exact immutable materialization generation being published.
    pub materialization_id: MaterializationId,
    /// One-based durable attempt count for this destination.
    pub number: u32,
    /// Exact relay and access destination.
    pub session: RelaySessionKey,
    /// Exact signed event bytes and identity.
    pub event: Event,
    /// Maximum time this one attempt may remain unresolved.
    pub timeout: Duration,
}

/// Exact result of one publisher attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    /// Relay accepted the event with this exact bounded message.
    Acknowledged {
        /// Exact bounded relay message.
        message: String,
    },
    /// Relay rejected the event with this exact bounded message.
    Rejected {
        /// Exact bounded relay message.
        message: String,
    },
    /// Relay access was required and not granted for this exact attempt.
    AuthenticationDenied {
        /// Exact scoped authentication reason.
        reason: String,
    },
    /// Bytes definitely were not handed to transport.
    NotHandedOff {
        /// Exact definite failure reason.
        reason: String,
    },
    /// Handoff or later outcome cannot be proven.
    OutcomeUnknown {
        /// Exact ambiguity reason.
        reason: String,
    },
}

/// Replaceable mechanism performing one exact publication attempt.
pub trait Publisher: Send + Sync {
    /// Perform exactly one attempt without selecting retry or destination policy.
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>>;
}
