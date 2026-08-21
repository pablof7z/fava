//! NIP-65 outbox routing behavior over an ordinary exact query source.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use fava_query::{
    OpenedQuerySource, Query, QueryAcquisition, QuerySource, QuerySourceClosed, QuerySourceError,
    SourceChangeFuture, SourceChanges, SourceEvent, SourceKind, SourceRevision, SourceSnapshot,
    SourceStatus,
};
use fava_router_outbox::OutboxRouter;
use fava_routing::{RoutePlan, RouteRequest, RouteTarget, Router};
use fava_state::{CachedEvent, RelayAccess, RelayEvidence, RelayUrl};
use fava_write::{EventBuilder, EventValue, Kind, Tag, Timestamp};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn known_lists_are_immediate_and_missing_list_uses_exact_indexer_query() {
    let author = Keys::generate();
    let recipient_a = Keys::generate();
    let recipient_b = Keys::generate();
    let author_relay = relay("author-write");
    let first_read_relay = relay("recipient-a-read");
    let later_read_relay = relay("recipient-b-read");
    let indexer = relay("indexer");
    let source = Arc::new(WatchSource::new());
    let router = OutboxRouter::new("nip65", [indexer.clone()], source.clone()).unwrap();
    router
        .remember(&EventValue::Signed(relay_list(
            &author,
            None,
            Some(&author_relay),
            1,
        )))
        .unwrap();
    router
        .remember(&EventValue::Signed(relay_list(
            &recipient_a,
            Some(&first_read_relay),
            None,
            1,
        )))
        .unwrap();
    let event = EventBuilder::new(author.public_key(), Kind::TextNote)
        .tag(p_tag(&recipient_a))
        .tag(p_tag(&recipient_b))
        .build()
        .unwrap();
    let request = RouteRequest::write(EventValue::Unsigned(event), RelayAccess::public());
    let (_, upstream) = watch::channel(Arc::new(RoutePlan::default()));
    let mut session = router.open(request, upstream).expect("router opens");

    let initial = RoutePlan::from_contribution(1, &session.current()).unwrap();
    assert_eq!(initial.destinations.len(), 2);
    assert!(!initial.settled);
    assert_eq!(
        initial.unresolved,
        BTreeSet::from([RouteTarget::Recipient(recipient_b.public_key())])
    );
    let query = source.query().expect("indexer query recorded");
    assert_eq!(
        query.selection().kinds,
        Some(BTreeSet::from([Kind::from(10_002_u16)]))
    );
    assert_eq!(
        query.selection().authors,
        Some(BTreeSet::from([recipient_b.public_key()]))
    );
    assert_eq!(
        query.source().acquisition(),
        &QueryAcquisition::Explicit(BTreeSet::from([indexer]))
    );

    source.replace(relay_list(&recipient_b, Some(&later_read_relay), None, 2));
    let changed = session.next_change().await.expect("later relay list");
    let final_plan = RoutePlan::from_contribution(2, &changed).unwrap();
    assert!(final_plan.settled);
    assert!(final_plan.unresolved.is_empty());
    assert_eq!(final_plan.destinations.len(), 3);
}

fn relay_list(
    keys: &Keys,
    read: Option<&RelayUrl>,
    write: Option<&RelayUrl>,
    timestamp: u64,
) -> fava_write::Event {
    let mut builder = EventBuilder::new(keys.public_key(), Kind::from(10_002_u16))
        .created_at(Timestamp::from(timestamp));
    if let Some(relay) = read {
        builder = builder.tag(Tag::parse(["r", relay.as_str(), "read"]).unwrap());
    }
    if let Some(relay) = write {
        builder = builder.tag(Tag::parse(["r", relay.as_str(), "write"]).unwrap());
    }
    builder.build().unwrap().finalize(keys).unwrap()
}

fn p_tag(keys: &Keys) -> Tag {
    Tag::parse(["p", &keys.public_key().to_hex()]).unwrap()
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).unwrap()
}

struct WatchSource {
    latest: watch::Sender<Arc<SourceSnapshot>>,
    query: Mutex<Option<Query>>,
}

impl WatchSource {
    fn new() -> Self {
        let (latest, _) = watch::channel(Arc::new(SourceSnapshot::empty(SourceKind::EventCache)));
        Self {
            latest,
            query: Mutex::new(None),
        }
    }

    fn query(&self) -> Option<Query> {
        self.query.lock().expect("query lock").clone()
    }

    fn replace(&self, event: fava_write::Event) {
        let revision = self.latest.borrow().revision.0.saturating_add(1);
        self.latest.send_replace(Arc::new(SourceSnapshot {
            kind: SourceKind::EventCache,
            revision: SourceRevision(revision),
            status: SourceStatus::Open,
            events: vec![SourceEvent::Cached(CachedEvent::new(
                event,
                RelayEvidence::default(),
            ))],
        }));
    }
}

impl QuerySource for WatchSource {
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        *self.query.lock().expect("query lock") = Some(query.clone());
        let receiver = self.latest.subscribe();
        let initial = receiver.borrow().as_ref().clone();
        Ok(OpenedQuerySource {
            initial,
            changes: Box::new(WatchChanges {
                receiver,
                closed: false,
            }),
        })
    }
}

struct WatchChanges {
    receiver: watch::Receiver<Arc<SourceSnapshot>>,
    closed: bool,
}

impl SourceChanges for WatchChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        Box::pin(async move {
            if self.closed || self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed);
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}
