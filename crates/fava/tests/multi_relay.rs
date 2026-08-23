//! Multi-relay provenance and reconnect-generation evidence through the public facade.

use std::num::NonZeroUsize;
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
use fava_transport::{
    BoundedReason, HandoffCorrelation, HandoffOutcome, OpenRelaySession, OperationGeneration,
    RelayInbound, RelayInboundFuture, RelayMessageStream, RelaySession, RelaySessionFuture,
    RelaySessionIdentity, ReleaseFuture, ReleaseOutcome, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_transport_testkit::detached_lease;
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
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        Box::pin(async move {
            let generation = self.next_generation.fetch_add(1, Ordering::SeqCst) + 1;
            let relay = request.key.relay.clone();
            let session = Arc::new(ScriptedSession {
                identity: RelaySessionIdentity {
                    key: request.key,
                    generation: OperationGeneration(generation),
                },
                mailbox: Arc::new(Mailbox::default()),
                sent: Mutex::new(Vec::new()),
            });
            self.sessions
                .lock()
                .expect("transport lock")
                .entry(relay)
                .or_default()
                .push(Arc::clone(&session));
            self.changed.notify_waiters();
            Ok(detached_lease(session as Arc<dyn RelaySession>))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct Mailbox {
    inbound: Mutex<VecDeque<Result<RelayInbound, TransportError>>>,
    changed: Notify,
    closed: AtomicBool,
}

struct ScriptedStream {
    mailbox: Arc<Mailbox>,
    identity: RelaySessionIdentity,
}

impl RelayMessageStream for ScriptedStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(async move {
            loop {
                if let Some(item) = self.mailbox.inbound.lock().expect("session lock").pop_front() {
                    return item;
                }
                if self.mailbox.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed(self.identity.clone()));
                }
                self.mailbox.changed.notified().await;
            }
        })
    }

    fn close(&mut self) {}
}

struct ScriptedSession {
    identity: RelaySessionIdentity,
    mailbox: Arc<Mailbox>,
    sent: Mutex<Vec<String>>,
}

impl ScriptedSession {
    fn generation(&self) -> u64 {
        self.identity.generation.0
    }

    fn receive(&self, message: &RelayMessage<'_>) {
        let frame = serde_json::to_string(message).expect("message encodes");
        self.mailbox
            .inbound
            .lock()
            .expect("session lock")
            .push_back(Ok(RelayInbound::Frame {
                identity: self.identity.clone(),
                frame: frame.into_bytes(),
                received_at: fava_state::Timestamp::now(),
            }));
        self.mailbox.changed.notify_one();
    }

    fn disconnect(&self) {
        self.mailbox
            .inbound
            .lock()
            .expect("session lock")
            .push_back(Ok(RelayInbound::Disconnected {
                identity: self.identity.clone(),
                reason: TransportFailure::Disconnected {
                    detail: BoundedReason::new("injected"),
                },
            }));
        self.mailbox.changed.notify_one();
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
    fn identity(&self) -> RelaySessionIdentity {
        self.identity.clone()
    }

    fn send(
        &self,
        frame: Vec<u8>,
        correlation: HandoffCorrelation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.mailbox.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    identity: self.identity.clone(),
                    correlation,
                    reason: TransportFailure::SessionClosed,
                };
            }
            self.sent
                .lock()
                .expect("session lock")
                .push(String::from_utf8_lossy(&frame).into_owned());
            HandoffOutcome::HandedOff {
                identity: self.identity.clone(),
                correlation,
            }
        })
    }

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        Box::new(ScriptedStream {
            mailbox: Arc::clone(&self.mailbox),
            identity: self.identity.clone(),
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.mailbox.closed.store(true, Ordering::SeqCst);
            self.mailbox.changed.notify_waiters();
            Ok(ReleaseOutcome::Closed)
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
    old.disconnect();

    let current = transport.session(&relay, 1).await;
    let current_subscription = current.subscription();
    assert!(current.generation() > old.generation());
    assert_ne!(current_subscription, old_subscription);

    let event = EventBuilder::new(Kind::TextNote, "current generation")
        .finalize(&Keys::generate())
        .expect("event signs");
    current.receive(&RelayMessage::event(
        old_subscription.clone(),
        event.clone(),
    ));
    settle().await;
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
    let winner_a = addressable_event(&keys, 30, "winner A", "same");
    let winner_b = addressable_event(&keys, 20, "winner B", "same");
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

async fn settle() {
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
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
