//! Query aggregation recomputes provenance from current qualifying inputs.

use fava_query::{Query, QueryEvaluator, SourceEvent, SourceKind, SourceRevision, SourceSnapshot};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_state::RelayEvent;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::{RelayUrl, Timestamp};

#[test]
fn same_event_uses_earliest_time_per_serving_exact_session()
-> Result<(), Box<dyn std::error::Error>> {
    let event = EventBuilder::new(Kind::TextNote, "shared").finalize(&Keys::generate())?;
    let a = RelaySessionKey {
        relay: RelayUrl::parse("wss://a.example")?,
        access: RelayAccess::Public,
    };
    let b = RelaySessionKey {
        relay: RelayUrl::parse("wss://b.example")?,
        access: RelayAccess::Public,
    };
    let source = SourceSnapshot::current(
        SourceKind::EventCache,
        SourceRevision(1),
        vec![
            SourceEvent::Relay(RelayEvent::new(
                event.clone(),
                a.clone(),
                Timestamp::from(9),
            )),
            SourceEvent::Relay(RelayEvent::new(
                event.clone(),
                a.clone(),
                Timestamp::from(3),
            )),
            SourceEvent::Relay(RelayEvent::new(
                event.clone(),
                b.clone(),
                Timestamp::from(5),
            )),
        ],
    );
    let result = StandardQueryEvaluator.evaluate(
        &Query::events().with_relay_access(RelayAccess::Public),
        &[source],
    )?;
    let occurrences = result.events[0].relay_occurrences();
    assert_eq!(occurrences.event_id(), event.id);
    assert_eq!(occurrences.len(), 2);
    assert_eq!(
        occurrences
            .occurrences()
            .find(|item| item.session == a)
            .expect("a")
            .observed_at,
        Timestamp::from(3),
    );
    assert!(occurrences.occurrences().any(|item| item.session == b));
    Ok(())
}
