//! Live admitted state is independent of optional null retention.

mod support;

use std::sync::Arc;

use fava_event_cache::{EventCache, EventCacheError};
use fava_query::{
    OpenedQuerySource, Query, QueryEvaluator, QuerySource, QuerySourceError, SourceChangeFuture,
    SourceChanges, SourceKind, SourceSnapshot,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{EventStateMutation, RelayEvent};
use fava_wire::RelayMessage;
use nostr::event::{EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use nostr::types::Timestamp;
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
    fn transact(
        &self,
        decide: &dyn Fn(&[RelayEvent]) -> Vec<EventStateMutation>,
    ) -> Result<usize, EventCacheError> {
        let _ = decide(&[]);
        Err(EventCacheError::Refused(
            "retention deliberately unavailable".to_owned(),
        ))
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

#[tokio::test(flavor = "current_thread")]
async fn live_event_replacement_and_deletion_survive_refusing_null_retention()
-> Result<(), Box<dyn std::error::Error>> {
    let assembly = assemble();
    let null = Arc::new(NullCache);
    let source: Arc<dyn QuerySource> = null.clone();
    let evaluator: Arc<dyn QueryEvaluator> = Arc::new(StandardQueryEvaluator);
    let observer = assembly
        .with_local(source, evaluator)
        .with_event_cache(null.clone());
    let url = relay("null-cache");
    let query = Query::events().only_from_relays([url.clone()])?;
    let observation = observer.open(query.clone())?;
    wait_until(|| assembly.peer(&url).is_some()).await;
    let peer = assembly.established(&url);
    wait_until(|| requests(Some(peer.clone())).len() == 1).await;
    let wire = requests(Some(peer.clone()))[0].0.clone();
    let keys = Keys::generate();
    let old = EventBuilder::new(Kind::Metadata, "old")
        .custom_created_at(Timestamp::from(1))
        .finalize(&keys)?;
    let new = EventBuilder::new(Kind::Metadata, "new")
        .custom_created_at(Timestamp::from(2))
        .finalize(&keys)?;

    push(&peer, &RelayMessage::event(wire.clone(), old.clone()));
    wait_until(|| {
        observation
            .current()
            .events
            .iter()
            .any(|record| record.id() == old.id)
    })
    .await;
    push(&peer, &RelayMessage::event(wire.clone(), new.clone()));
    wait_until(|| {
        observation.current().events.len() == 1 && observation.current().events[0].id() == new.id
    })
    .await;

    let deletion = EventBuilder::new(Kind::EventDeletion, "")
        .tag(Tag::event(new.id))
        .custom_created_at(Timestamp::from(3))
        .finalize(&keys)?;
    push(&peer, &RelayMessage::event(wire, deletion.clone()));
    wait_until(|| {
        observation
            .current()
            .events
            .iter()
            .all(|record| record.id() != new.id)
    })
    .await;
    assert_eq!(null.len()?, 0);
    observation.close();

    let later = observer.open(query)?;
    assert!(later.current().events.is_empty());
    later.close();
    Ok(())
}
