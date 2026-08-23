//! Component evidence for the third query-source role and evaluator totality.
//!
//! Authority: `GOALS:344-350` (QUERY-005) — admitted live relay occurrences are
//! current query input even when the selected event cache does not retain them.
//! The evaluator must remain pure, total, and panic-free over every source role.

use fava_query::{
    Query, QueryEvaluator, QuerySnapshot, SourceEvent, SourceKind, SourceRevision, SourceSnapshot,
    SourceStatus, SourceTerminationCause,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;

fn session(url: &str) -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse(url).expect("test relay url parses"),
        RelayAccess::public(),
    )
}

fn signed(keys: &Keys, created_at: u64, content: &str) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn admitted(event: Event, session: RelaySessionKey) -> SourceEvent {
    SourceEvent::Cached(CachedEvent::new(
        event,
        RelayEvidence::one(session, Timestamp::from(1)),
    ))
}

fn live_relay(session: RelaySessionKey, events: Vec<SourceEvent>) -> SourceSnapshot {
    SourceSnapshot {
        kind: SourceKind::LiveRelay { session },
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events,
    }
}

/// A profile with no retaining event cache still delivers verified live relay
/// occurrences into the query result.
#[test]
fn live_relay_occurrences_enter_results_without_a_retaining_cache() {
    let keys = Keys::generate();
    let served = session("wss://live.example");
    let event = signed(&keys, 100, "only ever seen live");

    let sources = [live_relay(
        served.clone(),
        vec![admitted(event.clone(), served.clone())],
    )];

    let result = StandardQueryEvaluator
        .evaluate(&Query::events(), &sources)
        .expect("evaluation succeeds");

    assert_eq!(
        result.events.len(),
        1,
        "an admitted live occurrence is current query input with no cache present"
    );
    assert_eq!(result.events[0].id(), event.id);
    assert!(
        result.events[0]
            .relay_evidence
            .observations()
            .any(|observation| observation.session == served),
        "the serving relay session survives into the record's evidence"
    );

    let evidence = result
        .evidence
        .source(&SourceKind::LiveRelay {
            session: served.clone(),
        })
        .expect("live relay source evidence is reported");
    assert_eq!(evidence.revision, SourceRevision(1));
    assert_eq!(evidence.status, SourceStatus::Open);
}

/// Live relay contributions merge with cache and write-store contributions
/// under one universal merge, and each role keeps its own scoped evidence.
#[test]
fn every_source_role_reports_its_own_scoped_evidence() {
    let keys = Keys::generate();
    let served = session("wss://live.example");
    let event = signed(&keys, 100, "shared");

    let sources = [
        SourceSnapshot::empty(SourceKind::EventCache),
        SourceSnapshot {
            status: SourceStatus::Closed {
                cause: SourceTerminationCause::ProviderClosed,
            },
            ..SourceSnapshot::empty(SourceKind::WriteStore)
        },
        live_relay(served.clone(), vec![admitted(event, served.clone())]),
    ];

    let result = StandardQueryEvaluator
        .evaluate(&Query::events(), &sources)
        .expect("evaluation succeeds");

    assert_eq!(result.evidence.sources.len(), 3);
    assert_eq!(
        result
            .evidence
            .source(&SourceKind::WriteStore)
            .expect("write store evidence")
            .status,
        SourceStatus::Closed {
            cause: SourceTerminationCause::ProviderClosed
        },
        "a terminated source keeps the cause that ended it"
    );
    assert_eq!(
        result
            .evidence
            .source(&SourceKind::EventCache)
            .expect("event cache evidence")
            .status,
        SourceStatus::Open
    );
    assert!(
        result
            .evidence
            .source(&SourceKind::LiveRelay { session: served })
            .is_some(),
        "a live relay role is addressable by its exact session"
    );
}

