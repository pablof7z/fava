use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{EventValue, Fava, Receipt, RelayUrl};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_external_semantic_capability_proof::selected_applier;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_signer_local::LocalSigner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{
    BoundedText, Connection, Connectivity, HandoffCorrelation, HandoffFuture, HandoffOutcome,
    OpenRelaySession, RelayConnection, RelaySession, RelaySessionFuture, RelaySessionIdentity,
    ReleaseFuture, ReleaseOutcome, Router, SubscriptionId, Transport, TransportFailure,
    TransportShutdownFuture, publish_authentication_requests, subscription_id,
};
use fava_transport_testkit::detached_lease;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventId};
use nostr::key::Keys;
use nostr::message::RelayMessage;
use serde_json::Value;
use tokio::sync::broadcast;

/// Unheard authentication requests held before the oldest is dropped.
///
/// This proof never exercises NIP-42, so a small backlog is enough; it exists
/// only to satisfy [`Transport::authentication_requests`].
const REQUEST_BACKLOG: usize = 8;

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
        .appliers([selected_applier()])
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

struct Shared {
    state: Mutex<ScriptState>,
    changed: tokio::sync::Notify,
    opens: AtomicU64,
    /// Every session the transport currently holds, for [`Transport::sessions`].
    sessions: Mutex<Vec<Arc<dyn RelaySession>>>,
    /// Sessions whose relay has asked them to authenticate. Never published
    /// by this fake, which speaks no NIP-42.
    requests: broadcast::Sender<Arc<dyn RelaySession>>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            state: Mutex::default(),
            changed: tokio::sync::Notify::default(),
            opens: AtomicU64::default(),
            sessions: Mutex::default(),
            requests: broadcast::Sender::new(REQUEST_BACKLOG),
        }
    }
}

#[derive(Default)]
struct ScriptState {
    subscriptions: HashMap<String, Arc<Router>>,
    publications: Vec<Publication>,
    closed_publications: HashSet<EventId>,
}

#[derive(Clone)]
struct Publication {
    event: Event,
    router: Arc<Router>,
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

    /// Deliver one `EVENT` for `subscription` directly onto the session's
    /// router, exactly as a transport implementation would after decoding a
    /// frame off its own socket.
    pub fn deliver(&self, subscription: &str, event: &Event) {
        let router = self
            .shared
            .state
            .lock()
            .expect("script lock")
            .subscriptions
            .get(subscription)
            .cloned()
            .expect("subscription exists");
        router.deliver(RelayMessage::Event {
            subscription_id: std::borrow::Cow::Owned(SubscriptionId::new(subscription)),
            event: std::borrow::Cow::Owned(event.clone()),
        });
    }

    pub fn eose(&self, subscription: &str) {
        let router = self
            .shared
            .state
            .lock()
            .expect("script lock")
            .subscriptions
            .get(subscription)
            .cloned()
            .expect("subscription exists");
        router.deliver(RelayMessage::EndOfStoredEvents(std::borrow::Cow::Owned(
            SubscriptionId::new(subscription),
        )));
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
        publication.router.deliver(RelayMessage::Ok {
            event_id: id,
            status: true,
            message: std::borrow::Cow::Borrowed("stored"),
        });
        id
    }

    pub async fn wait_closed(&self, event_id: EventId) {
        self.wait_for("publication session close", |state| {
            state.closed_publications.contains(&event_id).then_some(())
        })
        .await;
    }

