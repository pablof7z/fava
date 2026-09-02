//! Live occurrence aggregation is bounded by installed exact sessions.

mod support;

use std::sync::Arc;
use std::sync::Mutex;

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QueryEvaluationError, QueryEvaluator, QueryShortfall, QuerySnapshot,
    QuerySource, QuerySourceError, SourceChangeFuture, SourceChanges, SourceKind, SourceSnapshot,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{EventStateMutation, RelayEvent};
use fava_wire::RelayMessage;
use nostr::event::{EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use support::{assemble, push, relay, requests, wait_until};

#[derive(Default)]
struct NullCache;

impl QuerySource for NullCache {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        Ok(OpenedQuerySource {
            initial: SourceSnapshot::empty(SourceKind::EventCache),
            changes: Box::new(NullChanges),
        })
    }
}

struct NullChanges;

impl SourceChanges for NullChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}

impl EventCache for NullCache {
    fn source_coverage(
        &self,
        _session: &nostr::types::RelayUrl,
        _filter: &nostr::filter::Filter,
    ) -> Result<Option<fava_query::SourceCoverage>, EventCacheError> {
        Ok(None)
    }

    fn retain_source_coverage(
        &self,
        _coverage: fava_query::SourceCoverage,
    ) -> Result<(), EventCacheError> {
        Ok(())
    }

    fn transact(
        &self,
        decide: &dyn Fn(&[RelayEvent]) -> Vec<EventStateMutation>,
    ) -> Result<usize, EventCacheError> {
        let _ = decide(&[]);
        Ok(0)
    }

    fn commit(&self, _mutations: Vec<EventStateMutation>) -> Result<(), EventCacheError> {
        Ok(())
    }

    fn event(&self, _id: nostr::event::EventId) -> Result<Option<RelayEvent>, EventCacheError> {
        Ok(None)
    }

    fn len(&self) -> Result<usize, EventCacheError> {
        Ok(0)
    }
}

#[derive(Default)]
struct RecordingEvaluator {
    live_retractions: Mutex<Vec<Vec<nostr::event::EventId>>>,
}

impl RecordingEvaluator {
    fn latest_live_retractions(&self) -> Vec<nostr::event::EventId> {
        self.live_retractions
            .lock()
            .expect("recording evaluator lock")
            .last()
            .cloned()
            .unwrap_or_default()
    }

    fn saw_live_retraction(&self, event_id: nostr::event::EventId) -> bool {
        self.live_retractions
            .lock()
            .expect("recording evaluator lock")
            .iter()
            .any(|revision| revision.contains(&event_id))
    }
}

