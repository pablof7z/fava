use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{EventValue, Fava, Receipt, RelayUrl};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_external_semantic_capability_proof::selected_materializer;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_relay::RelaySessionKey;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{
    BoundedReason, HandoffCorrelation, HandoffFuture, HandoffOutcome, OpenRelaySession,
    RelayInbound, RelayInboundFuture, RelayMessageStream, RelaySession, RelaySessionFuture,
    RelaySessionIdentity, ReleaseFuture, ReleaseOutcome, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_transport_testkit::detached_lease;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventId};
use nostr::key::Keys;
use serde_json::{Value, json};
use tokio::sync::Notify;

const DEADLINE: Duration = Duration::from_secs(2);

mod waits;

pub use waits::{
    open_external_source, open_observation, wait_eose, wait_first_record, wait_generation_record,
    wait_receipt, wait_terminal,
};

pub struct Harness {
    pub fava: Fava,
    pub transport: Arc<ScriptedTransport>,
    pub relay: RelayUrl,
}

pub fn harness(keys: Keys) -> Harness {
    let relay = RelayUrl::parse("wss://external-semantic.example").expect("relay URL");
    let transport = Arc::new(ScriptedTransport::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner))
        .transport(Arc::clone(&transport))
        .signer(Arc::new(LocalSigner::new(keys)))
        .application_materializers([selected_materializer()])
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("public external assembly builds");
    Harness {
        fava,
        transport,
        relay,
    }
}

pub fn signed(receipt: &Receipt) -> &Event {
    match &receipt.current.event {
        EventValue::Signed(event) => event,
        EventValue::Unsigned(_) => panic!("receipt event is not signed"),
    }
}

#[derive(Clone, Default)]
pub struct ScriptedTransport {
    shared: Arc<Shared>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<ScriptState>,
    changed: Notify,
    opens: AtomicU64,
}

#[derive(Default)]
struct ScriptState {
    subscriptions: HashMap<String, Arc<Inbox>>,
    publications: Vec<Publication>,
    closed_publications: HashSet<EventId>,
}

#[derive(Clone)]
struct Publication {
    event: Event,
    inbox: Arc<Inbox>,
}

struct Inbox {
    frames: Mutex<VecDeque<Result<String, TransportError>>>,
    notify: Notify,
    closed: AtomicBool,
    publication: Mutex<Option<EventId>>,
}

impl Inbox {
    fn new() -> Self {
        Self {
            frames: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            closed: AtomicBool::new(false),
            publication: Mutex::new(None),
        }
    }

    fn push(&self, frame: String) {
        self.frames.lock().expect("inbox lock").push_back(Ok(frame));
        self.notify.notify_one();
    }
}

impl ScriptedTransport {
    pub fn open_count(&self) -> u64 {
        self.shared.opens.load(Ordering::SeqCst)
    }

    pub fn publication_count(&self) -> usize {
        self.shared
            .state
            .lock()
            .expect("script lock")
            .publications
            .len()
    }

    pub async fn subscription(&self) -> String {
        self.wait_for("subscription open", |state| {
            state.subscriptions.keys().next().cloned()
        })
        .await
    }

    pub async fn published(&self, index: usize) -> Event {
        self.wait_for("publication handoff", |state| {
            state
                .publications
                .get(index)
                .map(|publication| publication.event.clone())
        })
        .await
    }

    pub fn deliver(&self, subscription: &str, event: &Event) {
        let inbox = self
            .shared
            .state
            .lock()
            .expect("script lock")
            .subscriptions
            .get(subscription)
            .cloned()
            .expect("subscription exists");
        inbox.push(json!(["EVENT", subscription, event]).to_string());
    }

    pub fn eose(&self, subscription: &str) {
        let inbox = self
            .shared
            .state
            .lock()
            .expect("script lock")
            .subscriptions
            .get(subscription)
            .cloned()
            .expect("subscription exists");
        inbox.push(json!(["EOSE", subscription]).to_string());
    }

    pub fn acknowledge(&self, index: usize) -> EventId {
        let publication = self
            .shared
            .state
            .lock()
            .expect("script lock")
            .publications
            .get(index)
            .cloned()
            .expect("publication exists");
        let id = publication.event.id;
        publication
            .inbox
            .push(json!(["OK", id, true, "stored"]).to_string());
        id
    }

    pub async fn wait_closed(&self, event_id: EventId) {
        self.wait_for("publication session close", |state| {
            state.closed_publications.contains(&event_id).then_some(())
        })
        .await;
    }

    async fn wait_for<T>(&self, label: &str, predicate: impl Fn(&ScriptState) -> Option<T>) -> T {
        waits::with_deadline(label, || self.shared.describe(), async {
            loop {
                let changed = self.shared.changed.notified();
                if let Some(value) = predicate(&self.shared.state.lock().expect("script lock")) {
                    return value;
                }
                changed.await;
            }
        })
        .await
    }
}