    async fn wait_for<T>(
        &self,
        label: &str,
        predicate: impl Fn(&ScriptState) -> Option<T>,
    ) -> T {
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
}

impl Transport for ScriptedTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let generation = self.shared.opens.fetch_add(1, Ordering::SeqCst) + 1;
        let owner = Arc::clone(&self.shared);
        let inbound_capacity = request.bounds.inbound_frames.get();
        Box::pin(async move {
            let identity = RelaySessionIdentity {
                relay: request.relay,
                connection: RelayConnection::new(generation)
                    .expect("scripted session connection is non-zero"),
            };
            let router = Arc::new(Router::new(Connection {
                connectivity: Connectivity::Connected,
                ..Connection::opening(identity.clone())
            }));
            let session: Arc<dyn RelaySession> = Arc::new(ScriptedSession {
                identity,
                owner: Arc::clone(&owner),
                router,
                inbound_capacity,
                subscription_counter: AtomicU64::new(0),
                closed: AtomicBool::new(false),
                publication: Mutex::new(None),
            });
            owner
                .sessions
                .lock()
                .expect("session registry is not poisoned")
                .push(Arc::clone(&session));
            let watched = Arc::downgrade(&session);
            let requests = owner.requests.clone();
            tokio::spawn(publish_authentication_requests(watched, requests));
            Ok(detached_lease(session))
        })
    }

    fn holders(&self, _relay: &RelayUrl, _authority: &Authority) -> Option<std::num::NonZeroUsize> {
        None
    }

    fn sessions(&self) -> Vec<Arc<dyn RelaySession>> {
        self.shared
            .sessions
            .lock()
            .expect("session registry is not poisoned")
            .clone()
    }

    fn authentication_requests(&self) -> broadcast::Receiver<Arc<dyn RelaySession>> {
        self.shared.requests.subscribe()
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct ScriptedSession {
    identity: RelaySessionIdentity,
    owner: Arc<Shared>,
    router: Arc<Router>,
    inbound_capacity: usize,
    /// Monotonic source of this session's wire subscription identifiers.
    subscription_counter: AtomicU64,
    closed: AtomicBool,
    publication: Mutex<Option<EventId>>,
}

impl ScriptedSession {
    /// Parse one outbound frame Fava handed off, and apply its effect to the
    /// shared script state -- exactly what a real socket write would trigger
    /// on the relay side, done in-process.
    fn sent(&self, correlation: HandoffCorrelation, frame: &str) -> HandoffOutcome {
        let refuse = |reason: &str| HandoffOutcome::NotHandedOff {
            identity: self.identity.clone(),
            correlation,
            reason: TransportFailure::Disconnected {
                detail: BoundedText::new(reason),
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
                self.owner
                    .state
                    .lock()
                    .expect("script lock")
                    .subscriptions
                    .insert(subscription.to_owned(), Arc::clone(&self.router));
                self.owner.changed.notify_waiters();
            }
            "EVENT" => {
                let event = value
                    .get(1)
                    .cloned()
                    .and_then(|value| serde_json::from_value::<Event>(value).ok());
                let Some(event) = event else {
                    return refuse("EVENT omitted a valid signed event");
                };
                *self.publication.lock().expect("publication lock") = Some(event.id);
                self.owner
                    .state
                    .lock()
                    .expect("script lock")
                    .publications
                    .push(Publication {
                        event,
                        router: Arc::clone(&self.router),
                    });
                self.owner.changed.notify_waiters();
            }
            "CLOSE" => {}
            _ => return refuse("script received an unsupported command"),
        }
        HandoffOutcome::HandedOff {
            identity: self.identity.clone(),
            correlation,
        }
    }
}

impl RelaySession for ScriptedSession {
    fn identity(&self) -> RelaySessionIdentity {
        self.identity.clone()
    }

    fn router(&self) -> &Router {
        &self.router
    }

    fn inbound_capacity(&self) -> usize {
        self.inbound_capacity
    }

    fn enqueue(&self, frame: Vec<u8>) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let text = String::from_utf8(frame).unwrap_or_default();
        let _ = self.sent(HandoffCorrelation::new(0), &text);
    }

    fn mint_subscription_id(&self) -> SubscriptionId {
        subscription_id(self.subscription_counter.fetch_add(1, Ordering::SeqCst))
    }

    fn hand_off(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    identity: self.identity.clone(),
                    correlation,
                    reason: TransportFailure::SessionClosed,
                };
            }
            let text = String::from_utf8(frame).unwrap_or_default();
            self.sent(correlation, &text)
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.closed.store(true, Ordering::SeqCst);
            self.router.close();
            self.owner
                .sessions
                .lock()
                .expect("session registry is not poisoned")
                .retain(|session| session.identity() != self.identity);
            if let Some(event_id) = *self.publication.lock().expect("publication lock") {
                self.owner
                    .state
                    .lock()
                    .expect("script lock")
                    .closed_publications
                    .insert(event_id);
            }
            self.owner.changed.notify_waiters();
            Ok(ReleaseOutcome::Closed)
        })
    }
}
