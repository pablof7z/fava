//! Frozen-contract falsifiers for `fava-transport` (FROZEN-CONTRACTS.md §8, §1).

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, OperationGeneration, RelayInbound,
    Transport, TransportBounds, TransportDeadlines, TransportFailure,
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
        initial_generation: OperationGeneration::new(1),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn one_physical_session_fans_out_every_inbound_frame_to_every_consumer() {
    let transport = FakeTransport::new();
    let lease = transport
        .acquire_session(request())
        .await
        .expect("acquires");
    let mut first = lease.session().messages();
    let mut second = lease.session().messages();

    transport
        .relay(&key())
        .expect("relay is registered")
        .push_frame(b"[\"EOSE\",\"one\"]".to_vec());

    let one = first.next_inbound().await.expect("first consumer receives");
    let two = second
        .next_inbound()
        .await
        .expect("second consumer receives");
    assert!(matches!(one, RelayInbound::Frame { ref frame, .. } if frame == b"[\"EOSE\",\"one\"]"));
    assert!(matches!(two, RelayInbound::Frame { ref frame, .. } if frame == b"[\"EOSE\",\"one\"]"));
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
        first.session().identity().generation,
        second.session().identity().generation
    );
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
            .send(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation(attempt))
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
        .send(b"[\"REQ\",\"a\",{}]".to_vec(), HandoffCorrelation(7))
        .await;

    assert_eq!(outcome.identity(), &expected);
    assert_eq!(outcome.correlation(), HandoffCorrelation(7));
}
