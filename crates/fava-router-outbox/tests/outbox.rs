//! Outbox router declaration and unresolved-state behavior.

use std::sync::Arc;

use fava_query::{Freshness, Query};
use fava_router_outbox::OutboxRouter;
use fava_routing::{CoverageState, RoutePlan, RouteRequest, RouteTarget, Router};
use fava_write::{EventBuilder, EventValue, Kind};
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[test]
fn declares_one_bounded_max_age_kind_10002_query() {
    let author = Keys::generate();
    let indexer = RelayUrl::parse("wss://indexer.example").unwrap();
    let router = OutboxRouter::new("nip65", [indexer.clone()]).unwrap();
    let request = RouteRequest::Read(Query::events().authors([author.public_key()]).unwrap());
    let queries = router.queries(&request, &RoutePlan::default()).unwrap();
    assert_eq!(queries.len(), 1);
    assert!(matches!(queries[0].freshness(), Freshness::MaxAge(_)));
    assert_eq!(
        queries[0].selection().kinds,
        Some([Kind::from(10_002_u16)].into())
    );
    assert_eq!(
        queries[0].source().acquisition(),
        &fava_query::QueryAcquisition::Explicit([indexer].into())
    );
}

#[test]
fn zero_indexers_remains_unresolved() {
    let author = Keys::generate();
    let event = EventBuilder::new(Kind::TextNote)
        .by(author.public_key())
        .build()
        .unwrap();
    let request = RouteRequest::Write(EventValue::Unsigned(event));
    let router = OutboxRouter::new("nip65", []).unwrap();
    let mut session = router
        .open(request, Arc::new(RoutePlan::default()), Vec::new())
        .unwrap();
    assert_eq!(
        session
            .current()
            .coverage
            .get(&RouteTarget::Author(author.public_key())),
        Some(&CoverageState::Unresolved)
    );
    session.close();
}
