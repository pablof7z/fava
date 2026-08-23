//! Current facts about writes that have not settled.

use std::time::Duration;

use fava_query::BoundedText;

/// One write that has not settled.
///
/// Authority: GOALS:1419-1429 (OPS-003).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteDiagnostic {
    /// Receipt identity, rendered by the write-store owner.
    pub receipt: BoundedText,
    /// Single classification of why it is stuck.
    pub classification: WriteStall,
    /// How long it has been in this classification.
    pub stuck_for: Duration,
}

/// The one classification a stalled write carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteStall {
    /// No route has been resolved yet.
    Unrouted,
    /// No signer is available for the required author.
    Unsignable,
    /// Routed and signed, awaiting handoff.
    AwaitingDelivery,
    /// Delivery attempts are being retried.
    Retrying {
        /// Attempts made.
        attempts: usize,
    },
    /// Delivery is exhausted and no further attempt is scheduled.
    Undeliverable {
        /// Bounded reason.
        detail: BoundedText,
    },
}
