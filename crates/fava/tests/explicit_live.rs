//! Public-facade explicit live-query evidence over a scripted transport.

use std::collections::VecDeque;
use std::num::NonZeroUsize;
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
use fava_wire::{ClientMessage, RelayMessage, encode_client};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use tokio::sync::Notify;

#[derive(Default)]
struct Script {
    inbound: Mutex<VecDeque<Result<RelayInbound, TransportError>>>,
    sent: Mutex<Vec<String>>,
    notify: Notify,
}

impl Script {
    fn receive(&self, message: &RelayMessage<'_>) {
        let frame = serde_json::to_string(&message).expect("message encodes");
        self.inbound
            .lock()
            .expect("script lock")
            .push_back(Ok(RelayInbound::Frame {
                identity: scripted_identity(),
                frame: frame.into_bytes(),
                received_at: Timestamp::now(),
            }));
        self.notify.notify_one();
    }

    fn sent(&self) -> Vec<String> {
        self.sent.lock().expect("script lock").clone()
    }

    fn disconnect(&self, detail: &str) {
        self.inbound
            .lock()
            .expect("script lock")
            .push_back(Ok(RelayInbound::Disconnected {
                identity: scripted_identity(),
                reason: TransportFailure::Disconnected {
                    detail: BoundedReason::new(detail),
                },
            }));
        self.notify.notify_one();
    }
}

/// The scripted relay wears one fixed generation for the whole test.
fn scripted_identity() -> RelaySessionIdentity {
    RelaySessionIdentity {
        key: RelaySessionKey::new(
            RelayUrl::parse("wss://relay.example").expect("relay URL"),
            fava_state::RelayAccess::public(),
        ),
        generation: OperationGeneration(7),
    }
}

struct ScriptedTransport {
    script: Arc<Script>,
}

impl Transport for ScriptedTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let script = Arc::clone(&self.script);
        Box::pin(async move {
            let session: Arc<dyn RelaySession> = Arc::new(ScriptedSession {
                identity: RelaySessionIdentity {
                    key: request.key,
                    generation: OperationGeneration(7),
                },
                script,
                closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
            Ok(detached_lease(session))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct ScriptedSession {
    identity: RelaySessionIdentity,
    script: Arc<Script>,
    closed: Arc<std::sync::atomic::AtomicBool>,
}

struct ScriptedStream {
    identity: RelaySessionIdentity,
    script: Arc<Script>,
    closed: Arc<std::sync::atomic::AtomicBool>,
}

impl RelayMessageStream for ScriptedStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(async move {
            loop {
                if let Some(item) = self.script.inbound.lock().expect("script lock").pop_front() {
                    return item;
                }
                if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err(TransportError::Closed(self.identity.clone()));
                }
                self.script.notify.notified().await;
            }
        })
    }

    fn close(&mut self) {}
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
            if self.closed.load(std::sync::atomic::Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    identity: self.identity.clone(),
                    correlation,
                    reason: TransportFailure::SessionClosed,
                };
            }
            self.script
                .sent
                .lock()
                .expect("script lock")
                .push(String::from_utf8_lossy(&frame).into_owned());
            HandoffOutcome::HandedOff {
                identity: self.identity.clone(),
                correlation,
            }
        })
    }

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        Box::new(ScriptedStream {
            identity: self.identity.clone(),
            script: Arc::clone(&self.script),
            closed: Arc::clone(&self.closed),
        })
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
            self.script.notify.notify_waiters();
            Ok(ReleaseOutcome::Closed)
        })
    }
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
    script.disconnect("injected");
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
    let disconnect = &facts.failures[0].2;
    assert!(
        disconnect.starts_with("Disconnected") && disconnect.contains("injected"),
        "disconnect must stay a distinct, verbatim-carrying fact, got {disconnect}"
    );
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
