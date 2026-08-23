//! An owner's grip on one spawned task.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use thiserror::Error;
use tokio::sync::oneshot;

use crate::name::TaskName;

/// An owner's grip on one spawned task.
///
/// Dropping the handle does not detach the task: the join registry keeps its
/// own grip so shutdown can still join work whose handle was dropped.
pub struct TaskHandle<T> {
    name: TaskName,
    finished: Arc<AtomicBool>,
    outcome: oneshot::Receiver<Result<T, TaskFailure>>,
}

impl<T> fmt::Debug for TaskHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskHandle")
            .field("name", &self.name)
            .field("finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

impl<T> TaskHandle<T> {
    pub(crate) fn new(
        name: TaskName,
        finished: Arc<AtomicBool>,
        outcome: oneshot::Receiver<Result<T, TaskFailure>>,
    ) -> Self {
        Self {
            name,
            finished,
            outcome,
        }
    }

    /// Name of the task.
    #[must_use]
    pub fn name(&self) -> TaskName {
        self.name
    }

    /// Await completion.
    ///
    /// # Errors
    ///
    /// [`TaskFailure`] when the task panicked or was aborted at shutdown.
    pub async fn join(self) -> Result<T, TaskFailure> {
        match self.outcome.await {
            Ok(result) => result,
            Err(_dropped) => Err(TaskFailure::Aborted { name: self.name }),
        }
    }

    /// Whether the task has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// Why a task did not produce its value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskFailure {
    /// The task panicked; the payload is bounded.
    #[error("task {name} panicked: {detail}")]
    Panicked {
        /// Task name.
        name: TaskName,
        /// Bounded panic payload.
        detail: String,
    },
    /// The task was aborted because shutdown's deadline expired.
    #[error("task {name} was aborted at shutdown")]
    Aborted {
        /// Task name.
        name: TaskName,
    },
}
