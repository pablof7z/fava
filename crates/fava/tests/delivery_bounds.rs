//! Offline time, attempt ceilings, and ambiguity are exact and separate.

use std::future::Future;
use std::num::NonZeroU32;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::time::Duration;

use fava::{Fava, RelayDeliveryOutcome, WriteIntent, WriteRouting};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::{ClientMessage, RelayMessage};
use fava_write::{EventBuilder, Kind, Receipt, ReceiptId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

const UNREACHABLE: u8 = 0;
const REFUSES_HANDOFF: u8 = 1;
const AMBIGUOUS_AFTER_HANDOFF: u8 = 2;
const ACKNOWLEDGES: u8 = 3;

/// A transport whose behavior a test switches at will.
#[derive(Default)]
struct SwitchableTransport {
    mode: AtomicU8,
    connections: AtomicUsize,
}

impl SwitchableTransport {
    fn set(&self, mode: u8) {
        self.mode.store(mode, Ordering::SeqCst);
    }
}

impl Transport for SwitchableTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            if self.mode.load(Ordering::SeqCst) == UNREACHABLE {
                return Err(TransportError::ConnectionRefused(
                    "relay is offline".to_owned(),
                ));
            }
            self.connections.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(SwitchableSession {
                key,
                mode: self.mode.load(Ordering::SeqCst),
                accepted: std::sync::Mutex::new(None),
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct SwitchableSession {
    key: RelaySessionKey,
    mode: u8,
    accepted: std::sync::Mutex<Option<fava_write::EventId>>,
}

impl RelaySession for SwitchableSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        1
    }

    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.mode == REFUSES_HANDOFF {
                return HandoffOutcome::NotHandedOff {
                    reason: "relay refused the frame".to_owned(),
                };
            }
            if let Ok(ClientMessage::Event(event)) =
                serde_json::from_str::<ClientMessage<'static>>(&frame)
            {
                *self.accepted.lock().expect("test lock") = Some(event.id);
            }
            HandoffOutcome::HandedOff
        })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            if self.mode == AMBIGUOUS_AFTER_HANDOFF {
                return Err(TransportError::Disconnected(
                    "connection ended after the frame crossed".to_owned(),
                ));
            }
            loop {
                let accepted = *self.accepted.lock().expect("test lock");
                if let Some(id) = accepted {
                    return Ok(serde_json::to_string(&RelayMessage::ok(id, true, ""))
                        .expect("relay message encodes"));
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move { Ok(()) })
    }
}

fn relay() -> RelayUrl {
    RelayUrl::parse("ws://127.0.0.1:9").expect("relay URL")
}

fn destination() -> RelaySessionKey {
    RelaySessionKey::new(relay(), RelayAccess::public())
}

fn assembly(transport: Arc<SwitchableTransport>, ceiling: u32) -> (Fava, Keys) {
    let keys = Keys::generate();
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(transport)
        .signer(Arc::new(LocalSigner::new(keys.clone())))
        .publisher(Arc::new(Nip01Publisher::new()))
        .delivery_policy(Arc::new(
            StandardDeliveryPolicy::new(NonZeroU32::new(ceiling).expect("ceiling is non-zero"))
                .retrying_unreachable_after(Duration::from_millis(10)),
        ))
        .build()
        .expect("assembly builds");
    (fava, keys)
}

fn publish(fava: &Fava, keys: &Keys) -> ReceiptId {
    fava.publish(
        WriteIntent::event(
            EventBuilder::new(keys.public_key(), Kind::TextNote)
                .content("bounded delivery")
                .build()
                .expect("event builds"),
            WriteRouting::Explicit([relay()].into_iter().collect()),
        )
        .expect("intent is valid"),
    )
    .expect("write is accepted")
    .receipt_id
}

