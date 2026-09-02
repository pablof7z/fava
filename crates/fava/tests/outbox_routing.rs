//! Public outbox routing behavior on the single router-input path.

use std::sync::Arc;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{
    EventRecord, Freshness, QueryEvidence, QueryRevision, QuerySnapshot, RelayQueryEvidence,
    RelaySourceState, RouteOrigin, Timestamp,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_router_outbox::OutboxRouter;
use fava_routing::{CoverageState, RoutePlan, RouteRequest, RouteTarget, Router};
use fava_state::relay_occurrences_for_event;
use fava_subscriptions_no_grouping::planner;
use fava_transport_testkit::FakeTransport;
use fava_write::{EventBuilder, EventValue, Kind, Tag, UnsignedEvent};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::{Keys, PublicKey, SecretKey};
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
        transport
            .relay(&session(indexer), &Authority::Unauthenticated)
            .is_none(),
        "preview must not open a relay session or emit a REQ"
    );
}

#[test]
fn every_author_a_query_names_is_routed() {
    let indexer = relay("indexer");
    let outbox = relay("outbox");
    let authors: Vec<PublicKey> = (0..600_u32).map(author).collect();
    let lists = authors
        .iter()
        .map(|author| relay_list(*author, [outbox.clone()]))
        .collect();
    let router: Arc<dyn Router> =
        Arc::new(OutboxRouter::new("outbox", [indexer.clone()]).expect("router"));
    let request = RouteRequest::Read(
        Query::events()
            .authors(authors.iter().copied())
            .expect("six hundred authors"),
    );

    let mut session = fava_routing::open(
        std::slice::from_ref(&router),
        &request,
        &[vec![indexed_snapshot(indexer, lists)]],
    )
    .expect("open");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert_eq!(plan.shortfalls, Vec::<String>::new());
    assert_eq!(plan.destinations[&outbox].targets.len(), authors.len());
    assert!(
        authors.iter().all(|author| matches!(
            plan.coverage.get(&RouteTarget::Author(*author)),
            Some(CoverageState::Covered(_))
        )),
        "every named author is covered by its outbox relay"
    );
    session.close();
}

#[test]
fn a_relay_list_of_ten_thousand_relays_is_refused() {
    let indexer = relay("indexer");
    let author = author(0);
    let lists = vec![relay_list(
        author,
        (0..10_000_u32).map(|index| relay(&format!("supplied-{index}"))),
    )];
    let router: Arc<dyn Router> =
        Arc::new(OutboxRouter::new("outbox", [indexer.clone()]).expect("router"));
    let request = RouteRequest::Read(Query::events().authors([author]).expect("one author"));

    let mut session = fava_routing::open(
        std::slice::from_ref(&router),
        &request,
        &[vec![indexed_snapshot(indexer, lists)]],
    )
    .expect("open");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("route destinations exceed bound: 10000 > 256")),
        "a relay-supplied list of ten thousand relays stays refused: {:?}",
        plan.shortfalls
    );
    assert!(plan.destinations.is_empty());
    session.close();
}

fn author(index: u32) -> PublicKey {
    let mut bytes = [1_u8; 32];
    bytes[..4].copy_from_slice(&index.to_be_bytes());
    Keys::new(SecretKey::from_slice(&bytes).expect("secret key")).public_key()
}

fn relay_list(author: PublicKey, relays: impl IntoIterator<Item = RelayUrl>) -> EventValue {
    let mut event = UnsignedEvent::new(
        author,
        Timestamp::from(1),
        Kind::from(10_002_u16),
        relays
            .into_iter()
            .map(|relay| Tag::parse(["r", relay.as_str(), "write"]).expect("relay tag")),
        "",
    );
    event.ensure_id();
    EventValue::Unsigned(event)
}

fn indexed_snapshot(indexer: RelayUrl, lists: Vec<EventValue>) -> QuerySnapshot {
    QuerySnapshot {
        events: lists
            .into_iter()
            .map(|event| {
                let id = event.id().expect("finalized event");
                let occurrences = relay_occurrences_for_event(id, &[]).expect("occurrences");
                EventRecord::new(event, occurrences, None).expect("record")
            })
            .collect(),
        ..snapshot(
            indexer,
            RelaySourceState::StoredEventsComplete {
                at: Timestamp::from(1),
            },
        )
    }
}

fn write_request(author: nostr::key::PublicKey) -> RouteRequest {
    let event = EventBuilder::new(Kind::TextNote)
        .by(author)
        .build()
        .expect("event");
    RouteRequest::Write {
        event: EventValue::Unsigned(event),
        access: fava_relay::Authority::Unauthenticated,
    }
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

fn session(relay: RelayUrl) -> RelayUrl {
    relay
}
