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
            | RelayDeliveryOutcome::AuthenticationDenied { .. }
            | RelayDeliveryOutcome::GivenUp { .. }
            | RelayDeliveryOutcome::Unknown { .. }
            | RelayDeliveryOutcome::CancelledBeforeHandoff => DeliveryDecision::Settled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_definite_pre_handoff_failure_until_exact_ceiling() {
        let policy = StandardDeliveryPolicy::new(NonZeroU32::new(2).unwrap());
        let retryable = RelayDeliveryOutcome::Retryable {
            reason: "offline".to_owned(),
        };
        assert_eq!(
            policy.decide(DeliveryFacts {
                attempts: 1,
                outcome: &retryable,
            }),
            DeliveryDecision::AttemptNow
        );
        assert_eq!(
            policy.decide(DeliveryFacts {
                attempts: 2,
                outcome: &retryable,
            }),
            DeliveryDecision::GiveUp {
                reason: "attempt ceiling 2 reached after: offline".to_owned(),
            }
        );
    }

    #[test]
    fn authentication_denial_is_terminal_and_distinct_from_a_give_up() {
        let policy = StandardDeliveryPolicy::new(NonZeroU32::new(3).unwrap());
        let denied = RelayDeliveryOutcome::AuthenticationDenied {
            reason: "relay demanded authentication this attempt did not satisfy".to_owned(),
        };
        assert_eq!(
            policy.decide(DeliveryFacts {
                attempts: 1,
                outcome: &denied,
            }),
            DeliveryDecision::Settled,
            "this policy does not arrange authentication, so it stops without retrying"
        );
    }

    #[test]
    fn ambiguous_handoff_is_terminal_for_the_standard_policy() {
        let policy = StandardDeliveryPolicy::default();
        let unknown = RelayDeliveryOutcome::Unknown {
            reason: "connection ended after handoff".to_owned(),
        };
        assert_eq!(
            policy.decide(DeliveryFacts {
                attempts: 1,
                outcome: &unknown,
            }),
            DeliveryDecision::Settled
        );
    }
}