/// Two live-relay sources for different sessions are independent source roles,
/// not one collapsed role.
#[test]
fn live_relay_sources_are_scoped_per_session() {
    let keys = Keys::generate();
    let first = session("wss://first.example");
    let second = session("wss://second.example");
    let event = signed(&keys, 100, "served twice");

    let sources = [
        live_relay(first.clone(), vec![admitted(event.clone(), first.clone())]),
        live_relay(second.clone(), vec![admitted(event, second.clone())]),
    ];

    let result = StandardQueryEvaluator
        .evaluate(&Query::events(), &sources)
        .expect("evaluation succeeds");

    assert_eq!(result.events.len(), 1, "one event id yields one record");
    assert_eq!(
        result.events[0].relay_evidence.len(),
        2,
        "both serving sessions merge into one record's evidence"
    );
    assert_eq!(result.evidence.sources.len(), 2);
    assert_ne!(
        SourceKind::LiveRelay {
            session: first.clone()
        },
        SourceKind::LiveRelay { session: second },
        "live relay roles are distinguished by their session"
    );
}

/// The evaluator stays pure and total: identical inputs give identical outputs,
/// argument order does not change the answer, and no adversarial arrangement of
/// source roles produces a panic or an error.
#[test]
fn evaluation_is_pure_and_total_over_every_source_arrangement() {
    let keys = Keys::generate();
    let served = session("wss://live.example");
    let older = signed(&keys, 10, "older");
    let newer = signed(&keys, 20, "newer");

    let roles = [
        SourceSnapshot::empty(SourceKind::EventCache),
        SourceSnapshot::empty(SourceKind::WriteStore),
        live_relay(
            served.clone(),
            vec![
                admitted(older.clone(), served.clone()),
                admitted(newer.clone(), served.clone()),
                admitted(older.clone(), served.clone()),
            ],
        ),
        SourceSnapshot {
            status: SourceStatus::Closed {
                cause: SourceTerminationCause::Shutdown,
            },
            revision: SourceRevision(u64::MAX),
            ..live_relay(session("wss://gone.example"), Vec::new())
        },
    ];

    let ids = |snapshot: &QuerySnapshot| -> Vec<_> {
        snapshot
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect()
    };

    let query = Query::events();
    let first = StandardQueryEvaluator
        .evaluate(&query, &roles)
        .expect("evaluation is total");
    let second = StandardQueryEvaluator
        .evaluate(&query, &roles)
        .expect("evaluation is total");
    assert_eq!(first.events, second.events, "evaluation is deterministic");

    let mut reversed: Vec<_> = roles.to_vec();
    reversed.reverse();
    let reordered = StandardQueryEvaluator
        .evaluate(&query, &reversed)
        .expect("evaluation is total");
    assert_eq!(
        ids(&first),
        ids(&reordered),
        "source ordering does not change the merged answer"
    );

    // No source at all, and every single role alone, are all total.
    assert!(StandardQueryEvaluator.evaluate(&query, &[]).is_ok());
    for role in &roles {
        assert!(
            StandardQueryEvaluator
                .evaluate(&query, std::slice::from_ref(role))
                .is_ok(),
            "every source role alone evaluates without refusal"
        );
    }
}

/// The evaluator and its state rules contain no panic-capable construct.
///
/// This property is load-bearing: an application-supplied source must never be
/// able to abort the process through evaluation.
#[test]
fn the_evaluator_contains_no_panic_capable_construct() {
    let source = include_str!("../src/lib.rs");
    let code: Vec<&str> = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();

    for forbidden in [".unwrap(", ".expect(", "panic!", "unreachable!", "todo!"] {
        let hits: Vec<_> = code
            .iter()
            .filter(|line| line.contains(forbidden))
            .collect();
        assert!(
            hits.is_empty(),
            "evaluator must stay panic-free, found {forbidden} in {hits:?}"
        );
    }

    for line in &code {
        assert!(
            !line.contains("[i]") && !line.contains("[index]") && !line.contains("[0]"),
            "evaluator must not index a slice: {line}"
        );
    }
}
