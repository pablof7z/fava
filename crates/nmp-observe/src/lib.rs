//! Live-query ownership over independent local sources and bounded latest-state delivery.

use std::sync::Arc;

use nmp_query::{
    CanonicalQuery, EventQuery, OpenedQuerySource, QueryError, QueryEvaluationError,
    QueryEvaluator, QueryRevision, QuerySnapshot, QuerySource, QuerySourceError, SourceKind,
    SourceSnapshot, SourceStatus,
};
use thiserror::Error;
use tokio::sync::watch;

/// Configured local observation owner.
pub struct Observer {
    event_cache: Arc<dyn QuerySource>,
    write_store: Arc<dyn QuerySource>,
    evaluator: Arc<dyn QueryEvaluator>,
}

impl Observer {
    /// Construct the owner from neutral provider contracts.
    #[must_use]
    pub fn new(
        event_cache: Arc<dyn QuerySource>,
        write_store: Arc<dyn QuerySource>,
        evaluator: Arc<dyn QueryEvaluator>,
    ) -> Self {
        Self {
            event_cache,
            write_store,
            evaluator,
        }
    }

    /// Atomically open both local sources and return an immediately readable view.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when query validation, either source open, or
    /// initial evaluation fails. Any provisionally opened source is closed.
    pub fn open(&self, query: EventQuery) -> Result<Observation, ObserveError> {
        let query = query.canonicalize()?;
        let cache = self
            .event_cache
            .open(&query)
            .map_err(|error| ObserveError::SourceOpen {
                role: SourceKind::EventCache,
                error,
            })?;
        let writes = match self.write_store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut cache_changes = cache.changes;
                cache_changes.close();
                return Err(ObserveError::SourceOpen {
                    role: SourceKind::WriteStore,
                    error,
                });
            }
        };

        Observation::start(query, cache, writes, Arc::clone(&self.evaluator))
    }
}

/// One opened latest-state live query.
pub struct Observation {
    latest: watch::Receiver<Arc<QuerySnapshot>>,
    cancel: watch::Sender<bool>,
}

impl Observation {
    fn start(
        query: CanonicalQuery,
        cache: OpenedQuerySource,
        writes: OpenedQuerySource,
        evaluator: Arc<dyn QueryEvaluator>,
    ) -> Result<Self, ObserveError> {
        let mut sources = vec![cache.initial, writes.initial];
        let mut cache_changes = cache.changes;
        let mut write_changes = writes.changes;
        let mut initial = match evaluator.evaluate(&query, &sources) {
            Ok(initial) => initial,
            Err(error) => {
                cache_changes.close();
                write_changes.close();
                return Err(error.into());
            }
        };
        initial.revision = QueryRevision(1);
        let (latest_tx, latest) = watch::channel(Arc::new(initial));
        let (cancel, mut cancel_rx) = watch::channel(false);

        tokio::spawn(async move {
            let mut revision = 1_u64;
            let mut cache_open = true;
            let mut writes_open = true;
            loop {
                let changed = tokio::select! {
                    biased;
                    cancel_result = cancel_rx.changed() => {
                        if cancel_result.is_err() || *cancel_rx.borrow_and_update() {
                            break;
                        }
                        None
                    }
                    cache_result = cache_changes.next_change(), if cache_open => {
                        Some((SourceKind::EventCache, cache_result))
                    }
                    write_result = write_changes.next_change(), if writes_open => {
                        Some((SourceKind::WriteStore, write_result))
                    }
                };

                let Some((role, changed)) = changed else {
                    continue;
                };
                if let Ok(snapshot) = changed {
                    replace_source(&mut sources, snapshot);
                } else {
                    if role == SourceKind::EventCache {
                        cache_open = false;
                        cache_changes.close();
                    } else {
                        writes_open = false;
                        write_changes.close();
                    }
                    mark_source_closed(&mut sources, role);
                }
                let Ok(mut snapshot) = evaluator.evaluate(&query, &sources) else {
                    break;
                };
                let Some(next_revision) = revision.checked_add(1) else {
                    break;
                };
                revision = next_revision;
                snapshot.revision = QueryRevision(revision);
                latest_tx.send_replace(Arc::new(snapshot));
            }
            cache_changes.close();
            write_changes.close();
        });

        Ok(Self { latest, cancel })
    }

    /// Exact current snapshot, readable immediately after open.
    #[must_use]
    pub fn current(&self) -> Arc<QuerySnapshot> {
        Arc::clone(&self.latest.borrow())
    }

    /// Await a newer delivered current state.
    ///
    /// # Errors
    ///
    /// Returns [`ObservationClosed`] after explicit close, provider termination,
    /// evaluation failure, or engine teardown.
    pub async fn changed(&mut self) -> Result<Arc<QuerySnapshot>, ObservationClosed> {
        self.latest.changed().await.map_err(|_| ObservationClosed)?;
        Ok(Arc::clone(&self.latest.borrow_and_update()))
    }

