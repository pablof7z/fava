//! Fava execution owner.
//!
//! `fava-runtime` owns every task Fava starts and every task Fava must join,
//! the join registry that lets close prove no Fava-started task outlives it,
//! bounded command channels whose full state is a typed refusal rather than a
//! park, the deadline wrapped around every application-supplied provider call,
//! the panic isolation that turns a provider unwind into a typed completion,
//! and cancellation tokens and their propagation.
//!
//! It owns no meaning: it never inspects an event kind, chooses a route,
//! evaluates a query, or writes durable state.
//!
//! `fava-runtime` is a universal owner, not a replaceable provider. It exposes
//! concrete types, not a trait to implement.
//!
//! Authority: `docs/spec/ARCHITECTURE.md` §`fava-runtime` (owned resources,
//! owner relationship, provider isolation) and
//! `.planning/audit/2026-08-23/FROZEN-CONTRACTS.md` §5.

mod cancel;
mod channel;
mod generation;
mod name;
mod provider;
mod runtime;
mod task;

pub use cancel::CancellationToken;
pub use channel::{Receiver, SendRefusal, SendRefused, Sender};
pub use generation::OperationGeneration;
pub use name::{OperationName, TaskName};
pub use provider::ProviderCompletion;
pub use runtime::{Runtime, RuntimeConfig, RuntimeError};
pub use task::{TaskFailure, TaskHandle};
