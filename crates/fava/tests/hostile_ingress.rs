//! A hostile relay stays scoped: nothing enters state, healthy work continues.

#[path = "hostile_ingress/cases.rs"]
mod cases;

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use tokio::sync::Notify;

/// One scripted relay driven by the frames a test hands it.
#[derive(Default)]
struct Script {
    inbound: Mutex<Vec<String>>,
    sent: Mutex<Vec<String>>,
    notify: Notify,
    closed: AtomicBool,
}

impl Script {
    fn push(&self, message: &RelayMessage<'_>) {
        self.inbound
            .lock()
            .expect("test lock")
            .push(serde_json::to_string(message).expect("relay message encodes"));
        self.notify.notify_waiters();
    }

    fn push_raw(&self, frame: &str) {
        self.inbound
            .lock()
            .expect("test lock")
            .push(frame.to_owned());
        self.notify.notify_waiters();
    }

    /// The subscription id the relay was actually asked for.
    fn subscription(&self) -> Option<SubscriptionId> {
        self.sent
            .lock()
            .expect("test lock")
            .iter()
            .find_map(
                |frame| match serde_json::from_str::<ClientMessage<'static>>(frame).ok()? {
                    ClientMessage::Req {
                        subscription_id, ..
                    } => Some(subscription_id.into_owned()),
                    _ => None,
                },
            )
    }
}

struct ScriptedTransport {
    scripts: BTreeMap<RelayUrl, Arc<Script>>,
}

impl Transport for ScriptedTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        let script = self.scripts.get(&key.relay).cloned();
        Box::pin(async move {
            let script =
                script.ok_or_else(|| TransportError::ConnectionRefused("no relay".to_owned()))?;
            Ok(Arc::new(ScriptedSession { key, script }) as Arc<dyn RelaySession>)
        })
    }
}

struct ScriptedSession {
    key: RelaySessionKey,
    script: Arc<Script>,
}

impl RelaySession for ScriptedSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        1
    }

    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            self.script.sent.lock().expect("test lock").push(frame);
            HandoffOutcome::HandedOff
        })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            loop {
                if self.script.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed);
                }
                let next = {
                    let mut inbound = self.script.inbound.lock().expect("test lock");
                    if inbound.is_empty() {
                        None
                    } else {
                        Some(inbound.remove(0))
                    }
                };
                if let Some(frame) = next {
                    return Ok(frame);
                }
                self.script.notify.notified().await;
            }
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move {
            self.script.closed.store(true, Ordering::SeqCst);
            self.script.notify.notify_waiters();
            Ok(())
        })
    }
}

async fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + limit;
    while !ready() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "condition was not reached before the deadline"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

fn signed(keys: &Keys, content: &str, created_at: u64) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("event signs")
}
