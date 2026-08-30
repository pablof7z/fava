//! Owner-level evidence for the observation open sequence and its refusals.

mod support;

use std::sync::Arc;
use std::time::Duration;

use fava_observe::{ObserveError, Observer};
use fava_query::{
    OpenedQuerySource, Query, QueryEvaluationError, QueryEvaluator, QuerySnapshot, QuerySource,
    QuerySourceError, RelaySourceState, RouteOrigin, SourceKind,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_write_store_memory::MemoryWriteStore;
use support::{
    assemble, refusal, relay, relay_evidence, requests, session_key, settle, wait_until,
};

#[tokio::test(flavor = "current_thread")]
async fn opening_a_live_query_never_awaits_the_transport() {
    let stalled = relay("stalled");
    let assembly = assemble();
    assembly
        .transport
        .hold_establishment(&session_key(&stalled));
    let query = Query::events()
        .only_from_relays([stalled.clone()])
        .expect("explicit relay is valid");

    let observation = tokio::time::timeout(Duration::from_millis(50), async {
        assembly.observer.open(query)
    })
    .await
    .expect("open must not await relay establishment")
    .expect("the coherent local observation opens");

    assert!(observation.current().events.is_empty());
    assert_eq!(assembly.transport.dials(&session_key(&stalled)), 0);
    assert!(assembly.peer(&stalled).is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn live_freshness_through_the_owner_contributes_relay_demand() {
    let reachable = relay("reachable");
    let assembly = assemble();

    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([reachable.clone()])
                .expect("explicit relay is valid"),
        )
        .expect("the live query opens");

    wait_until(|| assembly.peer(&reachable).is_some()).await;
    wait_until(|| requests(assembly.peer(&reachable)).len() == 1).await;
    assert_eq!(assembly.transport.dials(&session_key(&reachable)), 1);
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn a_cache_only_query_opens_no_relay_work() {
    let assembly = assemble();

    let observation = assembly
        .observer
        .open(Query::events().cache_only())
        .expect("the cache-only query opens");

    settle().await;
    assert!(assembly.planner.inputs().is_empty());
    assert!(observation.current().evidence.relays.is_empty());
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn an_explicit_open_reports_its_route_origin_before_any_relay_answers() {
    let reachable = relay("reachable");
    let assembly = assemble();

    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([reachable.clone()])
                .expect("explicit relay is valid"),
        )
        .expect("the live query opens");

    let planned = relay_evidence(&observation, &reachable);
    assert_eq!(planned.route, RouteOrigin::Explicit);
    assert!(matches!(
        planned.state,
        RelaySourceState::Planned | RelaySourceState::Connecting
    ));
    assert!(!planned.stored_events_complete());
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn a_live_query_without_a_transport_is_refused_before_any_source_opens() {
    let cache = Arc::new(fava_event_cache_memory::MemoryEventCache::default());
    let observer = Observer::new(
        cache,
        Arc::new(MemoryWriteStore::default()),
        Arc::new(StandardQueryEvaluator),
    );

    let error = refusal(
        observer.open(
            Query::events()
                .only_from_relays([relay("unreachable")])
                .expect("explicit relay is valid"),
        ),
    );

    assert!(matches!(error, ObserveError::Relay(_)));
}

#[tokio::test(flavor = "current_thread")]
async fn a_refused_local_source_leaves_no_relay_work_behind() {
    let assembly = assemble();
    let observer = assembly.with_local(Arc::new(RefusingSource), Arc::new(StandardQueryEvaluator));

    let error = refusal(
        observer.open(
            Query::events()
                .only_from_relays([relay("reachable")])
                .expect("explicit relay is valid"),
        ),
    );

    assert!(matches!(
        &error,
        ObserveError::SourceOpen { role, .. } if **role == SourceKind::EventCache
    ));
    settle().await;
    assert!(assembly.planner.inputs().is_empty());
    assert_eq!(
        assembly.transport.dials(&session_key(&relay("reachable"))),
        0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn initial_evaluation_failure_opens_no_relay_work() {
    let assembly = assemble();
    let observer = assembly.with_local(
        Arc::clone(&assembly.cache) as Arc<dyn QuerySource>,
        Arc::new(RefusingEvaluator),
    );

    let error = refusal(
        observer.open(
            Query::events()
                .only_from_relays([relay("reachable")])
                .expect("explicit relay is valid"),
        ),
    );

    assert!(matches!(error, ObserveError::Evaluation(_)));
    settle().await;
    assert_eq!(
        assembly.transport.dials(&session_key(&relay("reachable"))),
        0
    );
}

struct RefusingSource;

impl QuerySource for RefusingSource {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        Err(QuerySourceError::Refused(fava_query::BoundedText::new(
            "injected",
        )))
    }
}

struct RefusingEvaluator;

impl QueryEvaluator for RefusingEvaluator {
    fn evaluate(
        &self,
        _query: &Query,
        _sources: &[fava_query::SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError> {
        Err(QueryEvaluationError::Refused(fava_query::BoundedText::new(
            "injected",
        )))
    }
}
