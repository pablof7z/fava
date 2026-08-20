//! Thin Rust facade over the selected Fava provider assembly.

mod live;

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use fava_diagnostics::Diagnostics;
pub use fava_diagnostics::DiagnosticsSnapshot;
use fava_event_cache::EventCache;
use fava_observe::Observer;
pub use fava_observe::{Observation, ObservationClosed, ObserveError};
pub use fava_query::{
    EventRecord, Freshness, Query, QueryRevision, QuerySnapshot, ResultAuthority,
};
use fava_query::{QueryEvaluator, QuerySource};
use fava_subscriptions::SubscriptionPlanner;
use fava_transport::Transport;
pub use fava_write::{EventValue, ReceiptId};
use fava_write_store::WriteStore;
pub use fava_write_store::{AcceptedWrite, WriteStoreError};
use thiserror::Error;

/// Built engine instance for the selected local-source assembly.
pub struct Fava {
    observer: Observer,
    event_cache: Arc<dyn EventCache>,
    write_store: Arc<dyn WriteStore>,
    subscription_planner: Option<Arc<dyn SubscriptionPlanner>>,
    transport: Option<Arc<dyn Transport>>,
    diagnostics: Arc<Diagnostics>,
    next_subscription: AtomicU64,
}

impl Fava {
    /// Begin explicit provider assembly.
    #[must_use]
    pub fn builder() -> FavaBuilder {
        FavaBuilder::default()
    }

    /// Open a live query. The returned handle already contains local state.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when the declarative query is invalid or the
    /// configured local sources cannot establish one coherent initial view.
    pub async fn observe(&self, query: Query) -> Result<Observation, ObserveError> {
        if query.freshness() == Freshness::CacheOnly {
            self.observer.open(query)
        } else {
            live::open(self, query).await
        }
    }

    /// Accept one finalized local event into the durable-write authority.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the event is invalid or acceptance
    /// cannot commit atomically.
    pub fn accept_event(&self, event: EventValue) -> Result<AcceptedWrite, WriteStoreError> {
        self.write_store.accept_materialized(event)
    }

    /// Cancel one accepted event before publication work exists.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when cancellation cannot commit atomically.
    pub fn cancel_write(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError> {
        self.write_store.cancel(receipt_id)
    }

    /// Return one bounded immutable snapshot of current exact diagnostic facts.
    #[must_use]
    pub fn diagnostics(&self) -> DiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }
}

/// Static assembly builder. No provider is silently selected.
#[derive(Default)]
pub struct FavaBuilder {
    event_cache: Option<Arc<dyn EventCache>>,
    write_store: Option<Arc<dyn WriteStore>>,
    evaluator: Option<Arc<dyn QueryEvaluator>>,
    subscription_planner: Option<Arc<dyn SubscriptionPlanner>>,
    transport: Option<Arc<dyn Transport>>,
}

impl FavaBuilder {
    /// Select one event-cache provider.
    #[must_use]
    pub fn event_cache<T>(mut self, cache: Arc<T>) -> Self
    where
        T: EventCache + 'static,
    {
        self.event_cache = Some(cache);
        self
    }

    /// Select one write-store provider.
    #[must_use]
    pub fn write_store<T>(mut self, store: Arc<T>) -> Self
    where
        T: WriteStore + 'static,
    {
        self.write_store = Some(store);
        self
    }

    /// Select one local query evaluator.
    #[must_use]
    pub fn query_evaluator<T>(mut self, evaluator: Arc<T>) -> Self
    where
        T: QueryEvaluator + 'static,
    {
        self.evaluator = Some(evaluator);
        self
    }

    /// Select one exact subscription planner.
    #[must_use]
    pub fn subscription_planner<T>(mut self, planner: Arc<T>) -> Self
    where
        T: SubscriptionPlanner + 'static,
    {
        self.subscription_planner = Some(planner);
        self
    }

    /// Select one relay transport provider.
    #[must_use]
    pub fn transport<T>(mut self, transport: Arc<T>) -> Self
    where
        T: Transport + 'static,
    {
        self.transport = Some(transport);
        self
    }

    /// Validate the complete Slice 1 assembly.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] naming the first required provider role that was
    /// not selected.
    pub fn build(self) -> Result<Fava, BuildError> {
        let event_cache = self.event_cache.ok_or(BuildError::MissingEventCache)?;
        let write_store = self.write_store.ok_or(BuildError::MissingWriteStore)?;
        let evaluator = self.evaluator.ok_or(BuildError::MissingQueryEvaluator)?;
        let event_source: Arc<dyn QuerySource> = event_cache.clone();
        let write_source: Arc<dyn QuerySource> = write_store.clone();
        Ok(Fava {
            observer: Observer::new(event_source, write_source, evaluator),
            event_cache,
            write_store,
            subscription_planner: self.subscription_planner,
            transport: self.transport,
            diagnostics: Arc::new(Diagnostics::default()),
            next_subscription: AtomicU64::new(0),
        })
    }
}

/// Static assembly refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuildError {
    /// No event-cache authority was selected.
    #[error("Fava assembly requires one event-cache provider")]
    MissingEventCache,
    /// No write-store authority was selected.
    #[error("Fava assembly requires one write-store provider")]
    MissingWriteStore,
    /// No local query evaluator was selected.
    #[error("Fava assembly requires one query evaluator")]
    MissingQueryEvaluator,
}
