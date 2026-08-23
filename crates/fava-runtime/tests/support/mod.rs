//! Shared construction for runtime evidence.

use std::num::NonZeroUsize;

use fava_runtime::{Runtime, RuntimeConfig};

/// A runtime with generous bounds, for evidence that is not about a bound.
#[must_use]
pub fn runtime() -> Runtime {
    Runtime::new(config(16, 64, 64))
}

/// A runtime configuration with exact declared bounds.
#[must_use]
pub fn config(
    default_channel_depth: usize,
    max_tasks: usize,
    max_provider_operations: usize,
) -> RuntimeConfig {
    RuntimeConfig {
        default_channel_depth: nonzero(default_channel_depth),
        max_tasks: nonzero(max_tasks),
        max_provider_operations: nonzero(max_provider_operations),
    }
}

/// Build a `NonZeroUsize` from a literal that is known to be non-zero.
#[must_use]
pub fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("bound must be non-zero")
}
