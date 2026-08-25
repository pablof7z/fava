//! NIP-65 outbox routing behavior over an ordinary exact query source.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use fava_query::{
    BoundedText, OpenedQuerySource, Query, QueryAcquisition, QuerySource, QuerySourceClosed,
    QuerySourceError, SourceChangeFuture, SourceChanges, SourceEvent, SourceKind, SourceRevision,
    SourceSnapshot, SourceStatus, SourceTerminationCause,
};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_outbox::OutboxRouter;
use fava_routing::{CoverageState, RoutePlan, RouteRequest, RouteTarget, Router};
use fava_state::RelayEvent;
use fava_write::{EventBuilder, EventValue, Kind, Tag, Timestamp};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn current_query_values_route_and_missing_list_uses_exact_indexer_query() {
    let author = Keys::generate();
    let recipient_a = Keys::generate();
    let recipient_b = Keys::generate();
    let author_relay = relay("author-write");
    let first_read_relay = relay("recipient-a-read");
    let later_read_relay = relay("recipient-b-read");
    let indexer = relay("indexer");
    let source = Arc::new(WatchSource::new());
    let author_list = relay_list(&author, None, Some(&author_relay), 1);
    let recipient_a_list = relay_list(&recipient_a, Some(&first_read_relay), None, 1);
    source.replace_all(vec![author_list.clone(), recipient_a_list.clone()]);
    let router = OutboxRouter::new("nip65", [indexer.clone()], source.clone()).unwrap();
    let event = EventBuilder::new(author.public_key(), Kind::TextNote)
        .tag(p_tag(&recipient_a))
        .tag(p_tag(&recipient_b))
        .build()
        .unwrap();
    let request = RouteRequest::Write(EventValue::Unsigned(event));
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
        Some(BTreeSet::from([
            author.public_key(),
            recipient_a.public_key(),
            recipient_b.public_key(),
        ]))
    );
    assert_eq!(
        query.source().acquisition(),
        &QueryAcquisition::Explicit(BTreeSet::from([indexer]))
    );

    source.replace_all(vec![
        author_list,
        recipient_a_list,
        relay_list(&recipient_b, Some(&later_read_relay), None, 2),
    ]);
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

    fn replace_all(&self, events: Vec<fava_write::Event>) {
        let revision = self.latest.borrow().revision.0.saturating_add(1);
        self.latest.send_replace(Arc::new(SourceSnapshot {
            kind: SourceKind::EventCache,
            revision: SourceRevision(revision),
            status: SourceStatus::Open,
            retractions: Vec::new(),
            events: events.into_iter().map(source_event).collect(),
        }));
    }

    fn replace_malformed(&self, count: usize) {
        let keys = Keys::generate();
        self.replace_all(
            (0..count)
                .map(|index| {
                    EventBuilder::new(keys.public_key(), Kind::TextNote)
                        .created_at(Timestamp::from(index as u64 + 1))
                        .build()
                        .unwrap()
                        .finalize(&keys)
                        .unwrap()
                })
                .collect(),
        );
    }

    fn settle(&self) {
        let revision = self.latest.borrow().revision.0.saturating_add(1);
        self.latest.send_replace(Arc::new(SourceSnapshot {
            kind: SourceKind::EventCache,
            revision: SourceRevision(revision),
            status: SourceStatus::Closed {
                cause: SourceTerminationCause::ProviderClosed,
            },
            retractions: Vec::new(),
            events: Vec::new(),
        }));
    }
}

fn source_event(event: fava_write::Event) -> SourceEvent {
    let observed_at = event.created_at;
    SourceEvent::Relay(RelayEvent::new(
        event,
        RelaySessionKey {
            relay: relay("source"),
            access: RelayAccess::Public,
        },
        observed_at,
    ))
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
            if self.closed {
                return Err(QuerySourceClosed::local_close());
            }
            if self.receiver.changed().await.is_err() {
                return Err(QuerySourceClosed::provider_closed());
            }
            Ok(self.receiver.borrow_and_update().as_ref().clone())
        })
    }

    fn close(&mut self) {
        self.closed = true;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn failed_discovery_source_stays_unresolved_and_never_becomes_settled_absent() {
    let author = Keys::generate();
    let indexer = relay("indexer");
    let source = Arc::new(ClosingSource(SourceTerminationCause::ProviderFailed {
        detail: BoundedText::new("indexer socket died"),
    }));
    let router = OutboxRouter::new("nip65", [indexer], source).unwrap();
    let request = RouteRequest::Read(Query::events().authors([author.public_key()]));
    let (_, upstream) = watch::channel(Arc::new(RoutePlan::default()));
    let mut session = router.open(request, upstream).expect("router opens");

    let changed = session
        .next_change()
        .await
        .expect("source close is reported");
    let plan = RoutePlan::from_contribution(2, &changed).unwrap();

    assert_eq!(
        plan.coverage.get(&RouteTarget::Author(author.public_key())),
        Some(&CoverageState::Unresolved),
        "a closed discovery source is not settled absence"
    );
    assert!(!plan.settled, "unanswered discovery must not settle");
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("indexer socket died")),
        "the exact provider failure must survive as a shortfall: {:?}",
        plan.shortfalls
    );
}