    /// Close this observation. Repeated close is harmless.
    pub fn close(&self) {
        self.cancel.send_replace(true);
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

fn replace_source(sources: &mut [SourceSnapshot], changed: SourceSnapshot) {
    if let Some(source) = sources
        .iter_mut()
        .find(|source| source.kind == changed.kind)
    {
        *source = changed;
    }
}

fn mark_source_closed(sources: &mut [SourceSnapshot], role: SourceKind) {
    if let Some(source) = sources.iter_mut().find(|source| source.kind == role) {
        source.status = SourceStatus::Closed;
    }
}

/// Query-open refusal before a usable handle exists.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// Query structure or source policy was invalid.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// One named local source could not establish its initial boundary.
    #[error("{role:?} failed to open: {error}")]
    SourceOpen {
        /// Semantic source role.
        role: SourceKind,
        /// Scoped provider refusal.
        error: QuerySourceError,
    },
    /// Initial local evaluation failed.
    #[error(transparent)]
    Evaluation(#[from] QueryEvaluationError),
}

/// Terminal observation fact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("live query observation closed")]
pub struct ObservationClosed;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use nmp_query::{QuerySourceClosed, SourceChangeFuture, SourceChanges, SourceRevision};

    use super::*;

    struct TrackingSource {
        role: SourceKind,
        closes: Arc<AtomicUsize>,
    }

    impl QuerySource for TrackingSource {
        fn open(&self, _query: &CanonicalQuery) -> Result<OpenedQuerySource, QuerySourceError> {
            Ok(OpenedQuerySource {
                initial: SourceSnapshot {
                    kind: self.role,
                    revision: SourceRevision(0),
                    status: SourceStatus::Open,
                    events: Vec::new(),
                },
                changes: Box::new(TrackingChanges {
                    closes: Arc::clone(&self.closes),
                }),
            })
        }
    }

    struct TrackingChanges {
        closes: Arc<AtomicUsize>,
    }

    impl SourceChanges for TrackingChanges {
        fn next_change(&mut self) -> SourceChangeFuture<'_> {
            Box::pin(async { Err(QuerySourceClosed) })
        }

        fn close(&mut self) {
            self.closes.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct RefusingSource;

    impl QuerySource for RefusingSource {
        fn open(&self, _query: &CanonicalQuery) -> Result<OpenedQuerySource, QuerySourceError> {
            Err(QuerySourceError::Refused(
                "injected open failure".to_owned(),
            ))
        }
    }

    struct EmptyEvaluator;

    impl QueryEvaluator for EmptyEvaluator {
        fn evaluate(
            &self,
            _query: &CanonicalQuery,
            sources: &[SourceSnapshot],
        ) -> Result<QuerySnapshot, QueryEvaluationError> {
            Ok(QuerySnapshot::evaluated(Vec::new(), sources))
        }
    }

    struct FailingEvaluator;

    impl QueryEvaluator for FailingEvaluator {
        fn evaluate(
            &self,
            _query: &CanonicalQuery,
            _sources: &[SourceSnapshot],
        ) -> Result<QuerySnapshot, QueryEvaluationError> {
            Err(QueryEvaluationError::Refused(
                "injected evaluation failure".to_owned(),
            ))
        }
    }

    #[test]
    fn second_source_open_failure_closes_the_first_source() {
        let closes = Arc::new(AtomicUsize::new(0));
        let observer = Observer::new(
            Arc::new(TrackingSource {
                role: SourceKind::EventCache,
                closes: Arc::clone(&closes),
            }),
            Arc::new(RefusingSource),
            Arc::new(EmptyEvaluator),
        );

        let result = observer.open(EventQuery::events().cache_only());

        assert!(matches!(
            result,
            Err(ObserveError::SourceOpen {
                role: SourceKind::WriteStore,
                ..
            })
        ));
        assert_eq!(closes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn initial_evaluation_failure_closes_both_sources() {
        let closes = Arc::new(AtomicUsize::new(0));
        let observer = Observer::new(
            Arc::new(TrackingSource {
                role: SourceKind::EventCache,
                closes: Arc::clone(&closes),
            }),
            Arc::new(TrackingSource {
                role: SourceKind::WriteStore,
                closes: Arc::clone(&closes),
            }),
            Arc::new(FailingEvaluator),
        );

        let result = observer.open(EventQuery::events().cache_only());

        assert!(matches!(result, Err(ObserveError::Evaluation(_))));
        assert_eq!(closes.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_open_source_closure_is_scoped_evidence() {
        let closes = Arc::new(AtomicUsize::new(0));
        let observer = Observer::new(
            Arc::new(TrackingSource {
                role: SourceKind::EventCache,
                closes: Arc::clone(&closes),
            }),
            Arc::new(TrackingSource {
                role: SourceKind::WriteStore,
                closes: Arc::clone(&closes),
            }),
            Arc::new(EmptyEvaluator),
        );
        let mut observation = observer
            .open(EventQuery::events().cache_only())
            .expect("initial sources open");

        for _ in 0..2 {
            if observation
                .current()
                .evidence
                .sources
                .iter()
                .all(|source| source.status == SourceStatus::Closed)
            {
                break;
            }
            observation
                .changed()
                .await
                .expect("source closure updates rather than closing the query");
        }

        assert!(
            observation
                .current()
                .evidence
                .sources
                .iter()
                .all(|source| source.status == SourceStatus::Closed)
        );
        assert_eq!(closes.load(Ordering::SeqCst), 2);
        observation.close();
    }
}