async fn wait_for(
    fava: &Fava,
    receipt_id: ReceiptId,
    limit: Duration,
    mut ready: impl FnMut(&Receipt) -> bool,
) -> Receipt {
    let deadline = tokio::time::Instant::now() + limit;
    loop {
        let receipt = fava
            .receipt(receipt_id)
            .expect("receipt is readable")
            .expect("receipt exists");
        if ready(&receipt) {
            return receipt;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "receipt never reached the expected state: {receipt:?}"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

#[tokio::test]
async fn offline_time_spends_no_attempt_budget_and_the_write_stays_open() {
    let transport = Arc::new(SwitchableTransport::default());
    transport.set(UNREACHABLE);
    let (fava, keys) = assembly(Arc::clone(&transport), 1);
    let receipt_id = publish(&fava, &keys);

    let parked = wait_for(&fava, receipt_id, Duration::from_secs(5), |receipt| {
        matches!(
            receipt.destinations().get(&destination()),
            Some(RelayDeliveryOutcome::Unreachable { .. })
        )
    })
    .await;
    assert!(!parked.is_terminal(), "an offline relay is not a failure");
    let parked_generation = parked
        .attempts
        .get(&destination())
        .copied()
        .expect("the first unreachable attempt has exact identity");
    assert_eq!(parked_generation, 1);

    // Stay offline long enough to cross the ceiling many times over.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let still_parked = fava
        .receipt(receipt_id)
        .expect("receipt is readable")
        .expect("receipt exists");
    assert_eq!(
        still_parked.spent(&destination()),
        0,
        "no attempt was spent while no connection existed"
    );
    let delayed_generation = still_parked
        .attempts
        .get(&destination())
        .copied()
        .expect("the delayed retry has exact identity");
    assert!(
        delayed_generation > parked_generation,
        "WaitFor must authorize a delayed store-revalidated generation"
    );
    assert_ne!(
        delayed_generation,
        still_parked.spent(&destination()),
        "operation generation is not the spent-attempt policy budget"
    );
    assert!(
        matches!(
            still_parked.destinations().get(&destination()),
            Some(RelayDeliveryOutcome::Unreachable { .. })
        ),
        "unreachable never becomes given-up through the passage of time"
    );
    assert!(!still_parked.is_terminal());
    assert_eq!(transport.connections.load(Ordering::SeqCst), 0);

    // The same obligation resumes when a real attempt becomes possible.
    transport.set(REFUSES_HANDOFF);
    let terminal = tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(receipt_id))
        .await
        .expect("receipt settles once real attempts happen")
        .expect("receipt is readable");
    assert_eq!(
        terminal.spent(&destination()),
        1,
        "exactly the real attempt was spent"
    );
    assert!(
        terminal
            .attempts
            .get(&destination())
            .is_some_and(|generation| *generation > delayed_generation),
        "the real attempt advances the exact operation generation"
    );
    assert_eq!(
        transport.connections.load(Ordering::SeqCst),
        1,
        "only the real reachable handoff opens a session"
    );
    assert!(matches!(
        terminal.destinations().get(&destination()),
        Some(RelayDeliveryOutcome::GivenUp { .. })
    ));
}

#[tokio::test]
async fn real_retryable_attempts_reach_give_up_inside_the_declared_ceiling() {
    let transport = Arc::new(SwitchableTransport::default());
    transport.set(REFUSES_HANDOFF);
    let (fava, keys) = assembly(Arc::clone(&transport), 3);
    let receipt_id = publish(&fava, &keys);

    let terminal = tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(receipt_id))
        .await
        .expect("receipt settles")
        .expect("receipt is readable");

    let outcome = terminal
        .destinations()
        .get(&destination())
        .expect("one exact destination fact");
    let RelayDeliveryOutcome::GivenUp { reason } = outcome else {
        panic!("expected the declared give-up, got {outcome:?}");
    };
    assert_eq!(
        reason, "attempt ceiling 3 reached after: relay refused the frame",
        "the give-up names the exact ceiling and the last real failure"
    );
    assert_eq!(
        terminal.spent(&destination()),
        3,
        "exactly the declared ceiling was spent"
    );
    assert_eq!(transport.connections.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn a_crossed_handoff_without_an_outcome_stays_ambiguous() {
    let transport = Arc::new(SwitchableTransport::default());
    transport.set(AMBIGUOUS_AFTER_HANDOFF);
    let (fava, keys) = assembly(Arc::clone(&transport), 3);
    let receipt_id = publish(&fava, &keys);

    let terminal = tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(receipt_id))
        .await
        .expect("receipt settles")
        .expect("receipt is readable");

    let outcome = terminal
        .destinations()
        .get(&destination())
        .expect("one exact destination fact");
    assert!(
        matches!(outcome, RelayDeliveryOutcome::Unknown { .. }),
        "a crossed frame with no relay answer is ambiguous, got {outcome:?}"
    );
    assert_eq!(
        transport.connections.load(Ordering::SeqCst),
        1,
        "ambiguity is never retried into a second handoff"
    );

    // Give the lane every chance to rewrite the fact.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let later = fava
        .receipt(receipt_id)
        .expect("receipt is readable")
        .expect("receipt exists");
    assert_eq!(
        later.destinations().get(&destination()),
        terminal.destinations().get(&destination()),
        "ambiguity is never rewritten as acknowledged, rejected, or never sent"
    );
    assert_eq!(later.spent(&destination()), 1);

    // Cancellation cannot claim the bytes never crossed.
    let cancelled = fava.cancel_publication(receipt_id);
    let after_cancel = fava
        .receipt(receipt_id)
        .expect("receipt is readable")
        .expect("receipt exists");
    assert!(
        matches!(
            after_cancel.destinations().get(&destination()),
            Some(RelayDeliveryOutcome::Unknown { .. })
        ),
        "cancellation left {:?}",
        cancelled.map(|receipt| receipt.map(|receipt| receipt.outcome))
    );
}

#[tokio::test]
async fn a_relay_that_answers_is_acknowledged_rather_than_ambiguous() {
    // Guards against a policy that treats every crossed handoff as ambiguous.
    let transport = Arc::new(SwitchableTransport::default());
    transport.set(ACKNOWLEDGES);
    let (fava, keys) = assembly(Arc::clone(&transport), 3);
    let receipt_id = publish(&fava, &keys);

    let terminal = tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(receipt_id))
        .await
        .expect("receipt settles")
        .expect("receipt is readable");

    assert_eq!(
        terminal.destinations().get(&destination()),
        Some(&RelayDeliveryOutcome::Acknowledged {
            message: String::new()
        })
    );
    assert_eq!(terminal.spent(&destination()), 1);
}
