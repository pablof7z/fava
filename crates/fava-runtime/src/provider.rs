//! Typed completions of deadline-wrapped provider calls.
//!
//! Authority: ARCH:2366 "The runtime performs the work and returns typed
//! completions."

use std::any::Any;
use std::time::Duration;

use crate::generation::Round;
use crate::name::OperationName;

/// Longest retained panic payload, in bytes.
pub(crate) const PANIC_DETAIL_CAPACITY: usize = 512;

/// Typed completion of one deadline-wrapped provider call.
///
/// Every variant carries the operation slot and the generation the owner
/// authorised, so an owner that has moved on can reject the completion instead
/// of installing it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCompletion<T> {
    /// The provider returned within its deadline.
    Completed {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: Round,
        /// What the provider returned.
        value: T,
    },
    /// The deadline expired. The provider may still be running; the runtime
    /// owns detaching it and it can no longer affect the owner.
    TimedOut {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: Round,
        /// The deadline that expired.
        after: Duration,
    },
    /// The provider panicked and was isolated.
    Panicked {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: Round,
        /// Bounded panic payload.
        detail: String,
    },
    /// The owner's cancellation token fired.
    Cancelled {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: Round,
    },
    /// The runtime is shutting down and refused the call.
    Refused {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: Round,
    },
}

impl<T> ProviderCompletion<T> {
    /// Generation this completion belongs to. An owner MUST compare this
    /// against its current generation and discard stale completions.
    #[must_use]
    pub fn generation(&self) -> Round {
        match self {
            Self::Completed { generation, .. }
            | Self::TimedOut { generation, .. }
            | Self::Panicked { generation, .. }
            | Self::Cancelled { generation, .. }
            | Self::Refused { generation, .. } => *generation,
        }
    }

    /// Operation slot this completion belongs to.
    #[must_use]
    pub fn operation(&self) -> OperationName {
        match self {
            Self::Completed { operation, .. }
            | Self::TimedOut { operation, .. }
            | Self::Panicked { operation, .. }
            | Self::Cancelled { operation, .. }
            | Self::Refused { operation, .. } => *operation,
        }
    }

    /// What the provider returned, if it ran to completion.
    #[must_use]
    pub fn value(self) -> Option<T> {
        match self {
            Self::Completed { value, .. } => Some(value),
            _ => None,
        }
    }
}

/// Render an unwind payload as bounded, attributable text.
pub(crate) fn panic_detail(payload: &(dyn Any + Send)) -> String {
    let detail = if let Some(text) = payload.downcast_ref::<&'static str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic payload".to_owned()
    };
    truncate(detail)
}

/// Keep retained panic evidence bounded on a character boundary.
fn truncate(mut detail: String) -> String {
    if detail.len() <= PANIC_DETAIL_CAPACITY {
        return detail;
    }
    let mut end = PANIC_DETAIL_CAPACITY;
    while end > 0 && !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail.truncate(end);
    detail
}
