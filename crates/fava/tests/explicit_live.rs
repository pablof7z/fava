//! Public-facade explicit live-query evidence over a scripted transport.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::{ClientMessage, RelayMessage, encode_client};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use tokio::sync::Notify;

#[derive(Default)]
struct Script {
    inbound: Mutex<VecDeque<Result<String, TransportError>>>,
    sent: Mutex<Vec<String>>,
    opens: AtomicUsize,
    notify: Notify,
}

impl Script {
    fn receive(&self, message: &RelayMessage<'_>) {
        self.inbound
            .lock()
            .expect("script lock")
            .push_back(Ok(serde_json::to_string(&message).expect("message encodes")));
        self.notify.notify_one();
    }

    fn sent(&self) -> Vec<String> {
        self.sent.lock().expect("script lock").clone()
    }

    fn fail(&self, error: TransportError) {
        self.inbound
            .lock()
            .expect("script lock")
            .push_back(Err(error));
        self.notify.notify_one();
    }
}

struct ScriptedTransport {
    script: Arc<Script>,
}

struct PendingTransport;

impl Transport for PendingTransport {
    fn open_session(
        &self,
        _key: RelaySessionKey,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Arc<dyn RelaySession>, TransportError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(std::future::pending())
    }
}

#[derive(Default)]
struct FirstOpenThenPendingTransport {
    calls: AtomicUsize,
    opened: Mutex<Vec<Arc<ScriptedSession>>>,
}

