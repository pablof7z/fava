//! Owner of every task, timer, channel, deadline, and join in one Fava engine.
//!
//! Authority: ARCH:2350-2364.

use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use futures_util::FutureExt;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::cancel::CancellationToken;
use crate::channel::{Receiver, Sender};
use crate::generation::Round;
use crate::name::{OperationName, TaskName};
use crate::provider::{ProviderCompletion, panic_detail};
use crate::task::{TaskFailure, TaskHandle};

/// Owner of every task, timer, channel, deadline, and join in one Fava engine.
///
/// Authority: ARCH:2350-2364.
#[derive(Clone)]
pub struct Runtime {
    inner: Arc<Inner>,
}

/// Configuration supplied once at engine construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Default depth for bounded channels created without an explicit depth.
    pub default_channel_depth: NonZeroUsize,
    /// Maximum concurrently tracked spawned tasks. Exceeding it refuses.
    pub max_tasks: NonZeroUsize,
    /// Maximum concurrently running provider operations.
    pub max_provider_operations: NonZeroUsize,
}

/// One registered task and the grip shutdown joins it by.
struct Registered {
    name: TaskName,
    handle: JoinHandle<()>,
}

struct Inner {
    config: RuntimeConfig,
    root: CancellationToken,
    shutting_down: AtomicBool,
    next_ordinal: AtomicU64,
    registry: Mutex<BTreeMap<u64, Registered>>,
    provider_operations: AtomicUsize,
}

