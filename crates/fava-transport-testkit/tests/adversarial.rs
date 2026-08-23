//! The five adversarial classes the frozen contract requires the relay fake to
//! express: pending establishment, mid-operation failure, mid-operation
//! cancellation, stale completion, and a slow peer applying backpressure.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, ReleaseOutcome, Transport,
    TransportAmbiguity, TransportBounds, TransportDeadlines, TransportError, TransportFailure,
};
use fava_transport_testkit::FakeTransport;

fn key() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("ws://127.0.0.1:1/").expect("relay URL"),
        RelayAccess::public(),
    )
}

fn frames(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("non-zero")
}

fn request() -> OpenRelaySession {
    OpenRelaySession {
        key: key(),
        deadlines: TransportDeadlines {
            establish: Duration::from_millis(50),
            write: Duration::from_millis(50),
            idle: Duration::from_millis(50),
            close: Duration::from_millis(50),
        },
        bounds: TransportBounds {
            inbound_frames: frames(2),
            outbound_frames: frames(2),
            max_frame_bytes: frames(64),
        },
        reconnect_attempts: Some(frames(2)),
    }
}

// -------------------------------------------------- 1. pending establishment

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pending_establishment_expires_at_the_fava_owned_deadline() {
    let transport = FakeTransport::new();
    transport.hold_establishment(&key());

    let refusal = transport
        .acquire_session(request())
        .await
        .map(|_| ())
        .expect_err("establishment never completes");

    assert_eq!(
        refusal,
        TransportError::ConnectionRefused(TransportFailure::EstablishTimeout {
            after: Duration::from_millis(50)
        })
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn pending_establishment_that_completes_is_not_a_refusal() {
    let transport = FakeTransport::new();
    transport.hold_establishment(&key());
    let pending = tokio::spawn({
        let transport = transport.clone();
        async move { transport.acquire_session(request()).await.map(|_| ()) }
    });

    tokio::time::sleep(Duration::from_millis(10)).await;
    transport.release_establishment(&key());

    pending
        .await
        .expect("task joins")
        .expect("establishment completes inside its deadline");
    assert_eq!(transport.dials(&key()), 1);
}

// -------------------------------------------------- 2. mid-operation failure

#[tokio::test(flavor = "current_thread")]
async fn mid_operation_failure_reaches_every_consumer_as_an_attributed_disconnect() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let identity = lease.session().identity();
    let mut stream = lease.session().messages();

    transport
        .relay(&key())
        .expect("relay is registered")
        .fail_now("relay closed the connection");

    let item = stream.next_inbound().await.expect("a lifecycle item arrives");
    match item {
        RelayInbound::Disconnected {
            identity: reported,
            reason: TransportFailure::Disconnected { detail },
        } => {
            assert_eq!(reported, identity);
            assert_eq!(detail.as_str(), "relay closed the connection");
        }
        other => panic!("expected an attributed disconnect, got {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn a_frame_in_flight_when_the_session_fails_is_ambiguous_not_lost() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();

    let outcome = lease
        .session()
        .send(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation(1))
        .await;
    assert!(matches!(outcome, HandoffOutcome::HandedOff { .. }));

    relay.fail_now("socket reset mid-write");

    assert_eq!(
        relay.unflushed_completions(),
        vec![HandoffOutcome::Ambiguous {
            identity: lease.session().identity(),
            correlation: HandoffCorrelation(1),
            reason: TransportAmbiguity::DisconnectedInFlight {
                detail: fava_transport::BoundedReason::new("socket reset mid-write"),
            },
        }]
    );
}

// -------------------------------------------------- 3. mid-operation cancel

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn cancelling_a_handoff_mid_operation_leaves_no_half_frame() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();
    relay.block_queue();

    let send = lease
        .session()
        .send(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation(9));
    let cancelled = tokio::time::timeout(Duration::from_millis(10), send).await;

    assert!(cancelled.is_err(), "the handoff must still be in flight");
    assert_eq!(relay.queued_frames(), Vec::<Vec<u8>>::new());
    assert_eq!(relay.cancelled_handoffs(), vec![HandoffCorrelation(9)]);
}

// -------------------------------------------------- 4. stale completion

#[tokio::test(flavor = "current_thread")]
async fn a_reconnect_mints_a_new_generation_under_the_same_lease() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let before = lease.session().identity();
    let mut stream = lease.session().messages();

    transport
        .relay(&key())
        .expect("relay is registered")
        .reconnect();

    let item = loop {
        let next = stream.next_inbound().await.expect("a lifecycle item arrives");
        if !matches!(next, RelayInbound::Disconnected { .. }) {
            break next;
        }
    };
    let RelayInbound::Reconnected { previous, identity } = item else {
        panic!("expected a reconnect item, got {item:?}");
    };

    assert_eq!(previous, before);
    assert_eq!(identity.key, before.key);
    assert_eq!(identity.generation, before.generation.next());
    assert_eq!(lease.session().identity(), identity);
    assert_eq!(transport.dials(&key()), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stale_generation_completion_is_attributable_to_the_generation_that_made_it() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let stale = lease.session().identity();
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();
    let _ = lease
        .session()
        .send(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation(3))
        .await;

    relay.reconnect();
    let current = lease.session().identity();
    let completion = relay
        .unflushed_completions()
        .into_iter()
        .next()
        .expect("the in-flight frame produced a completion");

    assert_ne!(stale, current);
    assert_eq!(completion.identity(), &stale);
    assert_eq!(completion.correlation(), HandoffCorrelation(3));
}

#[tokio::test(flavor = "current_thread")]
async fn reconnect_exhaustion_is_an_item_not_a_silent_stop() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let mut stream = lease.session().messages();
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.refuse_reconnects("relay refuses");

    relay.fail_now("relay dropped us");

    let exhausted = loop {
        if let RelayInbound::ReconnectExhausted {
            attempts, reason, ..
        } = stream.next_inbound().await.expect("a lifecycle item arrives")
        {
            break (attempts, reason);
        }
    };
    assert_eq!(exhausted.0, 2);
    assert!(matches!(exhausted.1, TransportFailure::Disconnected { .. }));
}

// -------------------------------------------------- 5. slow peer backpressure

#[tokio::test(flavor = "current_thread")]
async fn a_slow_consumer_loses_bounded_inbound_items_and_is_told_exactly() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");
    let mut fast = lease.session().messages();
    let slow = lease.session().messages();
    let relay = transport.relay(&key()).expect("relay is registered");

    for index in 0..6_u8 {
        relay.push_frame(vec![index]);
    }

    drop(slow);
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(fast.next_inbound().await.expect("items arrive"));
    }
    let lost = seen
        .iter()
        .find_map(|item| match item {
            RelayInbound::Lost { dropped, .. } => Some(*dropped),
            _ => None,
        })
        .expect("the bounded consumer is told its exact loss");
    assert_eq!(lost, 4);
}

