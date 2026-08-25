//! Local evaluation evidence for repeated singleton kind selection.

use std::collections::BTreeSet;

use fava_query::{
    Query, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CachedEvent, RelayEvidence, Timestamp};
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind};
use nostr::key::Keys;

fn event(keys: &Keys, kind: Kind, created_at: u64) -> Event {
    EventBuilder::new(kind, format!("kind {}", kind.as_u16()))
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

#[test]
fn bounded_kind_set_local_selection_is_complete() {
    let keys = Keys::generate();
    let first = event(&keys, Kind::from_u16(30_001), 1);
    let second = event(&keys, Kind::from_u16(30_002), 2);
    let unrelated = event(&keys, Kind::from_u16(30_003), 3);
    let expected = BTreeSet::from([first.id, second.id]);
    let sources = [SourceSnapshot {
        kind: SourceKind::EventCache,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events: vec![first, second, unrelated]
            .into_iter()
            .map(|event| SourceEvent::Cached(CachedEvent::new(event, RelayEvidence::default())))
            .collect(),
    }];
    let query = Query::events()
        .kinds([Kind::from_u16(30_001), Kind::from_u16(30_002)])
        .expect("two kinds are bounded");

    let result = StandardQueryEvaluator
        .evaluate(&query, &sources)
        .expect("evaluation succeeds");
    let actual = result
        .events
        .iter()
        .map(fava_query::EventRecord::id)
        .collect::<BTreeSet<EventId>>();

    assert_eq!(actual, expected);
}
