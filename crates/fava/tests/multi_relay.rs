//! Multi-relay provenance and reconnect-generation evidence through the public facade.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, encode_client};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::key::Keys;
use tokio::sync::Notify;

#[derive(Default)]
struct ScriptedTransport {
    next_generation: AtomicU64,
    sessions: Mutex<BTreeMap<RelayUrl, Vec<Arc<ScriptedSession>>>>,
    changed: Notify,
}

impl ScriptedTransport {
    async fn session(&self, relay: &RelayUrl, index: usize) -> Arc<ScriptedSession> {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(session) = self
                    .sessions
                    .lock()
                    .expect("transport lock")
                    .get(relay)
                    .and_then(|sessions| sessions.get(index))
                    .cloned()
                {
                    return session;
                }
                self.changed.notified().await;
            }
        })
        .await
        .expect("session open deadline")
    }
}

impl Transport for ScriptedTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Arc<dyn RelaySession>, TransportError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let generation = self.next_generation.fetch_add(1, Ordering::SeqCst) + 1;
            let relay = key.relay.clone();
            let session = Arc::new(ScriptedSession {
                key,
                generation,
                inbound: Mutex::new(VecDeque::new()),
                sent: Mutex::new(Vec::new()),
                changed: Notify::new(),
                closed: AtomicBool::new(false),
            });
            self.sessions
                .lock()
                .expect("transport lock")
                .entry(relay)
                .or_default()
                .push(Arc::clone(&session));
            self.changed.notify_waiters();
            Ok(session as Arc<dyn RelaySession>)
        })
    }
}

struct ScriptedSession {
    key: RelaySessionKey,
    generation: u64,
    inbound: Mutex<VecDeque<Result<String, TransportError>>>,
    sent: Mutex<Vec<String>>,
    changed: Notify,
    closed: AtomicBool,
}

impl ScriptedSession {
    fn receive(&self, message: &RelayMessage<'_>) {
        self.inbound
            .lock()
            .expect("session lock")
            .push_back(Ok(serde_json::to_string(message).expect("message encodes")));
        self.changed.notify_one();
    }

    fn disconnect(&self) {
        self.inbound
            .lock()
            .expect("session lock")
            .push_back(Err(TransportError::Disconnected("injected".to_owned())));
        self.changed.notify_one();
    }

    fn subscription(&self) -> SubscriptionId {
        self.sent
            .lock()
            .expect("session lock")
            .iter()
            .find_map(|frame| {
                match serde_json::from_str::<ClientMessage<'static>>(frame)
                    .expect("client message decodes")
                {
                    ClientMessage::Req {
                        subscription_id, ..
                    } => Some(subscription_id.into_owned()),
                    _ => None,
                }
            })
            .expect("REQ was handed off")
    }
}

