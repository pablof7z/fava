//! Bounded reconnect policy primitive.

use std::time::Duration;

use thiserror::Error;

/// Bounded reconnect schedule with growth, ceiling, jitter, and attempt bound.
#[derive(Clone, Debug)]
pub struct Backoff {
    _placeholder: (),
}

/// Typed shortfall reported when a reconnect policy exhausts its attempt bound.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
#[error("reconnect exhausted after {attempts} attempts at a {ceiling:?} ceiling")]
pub struct BackoffShortfall {
    attempts: u32,
    ceiling: Duration,
}

impl BackoffShortfall {
    /// Attempts spent before the policy gave up.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Ceiling the policy had reached.
    #[must_use]
    pub fn ceiling(&self) -> Duration {
        self.ceiling
    }
}

impl Backoff {
    /// Declare a reconnect policy.
    #[must_use]
    pub fn new(_initial: Duration, _ceiling: Duration, _max_attempts: u32) -> Self {
        todo!()
    }

    /// Multiply each successive delay by this factor.
    #[must_use]
    pub fn with_growth(self, _factor: u32) -> Self {
        todo!()
    }

    /// Subtract up to `percent` of each delay using a seeded deterministic dither.
    #[must_use]
    pub fn with_jitter(self, _percent: u8, _seed: u64) -> Self {
        todo!()
    }

    /// Attempts already handed out.
    #[must_use]
    pub fn attempts(&self) -> u32 {
        todo!()
    }

    /// Ceiling this policy will not exceed.
    #[must_use]
    pub fn ceiling(&self) -> Duration {
        todo!()
    }

    /// Delay before the next attempt, or the typed shortfall at the bound.
    pub fn next_delay(&mut self) -> Result<Duration, BackoffShortfall> {
        todo!()
    }

    /// Return to the initial delay and zero attempts.
    pub fn reset(&mut self) {
        todo!()
    }
}
