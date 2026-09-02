//! One-attempt event publication contract.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use fava_relay::Authority;
use fava_transport::Transport;
use fava_write::{Event, ReceiptId, RevisionId, WriteId};
use nostr::types::RelayUrl;

/// One exact publication attempt at one exact destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishAttempt {
    /// Stable accepted-write identity.
    pub write_id: WriteId,
    /// Stable receipt identity.
    pub receipt_id: ReceiptId,
    /// Exact immutable revision generation being published.
    pub revision_id: RevisionId,
    /// One-based durable attempt count for this destination.
    pub number: u32,
    /// Exact relay destination.
    pub session: RelayUrl,
    /// Authority this write was accepted under, and so the connection this
    /// attempt must reach.
    pub authority: Authority,
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
    /// Relay requires authentication not supplied by this attempt, with this
    /// exact bounded message.
    AuthenticationRequired {
        /// Exact bounded relay message, carrying its `auth-required:` prefix.
        message: String,
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
