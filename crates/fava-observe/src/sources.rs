//! Local source opening, revision merge, evidence decoration, and delivery.

use std::sync::Arc;

use fava_diagnostics::Diagnostics;
use fava_query::{
    ObservationId, OpenedQuerySource, Query, QueryEvaluator, QueryEvidence, QueryRevision,
    QueryShortfall, QuerySnapshot, SourceChanges, SourceKind, SourceSnapshot, SourceStatus,
    SourceTerminationCause,
};
use fava_runtime::{CancellationToken, Runtime, TaskHandle, TaskName};
use tokio::sync::watch;

use crate::diagnostics;
use crate::registry::Registry;

/// Coalescing report for current-state revisions superseded at a watch boundary.
pub(crate) type Coalesced = Arc<dyn Fn(u64) + Send + Sync>;

/// Both local sources opened at one coherent boundary.
pub(crate) struct OpenSources {
    pub(crate) snapshots: Vec<SourceSnapshot>,
    pub(crate) cache: Box<dyn SourceChanges>,
    pub(crate) writes: Box<dyn SourceChanges>,
}

impl OpenSources {
    /// Build the source boundary from two opened providers.
    pub(crate) fn new(cache: OpenedQuerySource, writes: OpenedQuerySource) -> Self {
        Self {
            snapshots: vec![cache.initial, writes.initial],
            cache: cache.changes,
            writes: writes.changes,
        }
    }

    /// Release both sources without delivering anything.
    pub(crate) fn close(mut self) {
        self.cache.close();
        self.writes.close();
    }
}

/// Everything one observation's projection task needs.
pub(crate) struct Projection {
    pub(crate) id: ObservationId,
    pub(crate) registry: Arc<Registry>,
    pub(crate) diagnostics: Arc<Diagnostics>,
    pub(crate) evaluator: Arc<dyn QueryEvaluator>,
    pub(crate) coalesced: Option<Coalesced>,
    pub(crate) cancel: CancellationToken,
    pub(crate) woken: watch::Receiver<u64>,
}

/// Deliver later merged revisions until the observation is cancelled.
pub(crate) fn project(
    runtime: &Runtime,
    projection: Projection,
    query: Query,
    sources: OpenSources,
    initial: QuerySnapshot,
) -> (
    watch::Receiver<Arc<QuerySnapshot>>,
    Option<TaskHandle<Option<()>>>,
) {
    let (latest_tx, latest) = watch::channel(Arc::new(initial));
    let task = runtime
        .spawn_cancellable(
            TaskName("observe.projection"),
            projection.cancel.clone(),
            deliver(projection, query, sources, latest_tx),
        )
        .ok();
    (latest, task)
}

/// What woke the projection loop.
enum Wake {
    /// One local source produced a change or ended.
    Source(
        SourceKind,
        Box<Result<SourceSnapshot, fava_query::QuerySourceClosed>>,
    ),
    /// Owner-held relay or plan evidence moved.
    Evidence,
}

