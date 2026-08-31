//! Frozen-contract falsifiers for `fava-transport` (FROZEN-CONTRACTS.md §8, §1).

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, Transport, TransportBounds,
    TransportDeadlines, TransportFailure,
};
use fava_transport_testkit::FakeTransport;
use nostr::filter::Filter;
use nostr::types::RelayUrl;

fn key() -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse("ws://127.0.0.1:1/").expect("relay URL"),
        access: RelayAccess::Public,
    }
}

fn frames(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("non-zero")
}

fn request() -> OpenRelaySession {
    OpenRelaySession {
        key: key(),
        deadlines: TransportDeadlines {
            establish: Duration::from_millis(200),
            write: Duration::from_millis(200),
            idle: Duration::from_secs(30),
            close: Duration::from_millis(200),
        },
        bounds: TransportBounds {
            inbound_frames: frames(8),
            outbound_frames: frames(2),
            max_frame_bytes: frames(1024),
        },
        reconnect_attempts: Some(frames(3)),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn one_relay_message_reaches_only_the_subscription_that_owns_it() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("session opens");
    let session = std::sync::Arc::clone(lease.session());
    let mut owner = fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
        .await
        .expect("subscription opens");
    let mut bystander = fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
        .await
        .expect("second subscription opens");

    let relay = transport.relay(&key()).expect("peer");
    relay.push_frame(format!("[\"EOSE\",\"{}\"]", owner.id().as_str()).as_bytes());

    assert!(matches!(
        owner.next().await,
        fava_transport::SubscriptionItem::EndOfStoredEvents
    ));
    // The bystander asked for something else, so nothing reaches it.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), bystander.next())
            .await
            .is_err(),
        "a subscription must never receive another subscription's message"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn acquiring_a_live_session_does_not_dial() {
    let transport = FakeTransport::new();
    let first = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let second = transport.acquire_session(request()).await.expect("reuses");

    assert_eq!(transport.holders(&key()), Some(frames(2)));
    assert_eq!(transport.dials(&key()), 1);
    assert_eq!(
        first.session().identity().connection,
        second.session().identity().connection
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reacquiring_after_last_release_mints_a_fresh_physical_generation() {
    let transport = FakeTransport::new();
    let first = transport
        .acquire_session(request())
        .await
        .expect("first session acquires");
    let first_identity = first.session().identity();
    first.release().await.expect("last lease releases");

    let second = transport
        .acquire_session(request())
        .await
        .expect("second session acquires");

    assert_ne!(first_identity, second.session().identity());
    assert_eq!(transport.dials(&key()), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn exhausted_initial_generation_refuses_before_a_dial() {
    let transport = FakeTransport::new();
    transport.exhaust_generations();

    let error = transport
        .acquire_session(request())
        .await
        .map(|_| ())
        .expect_err("exhaustion refuses acquisition");

    assert_eq!(error, fava_transport::TransportError::GenerationExhausted);
    assert_eq!(transport.dials(&key()), 0);
    assert_eq!(transport.holders(&key()), None);
}

#[tokio::test(flavor = "current_thread")]
async fn exhausted_reconnect_generation_is_terminal_without_a_dial() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("session opens");
    let session = std::sync::Arc::clone(lease.session());
    let connection = fava_transport::RelaySessionExt::connection(&session);
    let dials = transport.dials(&key());

    transport.exhaust_generations();
    transport.relay(&key()).expect("peer").reconnect();

    let mut seen = Vec::new();
    for _ in 0..8 {
        while let Some(state) = connection.take() {
            seen.push(state);
        }
        if seen
            .iter()
            .any(|state| matches!(state, fava_transport::ConnectionState::Unreachable { .. }))
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        seen.iter()
            .any(|state| matches!(state, fava_transport::ConnectionState::Unreachable { .. })),
        "an exhausted reconnect budget is reported, not retried: {seen:?}"
    );
    assert_eq!(transport.dials(&key()), dials, "no dial was attempted");
}

#[tokio::test(flavor = "current_thread")]
async fn stalled_relay_yields_bounded_refusal_not_an_unbounded_park() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    transport
        .relay(&key())
        .expect("relay is registered")
        .stall_writer();

    let mut refusal = None;
    for attempt in 0..8_u64 {
        let outcome = lease
            .session()
            .hand_off(
                b"[\"REQ\",\"a\",{}]".to_vec(),
                HandoffCorrelation::new(attempt),
            )
            .await;
        if let HandoffOutcome::NotHandedOff { reason, .. } = outcome {
            refusal = Some(reason);
            break;
        }
    }

    assert!(
        matches!(
            refusal,
            Some(TransportFailure::OutboundQueueFull { capacity: 2 })
        ),
        "expected a bounded outbound refusal, got {refusal:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn handoff_completion_names_its_own_session_generation() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let expected = lease.session().identity();

    let outcome = lease
        .session()
        .hand_off(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation::new(7))
        .await;

    assert_eq!(outcome.identity(), &expected);
    assert_eq!(outcome.correlation(), HandoffCorrelation::new(7));
}