impl RelaySession for ScriptedSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(
        &self,
        frame: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    reason: "closed".to_owned(),
                };
            }
            self.sent.lock().expect("session lock").push(frame);
            HandoffOutcome::HandedOff
        })
    }

    fn next_message(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, TransportError>> + Send + '_>,
    > {
        Box::pin(async move {
            loop {
                if let Some(message) = self.inbound.lock().expect("session lock").pop_front() {
                    return message;
                }
                if self.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed);
                }
                self.changed.notified().await;
            }
        })
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            self.closed.store(true, Ordering::SeqCst);
            self.changed.notify_waiters();
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn duplicate_event_merges_only_actual_serving_relays() {
    let relays = relay_urls(3);
    let transport = Arc::new(ScriptedTransport::default());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), Arc::clone(&transport));
    let keys = Keys::generate();
    let event = EventBuilder::new(Kind::TextNote, "same event")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .expect("event signs");
    let mut observation = fava
        .observe(
            Query::events()
                .only_from_relays(relays.clone())
                .expect("explicit relays are valid"),
        )
        .await
        .expect("query opens");
    let first = transport.session(&relays[0], 0).await;
    let second = transport.session(&relays[1], 0).await;
    let third = transport.session(&relays[2], 0).await;

    first.receive(&RelayMessage::event(first.subscription(), event.clone()));
    second.receive(&RelayMessage::event(second.subscription(), event.clone()));
    third.receive(&RelayMessage::eose(third.subscription()));

    let latest = wait_for_snapshot(&mut observation, |snapshot| {
        snapshot
            .events
            .first()
            .is_some_and(|record| record.relay_evidence.len() == 2)
    })
    .await;
    assert_eq!(latest.events.len(), 1);
    let serving: Vec<_> = latest.events[0]
        .relay_evidence
        .observations()
        .map(|evidence| evidence.session.relay.clone())
        .collect();
    assert!(serving.contains(&relays[0]));
    assert!(serving.contains(&relays[1]));
    assert!(!serving.contains(&relays[2]));
    assert_eq!(cache.len().expect("cache readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn reconnect_uses_fresh_identity_and_rejects_old_subscription_frames() {
    let relay = relay_urls(1).remove(0);
    let transport = Arc::new(ScriptedTransport::default());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), Arc::clone(&transport));
    let mut observation = fava
        .observe(
            Query::events()
                .only_from_relays([relay.clone()])
                .expect("explicit relay is valid"),
        )
        .await
        .expect("query opens");
    let old = transport.session(&relay, 0).await;
    let old_subscription = old.subscription();
    old.receive(&RelayMessage::eose(old_subscription.clone()));
    wait_until(|| fava.diagnostics().eose.len() == 1).await;
    old.disconnect();

    let current = transport.session(&relay, 1).await;
    let current_subscription = current.subscription();
    assert!(current.generation() > old.generation());
    assert_ne!(current_subscription, old_subscription);
    assert_eq!(fava.diagnostics().eose.len(), 1);

    let event = EventBuilder::new(Kind::TextNote, "current generation")
        .finalize(&Keys::generate())
        .expect("event signs");
    current.receive(&RelayMessage::event(
        old_subscription.clone(),
        event.clone(),
    ));
    wait_until(|| {
        fava.diagnostics()
            .failures
            .iter()
            .any(|(_, generation, message)| {
                *generation == current.generation()
                    && message == &format!("unattributed EVENT for {old_subscription}")
            })
    })
    .await;
    assert_eq!(cache.len().expect("cache readable"), 0);

    current.receive(&RelayMessage::event(current_subscription, event.clone()));
    let latest = wait_for_snapshot(&mut observation, |snapshot| !snapshot.events.is_empty()).await;
    assert_eq!(latest.events[0].id(), event.id);
    assert_eq!(cache.len().expect("cache readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn multi_relay_replaceable_authority_survives_public_facade() {
    let relays = relay_urls(2);
    let transport = Arc::new(ScriptedTransport::default());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), Arc::clone(&transport));
    let keys = Keys::generate();
    let kind = Kind::from_u16(30_001);
    let winner_a = addressable_event(&keys, 20, "winner A", "same");
    let winner_b = addressable_event(&keys, 30, "winner B", "same");
    let shared = addressable_event(&keys, 10, "shared", "shared");
    let mut observation = fava
        .observe(
            Query::events()
                .kind(kind)
                .only_from_relays(relays.clone())
                .expect("explicit relays are valid"),
        )
        .await
        .expect("query opens");
    let first = transport.session(&relays[0], 0).await;
    let second = transport.session(&relays[1], 0).await;
    let first_subscription = first.subscription();
    let second_subscription = second.subscription();

    first.receive(&RelayMessage::event(
        first_subscription.clone(),
        winner_a.clone(),
    ));
    second.receive(&RelayMessage::event(
        second_subscription.clone(),
        winner_b.clone(),
    ));
    first.receive(&RelayMessage::event(
        first_subscription.clone(),
        shared.clone(),
    ));
    second.receive(&RelayMessage::event(
        second_subscription.clone(),
        shared.clone(),
    ));

    let latest = wait_for_snapshot(&mut observation, |snapshot| {
        snapshot.events.len() >= 2
            && snapshot
                .events
                .iter()
                .any(|record| record.id() == shared.id && record.relay_evidence.len() == 2)
    })
    .await;
    assert!(
        latest
            .events
            .iter()
            .any(|record| record.id() == winner_a.id)
    );
    assert!(
        latest
            .events
            .iter()
            .any(|record| record.id() == winner_b.id)
    );
    let shared_record = latest
        .events
        .iter()
        .find(|record| record.id() == shared.id)
        .expect("shared event remains visible once");
    assert_eq!(shared_record.relay_evidence.len(), 2);
    assert_eq!(
        latest
            .events
            .iter()
            .filter(|record| record.id() == shared.id)
            .count(),
        1
    );
    assert_eq!(cache.len().expect("cache readable"), 3);

    observation.close();
    observation.close();
    wait_until(|| {
        let first_close = encode_client(&ClientMessage::close(first_subscription.clone()))
            .expect("CLOSE encodes");
        let second_close = encode_client(&ClientMessage::close(second_subscription.clone()))
            .expect("CLOSE encodes");
        first
            .sent
            .lock()
            .expect("session lock")
            .contains(&first_close)
            && second
                .sent
                .lock()
                .expect("session lock")
                .contains(&second_close)
    })
    .await;
    assert!(observation.changed().await.is_err());
}

fn assembly(cache: Arc<MemoryEventCache>, transport: Arc<ScriptedTransport>) -> Fava {
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(transport)
        .build()
        .expect("assembly is complete")
}

fn relay_urls(count: usize) -> Vec<RelayUrl> {
    (0..count)
        .map(|index| {
            let url = format!("wss://relay-{index}.example");
            RelayUrl::parse(&url).expect("relay URL parses")
        })
        .collect()
}

fn addressable_event(keys: &Keys, created_at: u64, content: &str, identifier: &str) -> Event {
    EventBuilder::new(Kind::from_u16(30_001), content)
        .tags([Tag::identifier(identifier)])
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("event signs")
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}

async fn wait_for_snapshot(
    observation: &mut fava::Observation,
    predicate: impl Fn(&fava::QuerySnapshot) -> bool,
) -> Arc<fava::QuerySnapshot> {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return current;
            }
            observation.changed().await.expect("query stays open");
        }
    })
    .await
    .expect("snapshot deadline elapsed")
}