impl QueryEvaluator for RecordingEvaluator {
    fn evaluate(
        &self,
        query: &Query,
        sources: &[SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError> {
        self.live_retractions
            .lock()
            .expect("recording evaluator lock")
            .push(
                sources
                    .iter()
                    .filter(|source| matches!(source.kind, SourceKind::LiveRelay { .. }))
                    .flat_map(|source| source.retractions.iter().map(|item| item.event_id))
                    .collect(),
            );
        StandardQueryEvaluator.evaluate(query, sources)
    }
}

#[tokio::test(flavor = "current_thread")]
async fn replay_cannot_exceed_the_current_installed_session_count()
-> Result<(), Box<dyn std::error::Error>> {
    let assembly = assemble();
    let a = relay("bound-a");
    let b = relay("bound-b");
    let observation = assembly
        .observer
        .open(Query::events().only_from_relays([a.clone(), b.clone()])?)?;
    wait_until(|| assembly.peer(&a).is_some() && assembly.peer(&b).is_some()).await;
    let a_peer = assembly.established(&a);
    let b_peer = assembly.established(&b);
    wait_until(|| requests(Some(a_peer.clone())).len() == 1).await;
    wait_until(|| requests(Some(b_peer.clone())).len() == 1).await;
    let a_wire = requests(Some(a_peer.clone()))[0].0.clone();
    let b_wire = requests(Some(b_peer.clone()))[0].0.clone();
    let event = EventBuilder::new(Kind::TextNote, "replayed").finalize(&Keys::generate())?;
    for _ in 0..16 {
        push(&a_peer, &RelayMessage::event(a_wire.clone(), event.clone()));
    }
    push(&b_peer, &RelayMessage::event(b_wire, event.clone()));
    wait_until(|| {
        observation
            .current()
            .events
            .first()
            .is_some_and(|record| record.relay_occurrences().len() == 2)
    })
    .await;
    assert_eq!(observation.current().events.len(), 1);
    assert_eq!(observation.current().events[0].relay_occurrences().len(), 2);
    observation.close();
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
#[allow(clippy::too_many_lines)] // One causal sequence proves atomic replacement, deletion, then refusal at the exact bound.
async fn live_retention_applies_net_bounded_changes_then_refuses_overflow()
-> Result<(), Box<dyn std::error::Error>> {
    const LIVE_LIMIT: usize = 4_096;
    let assembly = assemble();
    let null = Arc::new(NullCache);
    let source: Arc<dyn QuerySource> = null.clone();
    let recording = Arc::new(RecordingEvaluator::default());
    let evaluator: Arc<dyn QueryEvaluator> = recording.clone();
    let observer = assembly
        .with_local(source, evaluator)
        .with_event_cache(null);
    let relay = relay("retention-bound");
    let observation = observer.open(Query::events().only_from_relays([relay.clone()])?)?;
    wait_until(|| assembly.peer(&relay).is_some()).await;
    let peer = assembly.established(&relay);
    wait_until(|| requests(Some(peer.clone())).len() == 1).await;
    let wire = requests(Some(peer.clone()))[0].0.clone();
    let author = Keys::generate();

    let replaceable = EventBuilder::new(Kind::Metadata, "replaceable-old")
        .custom_created_at(nostr::types::Timestamp::from(1))
        .finalize(&author)?;
    let deleted_target = EventBuilder::new(Kind::TextNote, "delete-at-capacity")
        .custom_created_at(nostr::types::Timestamp::from(2))
        .finalize(&author)?;
    push(
        &peer,
        &RelayMessage::event(wire.clone(), replaceable.clone()),
    );
    push(
        &peer,
        &RelayMessage::event(wire.clone(), deleted_target.clone()),
    );
    for index in 2..LIVE_LIMIT {
        let event = EventBuilder::new(Kind::TextNote, index.to_string())
            .custom_created_at(nostr::types::Timestamp::from(index as u64 + 1))
            .finalize(&author)?;
        push(&peer, &RelayMessage::event(wire.clone(), event));
        if index % 128 == 127 {
            wait_until(|| observation.current().events.len() == index + 1).await;
        }
    }
    wait_until(|| observation.current().events.len() == LIVE_LIMIT).await;
    assert!(
        !observation
            .current()
            .evidence
            .shortfalls
            .iter()
            .any(|shortfall| { matches!(shortfall, QueryShortfall::LiveRetentionLimit { .. }) })
    );

    let replacement = EventBuilder::new(Kind::Metadata, "replaceable-new")
        .custom_created_at(nostr::types::Timestamp::from(LIVE_LIMIT as u64 + 1))
        .finalize(&author)?;
    push(
        &peer,
        &RelayMessage::event(wire.clone(), replacement.clone()),
    );
    wait_until(|| {
        let current = observation.current();
        current.events.len() == LIVE_LIMIT
            && current
                .events
                .iter()
                .any(|record| record.id() == replacement.id)
            && current
                .events
                .iter()
                .all(|record| record.id() != replaceable.id)
    })
    .await;
    assert!(
        recording.saw_live_retraction(replaceable.id),
        "the accepted replacement revision must expose its exact retraction"
    );

    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(deleted_target.id))
        .custom_created_at(nostr::types::Timestamp::from(LIVE_LIMIT as u64 + 2))
        .finalize(&author)?;
    push(&peer, &RelayMessage::event(wire.clone(), deletion.clone()));
    wait_until(|| {
        let current = observation.current();
        current.events.len() == LIVE_LIMIT
            && current
                .events
                .iter()
                .any(|record| record.id() == deletion.id)
            && current
                .events
                .iter()
                .all(|record| record.id() != deleted_target.id)
    })
    .await;
    assert!(
        recording.saw_live_retraction(deleted_target.id),
        "the accepted deletion revision must expose its exact retraction"
    );
    assert!(
        !observation
            .current()
            .evidence
            .shortfalls
            .iter()
            .any(|shortfall| { matches!(shortfall, QueryShortfall::LiveRetentionLimit { .. }) })
    );

    let before_overflow = observation
        .current()
        .events
        .iter()
        .map(fava_query::EventRecord::id)
        .collect::<Vec<_>>();

    let overflow = EventBuilder::new(Kind::TextNote, "overflow")
        .custom_created_at(nostr::types::Timestamp::from(LIVE_LIMIT as u64 + 3))
        .finalize(&author)?;
    push(&peer, &RelayMessage::event(wire, overflow.clone()));
    wait_until(|| {
        observation
            .current()
            .evidence
            .shortfalls
            .iter()
            .any(|shortfall| {
                matches!(
                    shortfall,
                    QueryShortfall::LiveRetentionLimit {
                        limit,
                        refused: 1,
                        ..
                    } if limit.get() == LIVE_LIMIT
                )
            })
    })
    .await;

    let current = observation.current();
    assert_eq!(
        current
            .events
            .iter()
            .map(fava_query::EventRecord::id)
            .collect::<Vec<_>>(),
        before_overflow,
        "overflow refusal must preserve the complete accepted live state"
    );
    assert_eq!(current.events.len(), LIVE_LIMIT);
    assert!(
        current
            .events
            .iter()
            .all(|record| record.id() != overflow.id)
    );
    assert!(current.evidence.shortfalls.iter().any(|shortfall| {
        matches!(
            shortfall,
            QueryShortfall::LiveRetentionLimit {
                session,
                limit,
                refused: 1,
            } if *session == relay && limit.get() == LIVE_LIMIT
        )
    }));
    assert!(
        recording.latest_live_retractions().is_empty(),
        "the refused overflow revision must not repeat replacement/deletion retractions"
    );
    observation.close();
    Ok(())
}