#[tokio::test(flavor = "current_thread")]
async fn a_slow_peer_never_parks_an_unrelated_sender_on_the_same_session() {
    let transport = FakeTransport::new();
    let first = transport.acquire_session(request()).await.expect("acquires");
    let second = transport.acquire_session(request()).await.expect("reuses");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();

    for attempt in 0..2_u64 {
        let _ = first
            .session()
            .send(vec![b'a'], HandoffCorrelation(attempt))
            .await;
    }

    let refused = tokio::time::timeout(
        Duration::from_millis(200),
        second.session().send(vec![b'b'], HandoffCorrelation(99)),
    )
    .await
    .expect("the second holder is refused, never parked");

    assert!(matches!(
        refused,
        HandoffOutcome::NotHandedOff {
            reason: TransportFailure::OutboundQueueFull { .. },
            ..
        }
    ));
}

// -------------------------------------------------- lease refcount and close

#[tokio::test(flavor = "current_thread")]
async fn the_last_release_closes_deterministically() {
    let transport = FakeTransport::new();
    let first = transport.acquire_session(request()).await.expect("acquires");
    let second = transport.acquire_session(request()).await.expect("reuses");

    assert_eq!(
        second.release().await.expect("releases"),
        ReleaseOutcome::Retained {
            holders: frames(1)
        }
    );
    assert_eq!(
        first.release().await.expect("releases"),
        ReleaseOutcome::Closed
    );
    assert_eq!(transport.holders(&key()), None);
}

#[tokio::test(flavor = "current_thread")]
async fn shutdown_closes_every_registered_session() {
    let transport = FakeTransport::new();
    let lease = transport.acquire_session(request()).await.expect("acquires");

    transport
        .shutdown(Duration::from_millis(50))
        .await
        .expect("shutdown joins");

    assert_eq!(transport.holders(&key()), None);
    assert!(matches!(
        lease
            .session()
            .send(vec![b'a'], HandoffCorrelation(1))
            .await,
        HandoffOutcome::NotHandedOff {
            reason: TransportFailure::SessionClosed,
            ..
        }
    ));
    assert_eq!(
        transport
            .acquire_session(request())
            .await
            .map(|_| ())
            .unwrap_err(),
        TransportError::ShuttingDown
    );
}
