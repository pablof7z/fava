//! Live-query ownership over independent local sources and bounded latest-state delivery.

use std::sync::Arc;

use fava_query::{
    OpenedQuerySource, Query, QueryEvaluationError, QueryEvaluator, QueryRevision, QuerySnapshot,
    QuerySource, QuerySourceError, SourceKind, SourceSnapshot, SourceStatus,
    SourceTerminationCause,
};
use thiserror::Error;
use tokio::sync::watch;

/// Configured local observation owner.
#[derive(Clone)]
pub struct Observer {
    event_cache: Arc<dyn QuerySource>,
    write_store: Arc<dyn QuerySource>,
    evaluator: Arc<dyn QueryEvaluator>,
    coalesced: Option<Arc<dyn Fn(u64) + Send + Sync>>,
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
            coalesced: None,
        }
    }

    /// Report current-state revisions superseded at bounded watch boundaries.
    #[must_use]
    pub fn with_coalescing(mut self, report: Arc<dyn Fn(u64) + Send + Sync>) -> Self {
        self.coalesced = Some(report);
        self
    }

    /// Atomically open both local sources and return an immediately readable view.
    ///
    /// # Errors
    ///
    /// Returns [`ObserveError`] when either source open or initial evaluation
    /// fails. Any provisionally opened source is closed.
    pub fn open(&self, query: Query) -> Result<Observation, ObserveError> {
        let cache = self
            .event_cache
            .open(&query)
            .map_err(|error| ObserveError::SourceOpen {
                role: Box::new(SourceKind::EventCache),
                error,
            })?;
        let writes = match self.write_store.open(&query) {
            Ok(writes) => writes,
            Err(error) => {
                let mut cache_changes = cache.changes;
                cache_changes.close();
                return Err(ObserveError::SourceOpen {
                    role: Box::new(SourceKind::WriteStore),
                    error,
                });
            }
        };

        Observation::start(
            query,
            cache,
            writes,
            Arc::clone(&self.evaluator),
            self.coalesced.clone(),
        )
    }
}

/// One opened latest-state live query.
pub struct Observation {
    latest: watch::Receiver<Arc<QuerySnapshot>>,
    cancel: watch::Sender<bool>,
    additional_cancel: Vec<watch::Sender<bool>>,
    delivered_revision: QueryRevision,
    coalesced: Option<Arc<dyn Fn(u64) + Send + Sync>>,
}

impl Observation {
    fn start(
        query: Query,
        cache: OpenedQuerySource,
        writes: OpenedQuerySource,
        evaluator: Arc<dyn QueryEvaluator>,
        coalesced: Option<Arc<dyn Fn(u64) + Send + Sync>>,
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

        let task_coalesced = coalesced.clone();
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
                    if let Some(current) = sources.iter().find(|source| source.kind == role) {
                        report_skipped(
                            task_coalesced.as_deref(),
                            current.revision.0,
                            snapshot.revision.0,
                        );
                    }
                    replace_source(&mut sources, snapshot);
                } else {
                    if role == SourceKind::EventCache {
                        cache_open = false;
                        cache_changes.close();
                    } else {
                        writes_open = false;
                        write_changes.close();
                    }
                    mark_source_closed(&mut sources, &role);
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

        Ok(Self {
            latest,
            cancel,
            additional_cancel: Vec::new(),
            delivered_revision: QueryRevision(1),
            coalesced,
        })
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
        if *self.cancel.borrow() {
            return Err(ObservationClosed);
        }
        self.latest.changed().await.map_err(|_| ObservationClosed)?;
        if *self.cancel.borrow() {
            return Err(ObservationClosed);
        }
        let latest = Arc::clone(&self.latest.borrow_and_update());
        report_skipped(
            self.coalesced.as_deref(),
            self.delivered_revision.0,
            latest.revision.0,
        );
        self.delivered_revision = latest.revision;
        Ok(latest)
    }