async fn deliver(
    projection: Projection,
    query: Query,
    sources: OpenSources,
    latest_tx: watch::Sender<Arc<QuerySnapshot>>,
) {
    let OpenSources {
        mut snapshots,
        mut cache,
        mut writes,
    } = sources;
    let Projection {
        id,
        registry,
        diagnostics: facts,
        evaluator,
        coalesced,
        cancel: _cancel,
        mut woken,
    } = projection;
    let mut revision = 1_u64;
    let mut cache_open = true;
    let mut writes_open = true;
    loop {
        let wake = tokio::select! {
            biased;
            changed = woken.changed() => {
                if changed.is_err() || *woken.borrow_and_update() == u64::MAX {
                    break;
                }
                Wake::Evidence
            }
            result = cache.next_change(), if cache_open => {
                Wake::Source(SourceKind::EventCache, Box::new(result))
            }
            result = writes.next_change(), if writes_open => {
                Wake::Source(SourceKind::WriteStore, Box::new(result))
            }
        };

        let evaluate = match wake {
            Wake::Evidence => false,
            Wake::Source(role, result) => match *result {
                Ok(snapshot) => {
                    if let Some(current) = snapshots.iter().find(|source| source.kind == role) {
                        report_skipped(
                            coalesced.as_deref(),
                            &registry,
                            id,
                            current.revision.0,
                            snapshot.revision.0,
                        );
                    }
                    replace_source(&mut snapshots, snapshot);
                    true
                }
                Err(closed) => {
                    if role == SourceKind::EventCache {
                        cache_open = false;
                        cache.close();
                    } else {
                        writes_open = false;
                        writes.close();
                    }
                    mark_source_closed(&mut snapshots, &role, closed.cause);
                    true
                }
            },
        };

        let mut next = if evaluate {
            match evaluator.evaluate(&query, &snapshots) {
                Ok(snapshot) => snapshot,
                Err(_refused) => break,
            }
        } else {
            rebase(&latest_tx.borrow())
        };
        let Some(bumped) = revision.checked_add(1) else {
            break;
        };
        revision = bumped;
        next.revision = QueryRevision::new(revision);
        decorate(&registry, id, &mut next.evidence);
        publish(&facts, &registry, id, &next.evidence);
        latest_tx.send_replace(Arc::new(next));
    }
    cache.close();
    writes.close();
    facts.forget_query(id);
}

/// Carry the current events and source evidence into a new evidence-only revision.
fn rebase(current: &QuerySnapshot) -> QuerySnapshot {
    QuerySnapshot {
        revision: current.revision,
        events: Arc::clone(&current.events),
        evidence: QueryEvidence {
            sources: current.evidence.sources.clone(),
            ..QueryEvidence::default()
        },
    }
}

/// Attach the owner-held relay and plan evidence to one delivered snapshot.
pub(crate) fn decorate(registry: &Registry, id: ObservationId, evidence: &mut QueryEvidence) {
    let owned = registry.evidence(id);
    evidence.relays = owned.relays;
    evidence.plan = owned.plan;
    evidence
        .shortfalls
        .retain(|entry| !matches!(entry, QueryShortfall::CoalescedUpdates { .. }));
    if owned.coalesced > 0 {
        evidence.shortfalls.push(QueryShortfall::CoalescedUpdates {
            dropped: owned.coalesced,
        });
    }
}

/// Publish the ownership record for one observation.
pub(crate) fn publish(
    facts: &Diagnostics,
    registry: &Registry,
    id: ObservationId,
    evidence: &QueryEvidence,
) {
    let owned = registry.evidence(id);
    facts.query(diagnostics::query_fact(
        id,
        owned.route_revision,
        evidence.plan.as_ref().map(|plan| plan.revision),
        &evidence.relays,
        owned.coalesced,
    ));
}

fn report_skipped(
    report: Option<&(dyn Fn(u64) + Send + Sync)>,
    registry: &Registry,
    id: ObservationId,
    previous: u64,
    current: u64,
) {
    let skipped = current.saturating_sub(previous).saturating_sub(1);
    if skipped == 0 {
        return;
    }
    registry.record_coalesced(id, skipped);
    if let Some(report) = report {
        report(skipped);
    }
}

/// Report revisions the application never saw because a newer one replaced them.
pub(crate) fn report_delivery_gap(
    report: Option<&(dyn Fn(u64) + Send + Sync)>,
    previous: u64,
    current: u64,
) {
    let skipped = current.saturating_sub(previous).saturating_sub(1);
    if skipped > 0
        && let Some(report) = report
    {
        report(skipped);
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

/// Stamp the terminal fact the provider actually reported.
///
/// The cause is never fabricated here: a provider that failed and a provider
/// that closed cleanly are different facts, and `fava-router-outbox` settles
/// absence only on the latter.
fn mark_source_closed(
    sources: &mut [SourceSnapshot],
    role: &SourceKind,
    cause: SourceTerminationCause,
) {
    if let Some(source) = sources.iter_mut().find(|source| &source.kind == role) {
        source.status = SourceStatus::Closed { cause };
    }
}