impl Inner {
    fn registry(&self) -> MutexGuard<'_, BTreeMap<u64, Registered>> {
        self.registry.lock().unwrap_or_else(|poison| {
            self.registry.clear_poison();
            poison.into_inner()
        })
    }

    fn deregister(&self, ordinal: u64) {
        self.registry().remove(&ordinal);
    }

    fn release_provider_slot(&self) {
        self.provider_operations.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Runtime {
    /// Construct a runtime on the ambient async executor.
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                root: CancellationToken::root(),
                shutting_down: AtomicBool::new(false),
                next_ordinal: AtomicU64::new(0),
                registry: Mutex::new(BTreeMap::new()),
                provider_operations: AtomicUsize::new(0),
            }),
        }
    }

    /// Configuration this runtime was constructed with.
    #[must_use]
    pub fn config(&self) -> RuntimeConfig {
        self.inner.config
    }

    // ------------------------------------------------------------ spawning

    /// Spawn owned work and register it for shutdown join.
    ///
    /// The returned [`TaskHandle`] is the owner's grip; the join registry keeps
    /// its own so shutdown can join work whose handle was dropped.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TaskLimit`] at `max_tasks`;
    /// [`RuntimeError::ShuttingDown`] after shutdown began.
    pub fn spawn<F>(&self, name: TaskName, future: F) -> Result<TaskHandle<F::Output>, RuntimeError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let finished = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = oneshot::channel();
        let owner = Arc::downgrade(&self.inner);
        let flag = Arc::clone(&finished);
        self.register(name, move |ordinal| async move {
            let outcome = match AssertUnwindSafe(future).catch_unwind().await {
                Ok(value) => Ok(value),
                Err(payload) => Err(TaskFailure::Panicked {
                    name,
                    detail: panic_detail(payload.as_ref()),
                }),
            };
            flag.store(true, Ordering::Release);
            drop(sender.send(outcome));
            finish(&owner, ordinal, false);
        })?;
        Ok(TaskHandle::new(name, finished, receiver))
    }

    /// Spawn work bound to a cancellation token; cancelling the token drives
    /// the task to completion promptly.
    ///
    /// The task's value is `None` exactly when the token fired first.
    ///
    /// # Errors
    ///
    /// As [`Runtime::spawn`].
    pub fn spawn_cancellable<F>(
        &self,
        name: TaskName,
        token: CancellationToken,
        future: F,
    ) -> Result<TaskHandle<Option<F::Output>>, RuntimeError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawn(name, async move {
            tokio::select! {
                biased;
                () = token.cancelled() => None,
                value = future => Some(value),
            }
        })
    }

    /// Names of tasks currently registered. The shutdown-join falsifier reads
    /// this.
    #[must_use]
    pub fn outstanding_tasks(&self) -> Vec<TaskName> {
        self.inner
            .registry()
            .values()
            .map(|registered| registered.name)
            .collect()
    }

    // ------------------------------------------------------------ channels

    /// Create a bounded command channel.
    ///
    /// A channel created after shutdown began is created already closed, so a
    /// shutting-down runtime admits no new command traffic.
    #[must_use]
    pub fn channel<T: Send + 'static>(&self, depth: NonZeroUsize) -> (Sender<T>, Receiver<T>) {
        crate::channel::build(depth, self.inner.shutting_down.load(Ordering::Acquire))
    }

    /// Create a bounded command channel at the configured default depth.
    #[must_use]
    pub fn default_channel<T: Send + 'static>(&self) -> (Sender<T>, Receiver<T>) {
        self.channel(self.inner.config.default_channel_depth)
    }

    // ------------------------------------------------------------ providers

    /// Invoke one application-supplied provider call under a Fava-owned
    /// deadline, outside every owner lock, with panic isolation.
    ///
    /// This is the only sanctioned way to await a provider. A bare `.await` on
    /// a `dyn` provider anywhere in a lifecycle owner is a contract violation.
    ///
    /// On [`ProviderCompletion::TimedOut`] and [`ProviderCompletion::Cancelled`]
    /// the provider future is detached, not aborted: a provider mid-write to
    /// its own store must not be torn apart. The detached future stays in the
    /// join registry, so shutdown either joins it or names it in
    /// [`RuntimeError::ShutdownIncomplete`].
    ///
    /// Authority: ARCH:2372.
    pub async fn call_provider<T, F>(
        &self,
        operation: OperationName,
        generation: Round,
        deadline: Duration,
        call: F,
    ) -> ProviderCompletion<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        if !self.reserve_provider_slot() {
            return ProviderCompletion::Refused {
                operation,
                generation,
            };
        }

        let (sender, receiver) = oneshot::channel();
        let owner = Arc::downgrade(&self.inner);
        let spawned = self.register(TaskName(operation.0), move |ordinal| async move {
            let outcome = AssertUnwindSafe(call).catch_unwind().await;
            drop(sender.send(outcome));
            finish(&owner, ordinal, true);
        });
        if spawned.is_err() {
            self.inner.release_provider_slot();
            return ProviderCompletion::Refused {
                operation,
                generation,
            };
        }

        let token = self.cancellation_token();
        tokio::select! {
            biased;
            () = token.cancelled() => ProviderCompletion::Cancelled { operation, generation },
            answered = tokio::time::timeout(deadline, receiver) => match answered {
                Ok(Ok(Ok(value))) => ProviderCompletion::Completed { operation, generation, value },
                Ok(Ok(Err(payload))) => ProviderCompletion::Panicked {
                    operation,
                    generation,
                    detail: panic_detail(payload.as_ref()),
                },
                Ok(Err(_dropped)) => ProviderCompletion::Cancelled { operation, generation },
                Err(_elapsed) => ProviderCompletion::TimedOut {
                    operation,
                    generation,
                    after: deadline,
                },
            },
        }
    }

    /// Provider operations currently running.
    #[must_use]
    pub fn running_provider_operations(&self) -> usize {
        self.inner.provider_operations.load(Ordering::Acquire)
    }

    // ------------------------------------------------------------ time

    /// A cancellation token rooted in this runtime's shutdown token.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.inner.root.child()
    }

    /// Sleep on the runtime's clock. Test builds may drive it deterministically.
    pub async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    // ------------------------------------------------------------ shutdown

    /// Refuse new spawns and channels, cancel the root token, and join every
    /// registered task within `deadline`.
    ///
    /// Repeated shutdown is harmless: the second call owns no tasks and returns
    /// `Ok(())`.
    ///
    /// Authority: ARCH:2364 "resource joining and shutdown deadlines";
    /// GOALS:1497-1507 (OPS-009).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::ShutdownIncomplete`] naming the tasks that did not join.
    pub async fn shutdown(&self, deadline: Duration) -> Result<(), RuntimeError> {
        self.inner.shutting_down.store(true, Ordering::Release);
        self.inner.root.cancel();

        let registered: Vec<Registered> = {
            let mut registry = self.inner.registry();
            std::mem::take(&mut *registry).into_values().collect()
        };

        let end = tokio::time::Instant::now() + deadline;
        let mut unjoined = Vec::new();
        for Registered { name, mut handle } in registered {
            if tokio::time::timeout_at(end, &mut handle).await.is_err() {
                handle.abort();
                unjoined.push(name);
            }
        }

        if unjoined.is_empty() {
            Ok(())
        } else {
            Err(RuntimeError::ShutdownIncomplete { tasks: unjoined })
        }
    }

    // ------------------------------------------------------------ internals

    /// Register one owned task under the shutdown join registry.
    ///
    /// The registry lock is held across the spawn so a task cannot deregister
    /// itself before it is registered. Nothing is awaited while it is held.
    fn register<M, F>(&self, name: TaskName, make: M) -> Result<(), RuntimeError>
    where
        M: FnOnce(u64) -> F,
        F: Future<Output = ()> + Send + 'static,
    {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err(RuntimeError::ShuttingDown);
        }
        let limit = self.inner.config.max_tasks.get();
        let mut registry = self.inner.registry();
        if registry.len() >= limit {
            return Err(RuntimeError::TaskLimit { limit });
        }
        let ordinal = self.inner.next_ordinal.fetch_add(1, Ordering::AcqRel);
        let handle = tokio::spawn(make(ordinal));
        registry.insert(ordinal, Registered { name, handle });
        Ok(())
    }

    /// Claim one provider-operation slot, refusing at the declared bound or
    /// once shutdown has begun.
    fn reserve_provider_slot(&self) -> bool {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return false;
        }
        let limit = self.inner.config.max_provider_operations.get();
        let prior = self
            .inner
            .provider_operations
            .fetch_add(1, Ordering::AcqRel);
        if prior >= limit {
            self.inner.release_provider_slot();
            return false;
        }
        true
    }
}

