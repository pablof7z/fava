//! The five adversarial classes the frozen contract requires the relay fake to
//! express: pending establishment, mid-operation failure, mid-operation
//! cancellation, stale completion, and a slow peer applying backpressure.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, ReleaseOutcome, Transport,
    TransportAmbiguity, TransportBounds, TransportDeadlines, TransportError, TransportFailure,
};
use fava_transport_testkit::FakeTransport;
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

/// Drain a session's connection stream until one state matches, yielding between
/// polls so the fake's own tasks can run.
async fn until(
    connection: &mut tokio::sync::watch::Receiver<fava_transport::Connection>,
    mut matches: impl FnMut(&fava_transport::Connection) -> bool,
) -> fava_transport::Connection {
    connection
        .wait_for(|state| matches(state))
        .await
        .expect("the connection still reports its state")
        .clone()
}

#[tokio::test(flavor = "current_thread")]
async fn mid_operation_failure_reaches_every_reader_as_an_attributed_disconnect() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);

    transport
        .relay(&key())
        .expect("relay is registered")
        .fail_now("relay closed the connection");

    let state = until(&mut connection, |state| {
        matches!(
            state.connectivity,
            fava_transport::Connectivity::Disconnected { .. }
        )
    })
    .await;
    let fava_transport::Connectivity::Disconnected { detail, .. } = state.connectivity else {
        unreachable!("the predicate matched a disconnect")
    };
    assert_eq!(detail.as_str(), "relay closed the connection");
}

#[tokio::test(flavor = "current_thread")]
async fn a_frame_in_flight_when_the_session_fails_is_ambiguous_not_lost() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();

    let outcome = lease
        .session()
        .hand_off(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation::new(1))
        .await;
    assert!(matches!(outcome, HandoffOutcome::HandedOff { .. }));

    relay.fail_now("socket reset mid-write");

    assert_eq!(
        relay.unflushed_completions(),
        vec![HandoffOutcome::Ambiguous {
            identity: lease.session().identity(),
            correlation: HandoffCorrelation::new(1),
            reason: TransportAmbiguity::DisconnectedInFlight {
                detail: fava_transport::BoundedText::new("socket reset mid-write"),
            },
        }]
    );
}

// -------------------------------------------------- 3. mid-operation cancel

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn cancelling_a_handoff_mid_operation_leaves_no_half_frame() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();
    relay.block_queue();

    let send = lease
        .session()
        .hand_off(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation::new(9));
    let cancelled = tokio::time::timeout(Duration::from_millis(10), send).await;

    assert!(cancelled.is_err(), "the handoff must still be in flight");
    assert_eq!(relay.queued_frames(), Vec::<Vec<u8>>::new());
    assert_eq!(relay.cancelled_handoffs(), vec![HandoffCorrelation::new(9)]);
}

// -------------------------------------------------- 4. stale completion

#[tokio::test(flavor = "current_thread")]
async fn a_reconnect_mints_a_new_generation_under_the_same_lease() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let before = lease.session().identity();
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);

    transport
        .relay(&key())
        .expect("relay is registered")
        .reconnect();

    let state = until(&mut connection, |state| {
        matches!(state.connectivity, fava_transport::Connectivity::Connected)
            && state.identity != before
    })
    .await;
    let identity = state.identity;

    assert_eq!(identity.key, before.key);
    assert_eq!(
        identity.connection,
        before.connection.checked_next().expect("successor exists")
    );
    assert_eq!(lease.session().identity(), identity);
    assert_eq!(transport.dials(&key()), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stale_generation_completion_is_attributable_to_the_generation_that_made_it() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let stale = lease.session().identity();
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();
    let _ = lease
        .session()
        .hand_off(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation::new(3))
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
    assert_eq!(completion.correlation(), HandoffCorrelation::new(3));
}

