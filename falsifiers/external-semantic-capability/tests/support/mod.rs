use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{EventValue, Fava, Receipt, RelayUrl, WriteIntent, WriteRouting};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_external_semantic_capability_proof::selected_materializer;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_state::RelaySessionKey;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
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
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::clone(&transport))
        .signer(Arc::new(LocalSigner::new(keys)))
        .materializers([selected_materializer()])
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

pub fn explicit_intent(
    edit: fava::ReplaceableEventEdit,
    author: fava::PublicKey,
    relay: &RelayUrl,
) -> WriteIntent {
    WriteIntent::edit_as(
        edit,
        author,
        WriteRouting::Explicit(BTreeSet::from([relay.clone()])),
    )
    .expect("external edit intent")
}

pub fn raw_intent(event: fava::UnsignedEvent, relay: &RelayUrl) -> WriteIntent {
    WriteIntent::event(
        event,
        WriteRouting::Explicit(BTreeSet::from([relay.clone()])),
    )
    .expect("raw future intent")
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

    fn sent(&self, inbox: &Arc<Inbox>, frame: &str) -> HandoffOutcome {
        let Ok(value) = serde_json::from_str::<Value>(frame) else {
            return HandoffOutcome::NotHandedOff {
                reason: "script received invalid JSON".to_owned(),
            };
        };
        let Some(command) = value.get(0).and_then(Value::as_str) else {
            return HandoffOutcome::NotHandedOff {
                reason: "script received untyped frame".to_owned(),
            };
        };
        match command {
            "REQ" => {
                let Some(subscription) = value.get(1).and_then(Value::as_str) else {
                    return HandoffOutcome::NotHandedOff {
                        reason: "REQ omitted subscription id".to_owned(),
                    };
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
                    return HandoffOutcome::NotHandedOff {
                        reason: "EVENT omitted a valid signed event".to_owned(),
                    };
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
            _ => {
                return HandoffOutcome::NotHandedOff {
                    reason: "script received an unsupported command".to_owned(),
                };
            }
        }
        HandoffOutcome::HandedOff
    }
}

impl Transport for ScriptedTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        let generation = self.shared.opens.fetch_add(1, Ordering::SeqCst) + 1;
        let inbox = Arc::new(Inbox::new());
        let owner = Arc::clone(&self.shared);
        Box::pin(async move {
            Ok(Arc::new(ScriptedSession {
                key,
                generation,
                owner,
                inbox,
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct ScriptedSession {
    key: RelaySessionKey,
    generation: u64,
    owner: Arc<Shared>,
    inbox: Arc<Inbox>,
}

impl RelaySession for ScriptedSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.inbox.closed.load(Ordering::SeqCst) {
                HandoffOutcome::NotHandedOff {
                    reason: "scripted session is closed".to_owned(),
                }
            } else {
                self.owner.sent(&self.inbox, &frame)
            }
        })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            loop {
                let notified = self.inbox.notify.notified();
                if let Some(frame) = self.inbox.frames.lock().expect("inbox lock").pop_front() {
                    return frame;
                }
                if self.inbox.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed);
                }
                if tokio::time::timeout(DEADLINE, notified).await.is_err() {
                    let queued = self.inbox.frames.lock().expect("inbox lock").len();
                    let publication = *self.inbox.publication.lock().expect("publication lock");
                    return Err(TransportError::Disconnected(format!(
                        "script inbound deadline exceeded {DEADLINE:?}; last state: queued={queued}, closed={}, publication={publication:?}",
                        self.inbox.closed.load(Ordering::SeqCst)
                    )));
                }
            }
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
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
            Ok(())
        })
    }
}