    /// Attach one owner whose exact work must stop with this observation.
    pub fn attach_cancellation(&mut self, cancel: watch::Sender<bool>) {
        self.additional_cancel.push(cancel);
    }

    /// Close this observation. Repeated close is harmless.
    pub fn close(&self) {
        self.cancel.send_replace(true);
        for cancel in &self.additional_cancel {
            cancel.send_replace(true);
        }
    }
}

fn report_skipped(report: Option<&(dyn Fn(u64) + Send + Sync)>, previous: u64, current: u64) {
    let Some(report) = report else {
        return;
    };
    let skipped = current.saturating_sub(previous).saturating_sub(1);
    if skipped > 0 {
        report(skipped);
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        self.close();
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

fn mark_source_closed(sources: &mut [SourceSnapshot], role: &SourceKind) {
    if let Some(source) = sources.iter_mut().find(|source| &source.kind == role) {
        source.status = SourceStatus::Closed {
            cause: SourceTerminationCause::ProviderClosed,
        };
    }
}

/// Query-open refusal before a usable handle exists.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// One named local source could not establish its initial boundary.
    #[error("{role:?} failed to open: {error}")]
    SourceOpen {
        /// Query-source role. Boxed because [`SourceKind::LiveRelay`] carries a
        /// whole [`fava_state::RelaySessionKey`] — 120 bytes of relay URL that
        /// would otherwise widen every `Result<_, ObserveError>` on the open
        /// path, which is exactly what `clippy::result_large_err` refuses.
        role: Box<SourceKind>,
        /// Scoped provider refusal.
        error: QuerySourceError,
    },
    /// Initial local evaluation failed.
    #[error(transparent)]
    Evaluation(#[from] QueryEvaluationError),
    /// Relay work could not establish one exact live query.
    #[error("relay query refused: {0}")]
    Relay(String),
}

/// Terminal observation fact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("live query observation closed")]
pub struct ObservationClosed;

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use fava_query::{
        BoundedText, QuerySourceClosed, SourceChangeFuture, SourceChanges, SourceRevision,
    };

    use super::*;

    struct TrackingSource {
        role: SourceKind,
        closes: Arc<AtomicUsize>,
    }

    impl QuerySource for TrackingSource {
        fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
            Ok(OpenedQuerySource {
                initial: SourceSnapshot {
                    kind: self.role.clone(),
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
        fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
            Err(QuerySourceError::Refused(BoundedText::new(
                "injected open failure",
            )))
        }
    }

    struct EmptyEvaluator;

    impl QueryEvaluator for EmptyEvaluator {
        fn evaluate(
            &self,
            _query: &Query,
            sources: &[SourceSnapshot],
        ) -> Result<QuerySnapshot, QueryEvaluationError> {
            Ok(QuerySnapshot::evaluated(Vec::new(), sources))
        }
    }

    struct FailingEvaluator;

    impl QueryEvaluator for FailingEvaluator {
        fn evaluate(
            &self,
            _query: &Query,
            _sources: &[SourceSnapshot],
        ) -> Result<QuerySnapshot, QueryEvaluationError> {
            Err(QueryEvaluationError::Refused(BoundedText::new(
                "injected evaluation failure",
            )))
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

        let result = observer.open(Query::events().cache_only());

        assert!(matches!(
            &result,
            Err(ObserveError::SourceOpen { role, .. })
                if **role == SourceKind::WriteStore
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

        let result = observer.open(Query::events().cache_only());

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
            .open(Query::events().cache_only())
            .expect("initial sources open");

        for _ in 0..2 {
            if observation
                .current()
                .evidence
                .sources
                .iter()
                .all(|source| matches!(source.status, SourceStatus::Closed { .. }))
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
                .all(|source| matches!(source.status, SourceStatus::Closed { .. }))
        );
        assert_eq!(closes.load(Ordering::SeqCst), 2);
        observation.close();
    }
}