impl Shared {
    fn describe(&self) -> String {
        let state = self.state.lock().expect("script lock");
        format!(
            "opens={}, subscriptions={}, publications={}, closed_publications={}",
            self.opens.load(Ordering::SeqCst),
            state.subscriptions.len(),
            state.publications.len(),
            state.closed_publications.len()
        )
    }

    fn sent(
        &self,
        inbox: &Arc<Inbox>,
        identity: &RelaySessionIdentity,
        correlation: HandoffCorrelation,
        frame: &str,
    ) -> HandoffOutcome {
        let refuse = |reason: &str| HandoffOutcome::NotHandedOff {
            identity: identity.clone(),
            correlation,
            reason: TransportFailure::Disconnected {
                detail: BoundedReason::new(reason),
            },
        };
        let Ok(value) = serde_json::from_str::<Value>(frame) else {
            return refuse("script received invalid JSON");
        };
        let Some(command) = value.get(0).and_then(Value::as_str) else {
            return refuse("script received untyped frame");
        };
        match command {
            "REQ" => {
                let Some(subscription) = value.get(1).and_then(Value::as_str) else {
                    return refuse("REQ omitted subscription id");
                };
                self.state
                    .lock()
                    .expect("script lock")
                    .subscriptions
                    .insert(subscription.to_owned(), Arc::clone(inbox));
                self.changed.notify_waiters();
            }
            "EVENT" => {
                let event = value
                    .get(1)
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Event>(value).ok());
                let Some(event) = event else {
                    return refuse("EVENT omitted a valid signed event");
                };
                *inbox.publication.lock().expect("publication lock") = Some(event.id);
                self.state
                    .lock()
                    .expect("script lock")
                    .publications
                    .push(Publication {
                        event,
                        inbox: Arc::clone(inbox),
                    });
                self.changed.notify_waiters();
            }
            "CLOSE" => {}
            _ => return refuse("script received an unsupported command"),
        }
        HandoffOutcome::HandedOff {
            identity: identity.clone(),
            correlation,
        }
    }
}

impl Transport for ScriptedTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let generation = self.shared.opens.fetch_add(1, Ordering::SeqCst) + 1;
        let inbox = Arc::new(Inbox::new());
        let owner = Arc::clone(&self.shared);
        Box::pin(async move {
            let session: Arc<dyn RelaySession> = Arc::new(ScriptedSession {
                identity: RelaySessionIdentity {
                    key: request.key,
                    generation: fava_transport::RelaySessionGeneration::new(generation)
                        .expect("scripted session generation is non-zero"),
                },
                owner,
                inbox,
            });
            Ok(detached_lease(session))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<std::num::NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct ScriptedSession {
    identity: RelaySessionIdentity,
    owner: Arc<Shared>,
    inbox: Arc<Inbox>,
}

/// One consumer's view of a scripted session's inbound frames.
struct ScriptedStream {
    identity: RelaySessionIdentity,
    inbox: Arc<Inbox>,
}

impl RelayMessageStream for ScriptedStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(async move {
            loop {
                let notified = self.inbox.notify.notified();
                if let Some(frame) = self.inbox.frames.lock().expect("inbox lock").pop_front() {
                    return frame.map(|text| RelayInbound::Frame {
                        identity: self.identity.clone(),
                        frame: text.into_bytes(),
                        received_at: nostr::types::Timestamp::now(),
                    });
                }
                if self.inbox.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed(self.identity.clone()));
                }
                if tokio::time::timeout(DEADLINE, notified).await.is_err() {
                    let queued = self.inbox.frames.lock().expect("inbox lock").len();
                    let publication = *self.inbox.publication.lock().expect("publication lock");
                    return Err(TransportError::Disconnected(
                        TransportFailure::Disconnected {
                            detail: BoundedReason::new(format!(
                                "script inbound deadline exceeded {DEADLINE:?}; last state: queued={queued}, closed={}, publication={publication:?}",
                                self.inbox.closed.load(Ordering::SeqCst)
                            )),
                        },
                    ));
                }
            }
        })
    }

    fn close(&mut self) {}
}

impl RelaySession for ScriptedSession {
    fn identity(&self) -> RelaySessionIdentity {
        self.identity.clone()
    }

    fn send(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_> {
        Box::pin(async move {
            if self.inbox.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    identity: self.identity.clone(),
                    correlation,
                    reason: TransportFailure::SessionClosed,
                };
            }
            let text = String::from_utf8(frame).unwrap_or_default();
            self.owner
                .sent(&self.inbox, &self.identity, correlation, &text)
        })
    }

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        Box::new(ScriptedStream {
            identity: self.identity.clone(),
            inbox: Arc::clone(&self.inbox),
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.inbox.closed.store(true, Ordering::SeqCst);
            if let Some(event_id) = *self.inbox.publication.lock().expect("publication lock") {
                self.owner
                    .state
                    .lock()
                    .expect("script lock")
                    .closed_publications
                    .insert(event_id);
            }
            self.owner.changed.notify_waiters();
            self.inbox.notify.notify_waiters();
            Ok(ReleaseOutcome::Closed)
        })
    }
}
