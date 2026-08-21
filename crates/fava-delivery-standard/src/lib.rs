//! Standard finite-attempt delivery policy.

use std::num::NonZeroU32;

use fava_delivery::{DeliveryDecision, DeliveryFacts, DeliveryPolicy};
use fava_write::RelayDeliveryOutcome;

/// Standard policy with one exact per-destination attempt ceiling.
pub struct StandardDeliveryPolicy {
    maximum_attempts: NonZeroU32,
}

impl StandardDeliveryPolicy {
    /// Configure the finite per-destination attempt ceiling.
    #[must_use]
    pub const fn new(maximum_attempts: NonZeroU32) -> Self {
        Self { maximum_attempts }
    }
}

impl Default for StandardDeliveryPolicy {
    fn default() -> Self {
        Self::new(NonZeroU32::MIN)
    }
}

impl DeliveryPolicy for StandardDeliveryPolicy {
    fn decide(&self, facts: DeliveryFacts<'_>) -> DeliveryDecision {
        match facts.outcome {
            RelayDeliveryOutcome::Pending | RelayDeliveryOutcome::Retryable { .. }
                if facts.attempts < self.maximum_attempts.get() =>
            {
                DeliveryDecision::AttemptNow
            }
            RelayDeliveryOutcome::Retryable { reason } => DeliveryDecision::GiveUp {
                reason: format!(
                    "attempt ceiling {} reached after: {reason}",
                    self.maximum_attempts
                ),
            },
            RelayDeliveryOutcome::Pending
            | RelayDeliveryOutcome::Attempting
            | RelayDeliveryOutcome::Acknowledged { .. }
            | RelayDeliveryOutcome::Rejected { .. }
            | RelayDeliveryOutcome::GivenUp { .. }
            | RelayDeliveryOutcome::Unknown { .. }
            | RelayDeliveryOutcome::CancelledBeforeHandoff => DeliveryDecision::Settled,
        }
    }
}
