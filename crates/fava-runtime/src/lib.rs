//! Fava execution owner.
//!
//! `fava-runtime` performs the asynchronous work that universal owners
//! authorise and returns typed completions. It owns task execution, timers,
//! bounded command/completion channels, cancellation propagation, provider
//! panic and stall isolation, the bounded reconnect policy, and the joining of
//! every owned resource within a declared shutdown deadline.
//!
//! It interprets no event-kind meaning, chooses no route, calculates no query
//! result, and updates no durable state.

mod backoff;
mod cancel;
mod channel;
mod provider;
mod task;

pub use backoff::{Backoff, BackoffShortfall};
pub use cancel::Cancellation;
pub use channel::{Backpressure, BoundedReceiver, BoundedSender, bounded_channel};
pub use provider::{Completion, Generation, ProviderCall, ProviderFailure};
pub use task::{Runtime, ShutdownReport, SpawnRefusal, TaskId};
