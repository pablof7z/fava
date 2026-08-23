//! Task execution, the join registry, and bounded shutdown.

use std::future::Future;
use std::time::Duration;

use thiserror::Error;

use crate::cancel::Cancellation;
use crate::provider::{Completion, ProviderCall};

/// Attributable identity of one runtime-owned task.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskId {
    _placeholder: (),
}

impl TaskId {
    /// Static label the owner gave this task.
    #[must_use]
    pub fn label(&self) -> &'static str {
        todo!()
    }

    /// Monotonic ordinal within one runtime.
    #[must_use]
    pub fn ordinal(&self) -> u64 {
        todo!()
    }
}

/// Typed refusal of new execution.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SpawnRefusal {
    /// The runtime is shutting down or closed.
    #[error("runtime is closed")]
    Closed,
    /// The runtime already owns its declared maximum number of live tasks.
    #[error("runtime already owns its capacity of {capacity} tasks")]
    AtCapacity {
        /// Declared live-task capacity.
        capacity: usize,
    },
}

/// Bounded evidence of one shutdown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShutdownReport {
    _placeholder: (),
}

impl ShutdownReport {
    /// Tasks that stopped within the deadline.
    #[must_use]
    pub fn joined(&self) -> usize {
        todo!()
    }

    /// Tasks aborted because they outlived the deadline.
    #[must_use]
    pub fn unjoined(&self) -> usize {
        todo!()
    }

    /// Tasks that panicked during this runtime's life.
    #[must_use]
    pub fn panicked(&self) -> usize {
        todo!()
    }

    /// Bounded identities of the tasks that outlived the deadline.
    #[must_use]
    pub fn unjoined_tasks(&self) -> &[TaskId] {
        todo!()
    }

    /// Whether every owned task stopped within the deadline.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        todo!()
    }
}

/// Execution owner: every task it starts is owned, joinable, and cancellable.
#[derive(Clone, Debug)]
pub struct Runtime {
    _placeholder: (),
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    /// Create a runtime with the default live-task capacity.
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    /// Create a runtime bounded to `capacity` live tasks.
    #[must_use]
    pub fn with_task_capacity(_capacity: usize) -> Self {
        todo!()
    }

    /// Root cancellation token every owned resource derives from.
    #[must_use]
    pub fn cancellation(&self) -> Cancellation {
        todo!()
    }

    /// Live owned tasks.
    #[must_use]
    pub fn live_tasks(&self) -> usize {
        todo!()
    }

    /// Bounded identities of the tasks that panicked.
    #[must_use]
    pub fn panicked_tasks(&self) -> Vec<TaskId> {
        todo!()
    }

    /// Start one owned, joinable, panic-isolated task.
    pub fn spawn<F>(&self, _label: &'static str, _future: F) -> Result<TaskId, SpawnRefusal>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        todo!()
    }

    /// Perform one authorised provider call under its deadline and this runtime's
    /// cancellation, returning a completion correlated to the authorising operation.
    pub async fn invoke<F, T>(&self, _call: ProviderCall, _future: F) -> Completion<T>
    where
        F: Future<Output = T>,
    {
        todo!()
    }

    /// Refuse new work, cancel the root token, and join owned tasks within `deadline`.
    pub async fn shutdown(&self, _deadline: Duration) -> ShutdownReport {
        todo!()
    }
}