#[tokio::test(flavor = "current_thread")]
async fn discovery_source_completing_without_a_relay_list_settles_absence() {
    let author = Keys::generate();
    let indexer = relay("indexer");
    let source = Arc::new(WatchSource::new());
    let router = OutboxRouter::new("nip65", [indexer], source.clone()).unwrap();
    let request = RouteRequest::Read(Query::events().authors([author.public_key()]));
    let (_, upstream) = watch::channel(Arc::new(RoutePlan::default()));
    let mut session = router.open(request, upstream).expect("router opens");

    source.settle();
    let changed = session.next_change().await.expect("settlement is reported");
    let plan = RoutePlan::from_contribution(2, &changed).unwrap();

    assert_eq!(
        plan.coverage.get(&RouteTarget::Author(author.public_key())),
        Some(&CoverageState::SettledAbsent),
        "a source that completes without a relay list settles absence"
    );
    assert!(plan.settled);
}

#[tokio::test(flavor = "current_thread")]
async fn discarded_relay_list_failures_are_reported_as_an_exact_overflow_shortfall() {
    let author = Keys::generate();
    let indexer = relay("indexer");
    let source = Arc::new(WatchSource::new());
    let router = OutboxRouter::new("nip65", [indexer], source.clone()).unwrap();
    let request = RouteRequest::Read(Query::events().authors([author.public_key()]));
    let (_, upstream) = watch::channel(Arc::new(RoutePlan::default()));
    let mut session = router.open(request, upstream).expect("router opens");

    source.replace_malformed(300);
    let changed = session
        .next_change()
        .await
        .expect("malformed batch reported");

    assert!(
        changed.shortfalls.len() <= 256,
        "shortfalls must stay bounded: {}",
        changed.shortfalls.len()
    );
    assert!(
        changed
            .shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("discarded")),
        "dropped input must never be silent: {}",
        changed.shortfalls.len()
    );
}

/// A discovery source whose observation ends through the error channel with an
/// exact cause. The error channel is the only termination path production
/// providers actually take, so it is the path that must carry the cause.
struct ClosingSource(SourceTerminationCause);

impl QuerySource for ClosingSource {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        Ok(OpenedQuerySource {
            initial: SourceSnapshot::empty(SourceKind::EventCache),
            changes: Box::new(ClosedChanges(self.0.clone())),
        })
    }
}

struct ClosedChanges(SourceTerminationCause);

impl SourceChanges for ClosedChanges {
    fn next_change(&mut self) -> SourceChangeFuture<'_> {
        let cause = self.0.clone();
        Box::pin(async move { Err(QuerySourceClosed::new(cause)) })
    }

    fn close(&mut self) {}
}

#[tokio::test(flavor = "current_thread")]
async fn a_cleanly_closed_discovery_source_settles_absence_through_the_error_channel() {
    let author = Keys::generate();
    let indexer = relay("indexer");
    let source = Arc::new(ClosingSource(SourceTerminationCause::ProviderClosed));
    let router = OutboxRouter::new("nip65", [indexer], source).unwrap();
    let request = RouteRequest::Read(Query::events().authors([author.public_key()]));
    let (_, upstream) = watch::channel(Arc::new(RoutePlan::default()));
    let mut session = router.open(request, upstream).expect("router opens");

    let changed = session
        .next_change()
        .await
        .expect("source termination is reported");
    let plan = RoutePlan::from_contribution(2, &changed).unwrap();

    assert_eq!(
        plan.coverage.get(&RouteTarget::Author(author.public_key())),
        Some(&CoverageState::SettledAbsent),
        "a provider that closed cleanly proves the relay list is absent"
    );
    assert!(plan.settled, "settled absence terminates the route");
}
