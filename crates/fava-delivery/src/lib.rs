//! Replaceable bounded delivery-decision contract.

use std::time::Duration;

use fava_write::RelayDeliveryOutcome;

/// Durable facts available for one destination decision.
#[derive(Clone, Copy, Debug)]
pub struct DeliveryFacts<'a> {
    /// Attempts that actually reached a relay and were therefore spent.
    ///
    /// Time this destination spent offline or unreachable is not counted here.
    pub attempts: u32,
    /// Exact current destination fact.
    pub outcome: &'a RelayDeliveryOutcome,
}

/// One policy decision for current durable destination facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeliveryDecision {
    /// Authorize one attempt now.
    AttemptNow,
    /// Park this destination for exactly this long, then decide again.
    ///
    /// Waiting is not an attempt and spends no budget. The caller wakes early
    /// on cancellation.
    WaitFor(Duration),
    /// Stop retrying with an exact policy reason.
    GiveUp {
        /// Exact policy reason.
        reason: String,
    },
    /// Current destination is already settled or in flight.
    Settled,
}

/// Replaceable policy deciding attempts from durable facts.
pub trait DeliveryPolicy: Send + Sync {
    /// Decide the next action without performing effects or retaining a ledger.
    fn decide(&self, facts: DeliveryFacts<'_>) -> DeliveryDecision;
}
