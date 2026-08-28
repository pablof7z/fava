//! Public outbox routing behavior on the single router-input path.

use std::sync::Arc;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{
    Freshness, QueryEvidence, QueryRevision, QuerySnapshot, RelayQueryEvidence, RelaySourceState,
    RouteOrigin, Timestamp,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_outbox::OutboxRouter;
use fava_routing::{CoverageState, RoutePlan, RouteRequest, RouteTarget, Router};
use fava_subscriptions_no_grouping::planner;
use fava_transport_testkit::FakeTransport;
use fava_write::{EventBuilder, EventValue, Kind};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[test]
fn outbox_declares_one_max_age_query_for_all_requested_authors() {
    let author = Keys::generate().public_key();
    let indexer = relay("indexer");
    let router = OutboxRouter::new("outbox", [indexer.clone()]).expect("router");
    let request = RouteRequest::Read(Query::events().authors([author]).expect("one author"));

    let declared = router
        .queries(&request, &RoutePlan::default())
        .expect("declaration");
    assert_eq!(declared.len(), 1);
    assert!(matches!(declared[0].freshness(), Freshness::MaxAge(_)));
    assert_eq!(
        declared[0].source().acquisition(),
        &fava_query::QueryAcquisition::Explicit([indexer].into())
    );
}

#[test]
fn outbox_replaces_whole_snapshot_truth() {
    let author = Keys::generate().public_key();
    let indexer = relay("indexer");
    let router = OutboxRouter::new("outbox", [indexer.clone()]).expect("router");
    let request = write_request(author);
    let mut session = router
        .open(
            request.clone(),
            Arc::new(RoutePlan::default()),
            vec![snapshot(
                indexer.clone(),
                RelaySourceState::Open {
                    requested_at: Timestamp::from(1),
                },
            )],
        )
        .expect("open");
    assert_eq!(
        session.current().coverage.get(&RouteTarget::Author(author)),
        Some(&CoverageState::Unresolved)
    );

    let replaced = session
        .replace(
            Arc::new(RoutePlan::default()),
            vec![snapshot(
                indexer,
                RelaySourceState::StoredEventsComplete {
                    at: Timestamp::from(2),
                },
            )],
        )
        .expect("complete replacement");
    assert_eq!(
        replaced.coverage.get(&RouteTarget::Author(author)),
        Some(&CoverageState::SettledAbsent),
        "the newest complete snapshot replaces the prior unresolved truth"
    );
    session.close();
}

#[test]
fn zero_indexers_is_unresolved_not_absent() {
    let author = Keys::generate().public_key();
    let router = OutboxRouter::new("outbox", []).expect("router");
    let mut session = router
        .open(
            write_request(author),
            Arc::new(RoutePlan::default()),
            Vec::new(),
        )
        .expect("open without indexers");
    assert_eq!(
        session.current().coverage.get(&RouteTarget::Author(author)),
        Some(&CoverageState::Unresolved)
    );
    session.close();
}

#[test]
fn public_preview_is_local_only() {
    let author = Keys::generate().public_key();
    let indexer = relay("indexer");
    let transport = Arc::new(FakeTransport::new());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .router(Arc::new(
            OutboxRouter::new("outbox", [indexer.clone()]).expect("router"),
        ))
        .build()
        .expect("assembly");

    let plan = fava
        .preview_routes(&Query::events().authors([author]).expect("one author"))
        .expect("preview");
    assert_eq!(
        plan.coverage.get(&RouteTarget::Author(author)),
        Some(&CoverageState::Unresolved)
    );
    assert!(
        transport.relay(&session(indexer)).is_none(),
        "preview must not open a relay session or emit a REQ"
    );
}

fn write_request(author: nostr::key::PublicKey) -> RouteRequest {
    let event = EventBuilder::new(author, Kind::TextNote)
        .build()
        .expect("event");
    RouteRequest::Write(EventValue::Unsigned(event))
}

fn snapshot(indexer: RelayUrl, state: RelaySourceState) -> QuerySnapshot {
    QuerySnapshot {
        revision: QueryRevision(1),
        events: Arc::from([]),
        evidence: QueryEvidence {
            relays: vec![RelayQueryEvidence {
                session: session(indexer),
                generation: None,
                plan_revision: 1,
                branches: Vec::new(),
                state,
                shared_with: Vec::new(),
                shortfall: None,
                route: RouteOrigin::Explicit,
            }],
            ..QueryEvidence::default()
        },
    }
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay")
}

fn session(relay: RelayUrl) -> RelaySessionKey {
    RelaySessionKey {
        relay,
        access: RelayAccess::Public,
    }
}
