//! Standard finite-attempt delivery policy.

use std::num::NonZeroU32;
use std::time::Duration;

use fava_delivery::{DeliveryDecision, DeliveryFacts, DeliveryPolicy};
use fava_write::RelayDeliveryOutcome;

/// Interval a destination stays parked while no connection can be established.
pub const DEFAULT_UNREACHABLE_RETRY_AFTER: Duration = Duration::from_secs(5);

/// Standard policy with one exact per-destination attempt ceiling.
///
/// The ceiling counts attempts that actually reached a relay. A destination
/// that is offline or unreachable spends no budget: it is parked for the
/// configured interval and reconsidered, so being unreachable can never be
/// converted into a delivery failure by the passage of time alone.
pub struct StandardDeliveryPolicy {
    maximum_attempts: NonZeroU32,
    unreachable_retry_after: Duration,
}

impl StandardDeliveryPolicy {
    /// Configure the finite per-destination attempt ceiling.
    #[must_use]
    pub const fn new(maximum_attempts: NonZeroU32) -> Self {
        Self {
            maximum_attempts,
            unreachable_retry_after: DEFAULT_UNREACHABLE_RETRY_AFTER,
        }
    }

    /// Configure how long an unreachable destination stays parked.
    #[must_use]
    pub const fn retrying_unreachable_after(mut self, interval: Duration) -> Self {
        self.unreachable_retry_after = interval;
        self
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
            RelayDeliveryOutcome::Unreachable { .. } => {
                DeliveryDecision::WaitFor(self.unreachable_retry_after)
            }
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
            | RelayDeliveryOutcome::RefusedByLimit { .. }
            | RelayDeliveryOutcome::AuthenticationDenied { .. }
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
            reason: "handoff refused".to_owned(),
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
                reason: "attempt ceiling 2 reached after: handoff refused".to_owned(),
            }
        );
    }

    #[test]
    fn an_unreachable_destination_waits_and_never_reaches_the_ceiling() {
        let policy = StandardDeliveryPolicy::new(NonZeroU32::new(2).unwrap())
            .retrying_unreachable_after(Duration::from_millis(25));
        let unreachable = RelayDeliveryOutcome::Unreachable {
            reason: "connection refused".to_owned(),
        };
        for spent in [0, 2, 1_000] {
            assert_eq!(
                policy.decide(DeliveryFacts {
                    attempts: spent,
                    outcome: &unreachable,
                }),
                DeliveryDecision::WaitFor(Duration::from_millis(25)),
                "unreachable time never becomes a spent attempt"
            );
        }
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
