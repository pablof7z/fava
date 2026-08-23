//! Deadline-wrapped provider invocation and correlated completions.

use std::marker::PhantomData;
use std::time::Duration;

use thiserror::Error;

/// Monotonic identity of one owner operation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Generation(u64);

impl Generation {
    /// First generation of any operation.
    pub const FIRST: Self = Self(0);

    /// Generation that supersedes this one.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    /// Underlying counter value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

/// One authorised provider operation and the bound the runtime enforces on it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderCall {
    _placeholder: (),
}

impl ProviderCall {
    /// Describe one authorised provider operation.
    #[must_use]
    pub fn new(
        _provider: &'static str,
        _operation: &'static str,
        _deadline: Duration,
        _generation: Generation,
    ) -> Self {
        todo!()
    }

    /// Provider this call is attributed to.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        todo!()
    }

    /// Operation this call performs.
    #[must_use]
    pub fn operation(&self) -> &'static str {
        todo!()
    }

    /// Deadline the runtime enforces.
    #[must_use]
    pub fn deadline(&self) -> Duration {
        todo!()
    }

    /// Generation the completion will carry.
    #[must_use]
    pub fn generation(&self) -> Generation {
        todo!()
    }
}

/// Scoped, attributable provider refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderFailure {
    /// The provider did not answer within its deadline.
    #[error("provider exceeded its {deadline:?} deadline")]
    TimedOut {
        /// Deadline that was exceeded.
        deadline: Duration,
    },
    /// The provider panicked.
    #[error("provider panicked: {detail}")]
    Panicked {
        /// Bounded panic detail.
        detail: String,
    },
    /// The owning token cancelled the call.
    #[error("provider call cancelled")]
    Cancelled,
}

/// Result of one provider operation, correlated to the operation that authorised it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Completion<T> {
    _placeholder: PhantomData<T>,
}

impl<T> Completion<T> {
    /// Build a failed completion for one authorised call.
    #[must_use]
    pub fn failed(_call: &ProviderCall, _failure: ProviderFailure) -> Self {
        todo!()
    }

    /// Provider this completion is attributed to.
    #[must_use]
    pub fn provider(&self) -> &'static str {
        todo!()
    }

    /// Operation this completion answers.
    #[must_use]
    pub fn operation(&self) -> &'static str {
        todo!()
    }

    /// Generation this completion answers.
    #[must_use]
    pub fn generation(&self) -> Generation {
        todo!()
    }

    /// Whether this completion answers the owner's current generation.
    #[must_use]
    pub fn is_current(&self, _expected: Generation) -> bool {
        todo!()
    }

    /// Borrow the outcome.
    pub fn outcome(&self) -> Result<&T, &ProviderFailure> {
        todo!()
    }

    /// Take the outcome regardless of generation.
    pub fn into_outcome(self) -> Result<T, ProviderFailure> {
        todo!()
    }

    /// Take the outcome only when it answers the owner's current generation.
    pub fn accept_if_current(self, _expected: Generation) -> Option<Result<T, ProviderFailure>> {
        todo!()
    }
}