impl Transport for FirstOpenThenPendingTransport {
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
            if self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
                return std::future::pending().await;
            }
            let session = Arc::new(ScriptedSession {
                key,
                script: Arc::new(Script::default()),
                closed: std::sync::atomic::AtomicBool::new(false),
            });
            self.opened
                .lock()
                .expect("transport lock")
                .push(Arc::clone(&session));
            Ok(session as Arc<dyn RelaySession>)
        })
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
        let script = Arc::clone(&self.script);
        Box::pin(async move {
            script.opens.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(ScriptedSession {
                key,
                script,
                closed: std::sync::atomic::AtomicBool::new(false),
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct ScriptedSession {
    key: RelaySessionKey,
    script: Arc<Script>,
    closed: std::sync::atomic::AtomicBool,
}

impl RelaySession for ScriptedSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        7
    }

    fn send(
        &self,
        frame: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    reason: "closed".to_owned(),
                };
            }
            self.script.sent.lock().expect("script lock").push(frame);
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
                if let Some(message) = self.script.inbound.lock().expect("script lock").pop_front()
                {
                    return message;
                }
                self.script.notify.notified().await;
            }
        })
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
            self.script.notify.notify_waiters();
            Ok(())
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn relay_establishment_does_not_delay_the_coherent_local_observation() {
    let relay = RelayUrl::parse("wss://pending.example").expect("relay URL");
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(PendingTransport))
        .build()
        .expect("assembly is complete");

    let observation = tokio::time::timeout(
        Duration::from_millis(50),
        fava.observe(
            Query::events()
                .only_from_relays([relay])
                .expect("explicit relay is valid"),
        ),
    )
    .await
    .expect("local observation must not await relay establishment")
    .expect("local observation opens");

    assert!(observation.current().events.is_empty());
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn equivalent_observations_share_relay_work_until_the_last_handle_closes() {
    let relay = RelayUrl::parse("wss://shared.example").expect("relay URL");
    let script = Arc::new(Script::default());
    let fava = assembly(Arc::new(MemoryEventCache::default()), Arc::clone(&script));
    let query = Query::events()
        .only_from_relays([relay])
        .expect("explicit relay is valid");

    let first = fava
        .observe(query.clone())
        .await
        .expect("first query opens");
    let second = fava.observe(query).await.expect("second query opens");

    assert_eq!(script.opens.load(Ordering::SeqCst), 1);
    assert_eq!(script.sent().len(), 1);
    first.close();
    assert_eq!(script.sent().len(), 1);
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_observe_while_another_relay_opens_closes_provisional_work() {
    let transport = Arc::new(FirstOpenThenPendingTransport::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .build()
        .expect("assembly is complete");
    let query = Query::events()
        .only_from_relays([
            RelayUrl::parse("wss://a-open.example").expect("relay URL"),
            RelayUrl::parse("wss://b-pending.example").expect("relay URL"),
        ])
        .expect("explicit relays are valid");

    assert!(
        tokio::time::timeout(Duration::from_millis(50), fava.observe(query))
            .await
            .is_err()
    );
    assert_eq!(transport.calls.load(Ordering::SeqCst), 2);
    let first = transport
        .opened
        .lock()
        .expect("transport lock")
        .first()
        .cloned()
        .expect("first relay opened");
    assert!(first.closed.load(Ordering::SeqCst));
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_live_query_attributes_event_eose_and_exact_cancellation() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay URL");
    let script = Arc::new(Script::default());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), Arc::clone(&script));
    let keys = Keys::generate();
    let event = EventBuilder::new(Kind::TextNote, "stored")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .expect("event signs");
    let query = Query::events()
        .authors([keys.public_key()])
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid");

    let mut observation = fava.observe(query).await.expect("live query opens");
    let req = script.sent().first().cloned().expect("REQ handed off");
    let subscription =
        match serde_json::from_str::<ClientMessage<'static>>(&req).expect("REQ decodes") {
            ClientMessage::Req {
                subscription_id, ..
            } => subscription_id.into_owned(),
            other => panic!("expected REQ, got {other:?}"),
        };
    script.receive(&RelayMessage::event(subscription.clone(), event.clone()));
    script.receive(&RelayMessage::eose(subscription.clone()));

    let snapshot = tokio::time::timeout(Duration::from_secs(1), observation.changed())
        .await
        .expect("event deadline")
        .expect("query stays open");
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), event.id);
    assert_eq!(snapshot.events[0].relay_evidence.len(), 1);
    wait_until(Duration::from_secs(1), || {
        fava.diagnostics().eose.iter().any(|(key, generation, id)| {
            key.relay == relay && *generation == 7 && id == &subscription
        })
    })
    .await;

    let mut forged = event.clone();
    forged.content = "forged after signing".to_owned();
    let off_filter = EventBuilder::new(Kind::TextNote, "other author")
        .finalize(&Keys::generate())
        .expect("event signs");
    script.receive(&RelayMessage::event(subscription.clone(), forged));
    script.receive(&RelayMessage::event(subscription.clone(), off_filter));
    wait_until(Duration::from_secs(1), || {
        fava.diagnostics().failures.len() >= 2
    })
    .await;
    assert_eq!(cache.len().expect("cache readable"), 1);

    observation.close();
    observation.close();
    wait_until(Duration::from_secs(1), || {
        script.sent().iter().any(|frame| {
            frame
                == &encode_client(&ClientMessage::close(subscription.clone()))
                    .expect("CLOSE encodes")
        })
    })
    .await;
    assert!(observation.changed().await.is_err());
    assert_eq!(cache.len().expect("cache readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn silence_eose_auth_closed_and_disconnect_are_distinct_facts() {
    let relay = RelayUrl::parse("wss://relay.example").expect("relay URL");
    let script = Arc::new(Script::default());
    let fava = assembly(Arc::new(MemoryEventCache::default()), Arc::clone(&script));
    let observation = fava
        .observe(
            Query::events()
                .only_from_relays([relay])
                .expect("explicit relay is valid"),
        )
        .await
        .expect("live query opens");
    let subscription = fava
        .diagnostics()
        .subscriptions
        .first()
        .map(|(_, _, id)| id.clone())
        .expect("subscription is diagnosed");
    let silent = fava.diagnostics();
    assert!(silent.eose.is_empty());
    assert!(silent.closed.is_empty());
    assert!(silent.authentication_required.is_empty());
    assert!(silent.failures.is_empty());

    script.receive(&RelayMessage::eose(subscription.clone()));
    script.receive(&RelayMessage::auth("challenge"));
    script.receive(&RelayMessage::closed(subscription.clone(), "rate-limited"));
    script.fail(TransportError::Disconnected("injected".to_owned()));
    wait_until(Duration::from_secs(1), || {
        let facts = fava.diagnostics();
        facts.eose.len() == 1
            && facts.closed.len() == 1
            && facts.authentication_required.len() == 1
            && facts.failures.len() == 1
    })
    .await;
    let facts = fava.diagnostics();
    assert_eq!(facts.eose[0].2, subscription);
    assert_eq!(facts.closed[0].3, "rate-limited");
    assert_eq!(facts.failures[0].2, "relay session disconnected: injected");
    observation.close();
}

fn assembly(cache: Arc<MemoryEventCache>, script: Arc<Script>) -> Fava {
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(ScriptedTransport { script }))
        .build()
        .expect("assembly is complete")
}

async fn wait_until(deadline: Duration, predicate: impl Fn() -> bool) {
    tokio::time::timeout(deadline, async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}
