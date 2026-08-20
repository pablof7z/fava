//! Thin Rust facade over the selected NMP provider assembly.

use std::sync::Arc;

use nmp_event_cache::EventCache;
use nmp_observe::{Observation, ObserveError, Observer};
pub use nmp_query::{
    EventQuery, EventRecord, Freshness, QueryRevision, QuerySnapshot, ResultAuthority,
};
use nmp_query::{QueryEvaluator, QuerySource};
use nmp_write_store::WriteStore;
use thiserror::Error;

/// Built engine instance for the selected local-source assembly.
pub struct Nmp {
    observer: Observer,
}

impl Nmp {
    /// Begin explicit provider assembly.
    #[must_use]
    pub fn builder() -> NmpBuilder {
        NmpBuilder::default()
    }

    /// Open a live query. The returned handle already contains local state.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when the declarative query is invalid or the
    /// configured local sources cannot establish one coherent initial view.
    #[allow(clippy::unused_async)] // Preserve the specified async facade as later providers become asynchronous.
    pub async fn observe(&self, query: EventQuery) -> Result<Observation, ObserveError> {
        self.observer.open(query)
    }
}

/// Static assembly builder. No provider is silently selected.
#[derive(Default)]
pub struct NmpBuilder {
    event_cache: Option<Arc<dyn QuerySource>>,
    write_store: Option<Arc<dyn QuerySource>>,
    evaluator: Option<Arc<dyn QueryEvaluator>>,
}

impl NmpBuilder {
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

    /// Validate the complete Slice 1 assembly.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] naming the first required provider role that was
    /// not selected.
    pub fn build(self) -> Result<Nmp, BuildError> {
        let event_cache = self.event_cache.ok_or(BuildError::MissingEventCache)?;
        let write_store = self.write_store.ok_or(BuildError::MissingWriteStore)?;
        let evaluator = self.evaluator.ok_or(BuildError::MissingQueryEvaluator)?;
        Ok(Nmp {
            observer: Observer::new(event_cache, write_store, evaluator),
        })
    }
}

/// Static assembly refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BuildError {
    /// No event-cache authority was selected.
    #[error("NMP assembly requires one event-cache provider")]
    MissingEventCache,
    /// No write-store authority was selected.
    #[error("NMP assembly requires one write-store provider")]
    MissingWriteStore,
    /// No local query evaluator was selected.
    #[error("NMP assembly requires one query evaluator")]
    MissingQueryEvaluator,
}