/// Deregister one finished task and release its provider slot when it held one.
fn finish(owner: &Weak<Inner>, ordinal: u64, provider_slot: bool) {
    if let Some(inner) = owner.upgrade() {
        if provider_slot {
            inner.release_provider_slot();
        }
        inner.deregister(ordinal);
    }
}

/// Runtime refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeError {
    /// The task registry is at its declared bound.
    #[error("runtime holds {limit} tasks, its declared maximum")]
    TaskLimit {
        /// Declared maximum.
        limit: usize,
    },
    /// The provider-operation bound is reached.
    ///
    /// `call_provider` reports this condition as
    /// [`ProviderCompletion::Refused`], because a provider call answers with a
    /// completion rather than an error; this variant carries the same fact for
    /// owners that admit provider work through a fallible operation.
    #[error("runtime holds {limit} provider operations, its declared maximum")]
    ProviderOperationLimit {
        /// Declared maximum.
        limit: usize,
    },
    /// Shutdown began; no new work is admitted.
    #[error("runtime is shutting down")]
    ShuttingDown,
    /// Shutdown's deadline expired with work still registered.
    #[error("{} tasks did not join before the shutdown deadline", .tasks.len())]
    ShutdownIncomplete {
        /// Tasks that did not join.
        tasks: Vec<TaskName>,
    },
}