#[tokio::test(flavor = "current_thread")]
async fn reconnect_exhaustion_is_an_item_not_a_silent_stop() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.refuse_reconnects("relay refuses");

    relay.fail_now("relay dropped us");

    let state = until(&mut connection, |state| {
        matches!(
            state.connectivity,
            fava_transport::Connectivity::Disconnected { spent: Some(_), .. }
        )
    })
    .await;
    let fava_transport::Connectivity::Disconnected {
        spent: Some(attempts),
        ..
    } = state.connectivity
    else {
        unreachable!("the predicate matched a spent budget")
    };
    assert_eq!(
        attempts, 2,
        "an exhausted reconnect budget says how many attempts it spent"
    );
}

// -------------------------------------------------- 5. slow peer backpressure

#[tokio::test(flavor = "current_thread")]
async fn a_slow_subscription_loses_bounded_items_and_is_told_exactly() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let session = std::sync::Arc::clone(lease.session());
    let mut subscription =
        fava_transport::RelaySessionExt::subscribe(&session, vec![nostr::filter::Filter::new()])
            .await
            .expect("subscription opens");
    let relay = transport.relay(&key()).expect("relay is registered");

    // The queue is bounded at two frames; six arrive with nothing draining them.
    for _ in 0..6 {
        relay.push_frame(format!("[\"EOSE\",\"{}\"]", subscription.id().as_str()).as_bytes());
    }

    // Read until the stream has nothing more to say. What survived the bound
    // and what it says was lost must together account for all six: a reader
    // learns the exact size of its gap, never a silent one.
    let mut delivered = 0_u64;
    let mut lost = 0_u64;
    while let Ok(item) = tokio::time::timeout(Duration::from_millis(200), subscription.next()).await
    {
        match item {
            fava_transport::SubscriptionItem::EndOfStoredEvents => delivered += 1,
            fava_transport::SubscriptionItem::Lost { dropped } => lost += dropped,
            other => panic!("an overflowing subscription said something else: {other:?}"),
        }
    }
    assert!(lost > 0, "the overflow is reported, not silently dropped");
    assert_eq!(
        delivered + lost,
        6,
        "six offered, {delivered} buffered, {lost} told as lost"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_slow_peer_never_parks_an_unrelated_sender_on_the_same_session() {
    let transport = FakeTransport::new();
    let first = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let second = transport.acquire_session(request()).await.expect("reuses");
    let relay = transport.relay(&key()).expect("relay is registered");
    relay.stall_writer();

    for attempt in 0..2_u64 {
        let _ = first
            .session()
            .hand_off(vec![b'a'], HandoffCorrelation::new(attempt))
            .await;
    }

    let refused = tokio::time::timeout(
        Duration::from_millis(200),
        second
            .session()
            .hand_off(vec![b'b'], HandoffCorrelation::new(99)),
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
    let first = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let second = transport.acquire_session(request()).await.expect("reuses");

    assert_eq!(
        second.release().await.expect("releases"),
        ReleaseOutcome::Retained { holders: frames(1) }
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
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");

    transport
        .shutdown(Duration::from_millis(50))
        .await
        .expect("shutdown joins");

    assert_eq!(transport.holders(&key()), None);
    assert!(matches!(
        lease
            .session()
            .hand_off(vec![b'a'], HandoffCorrelation::new(1))
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

/// The transport says when a relay asks, and says nothing otherwise.
#[tokio::test(flavor = "current_thread")]
async fn a_relay_asking_to_authenticate_reaches_a_listener() {
    let transport = FakeTransport::new();
    let mut requests = fava_transport::Transport::authentication_requests(&transport);
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let session = std::sync::Arc::clone(lease.session());
    let relay = transport.relay(&key()).expect("relay is registered");

    // A connection nobody challenged is not this stream's business.
    relay.push_frame(b"[\"NOTICE\",\"nothing to do with authentication\"]");
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    assert!(
        requests.try_recv().is_err(),
        "a connection the relay never asked is not published"
    );

    relay.push_frame(b"[\"AUTH\",\"nonce-1\"]");
    let asked = requests
        .recv()
        .await
        .expect("the request reaches a listener");
    assert!(
        std::sync::Arc::ptr_eq(&asked, &session),
        "the session handed over is the one the relay asked"
    );
    assert!(matches!(
        fava_transport::RelaySessionExt::connection(&asked)
            .borrow()
            .authentication,
        fava_transport::Authentication::Requested { .. }
    ));
}
