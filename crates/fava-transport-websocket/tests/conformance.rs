//! Standard WebSocket transport conformance evidence, over real sockets.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, ReleaseOutcome, Transport,
    TransportBounds, TransportDeadlines, TransportError, TransportFailure,
};
use fava_transport_testkit::{
    require_acquire_reuses_live_session, require_attributed_handoff,
    require_bounded_outbound_refusal,
};
use futures_util::{SinkExt, StreamExt};
use nostr::types::RelayUrl;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use fava_transport_websocket::WebSocketTransport;

fn frames(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("non-zero")
}

async fn listener() -> (TcpListener, RelaySessionKey) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener binds");
    let url = RelayUrl::parse(&format!("ws://{}", listener.local_addr().expect("address")))
        .expect("relay URL");
    (
        listener,
        RelaySessionKey {
            relay: url,
            access: RelayAccess::Public,
        },
    )
}

fn request(key: RelaySessionKey) -> OpenRelaySession {
    OpenRelaySession {
        key,
        deadlines: TransportDeadlines {
            establish: Duration::from_secs(2),
            write: Duration::from_secs(2),
            idle: Duration::from_secs(10),
            close: Duration::from_secs(1),
        },
        bounds: TransportBounds {
            inbound_frames: frames(8),
            outbound_frames: frames(1),
            max_frame_bytes: frames(1_048_576),
        },
        reconnect_attempts: Some(frames(1)),
    }
}

// ------------------------------------------------------- section 8 falsifiers

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_physical_session_fans_out_every_inbound_frame_to_every_consumer() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket
            .send(Message::Text("[\"EOSE\",\"one\"]".into()))
            .await
            .expect("server sends");
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let transport = WebSocketTransport::new();
    let lease = transport
        .acquire_session(request(key))
        .await
        .expect("session opens");
    let mut first = lease.session().messages();
    let mut second = lease.session().messages();

    for stream in [&mut first, &mut second] {
        let item = stream.next_inbound().await.expect("a frame arrives");
        assert!(
            matches!(item, RelayInbound::Frame { ref frame, .. } if frame == b"[\"EOSE\",\"one\"]"),
            "consumer did not see the frame, got {item:?}"
        );
    }
    server.await.expect("server joins");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn acquiring_a_live_session_does_not_dial() {
    let (listener, key) = listener().await;
    let accepted = tokio::spawn(async move {
        let mut count = 0_usize;
        while let Ok(Ok((stream, _))) =
            tokio::time::timeout(Duration::from_millis(400), listener.accept()).await
        {
            count += 1;
            tokio::spawn(async move {
                let mut socket = accept_async(stream).await.expect("WebSocket accepts");
                while socket.next().await.is_some() {}
            });
        }
        count
    });

    let transport = WebSocketTransport::new();
    require_acquire_reuses_live_session(&transport, request(key))
        .await
        .expect("one key means one socket shared by two holders");

    assert_eq!(accepted.await.expect("server joins"), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_relay_yields_bounded_refusal_not_an_unbounded_park() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        // A peer that completes the handshake and then never reads: the socket
        // buffers fill, the bounded outbound queue fills, and Fava must refuse.
        let _socket = accept_async(stream).await.expect("WebSocket accepts");
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let transport = WebSocketTransport::new();
    let mut request = request(key);
    request.bounds.max_frame_bytes = frames(1_048_576);
    require_bounded_outbound_refusal(&transport, request, 256)
        .await
        .expect("a stalled peer becomes a refusal, not a park");
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handoff_completion_names_its_own_session_generation() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        while socket.next().await.is_some() {}
    });

    let transport = WebSocketTransport::new();
    require_attributed_handoff(&transport, request(key))
        .await
        .expect("every completion names its generation");
    server.abort();
}

// ------------------------------------------------------------- session facts

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complete_frame_handoff_reaches_the_relay() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket
            .next()
            .await
            .expect("one frame")
            .expect("valid frame")
    });

    let transport = WebSocketTransport::new();
    let lease = transport
        .acquire_session(request(key))
        .await
        .expect("session opens");
    let outcome = lease
        .session()
        .hand_off(b"[\"REQ\",\"one\",{}]".to_vec(), HandoffCorrelation::new(1))
        .await;

    assert!(matches!(outcome, HandoffOutcome::HandedOff { .. }));
    let received = server.await.expect("server joins");
    assert_eq!(received.into_text().expect("text"), "[\"REQ\",\"one\",{}]");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_oversized_frame_is_definitely_not_handed_off() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let _socket = accept_async(stream).await.expect("WebSocket accepts");
        tokio::time::sleep(Duration::from_millis(200)).await;
    });

    let transport = WebSocketTransport::new();
    let mut request = request(key);
    request.bounds.max_frame_bytes = frames(4);
    let lease = transport
        .acquire_session(request)
        .await
        .expect("session opens");
    let outcome = lease
        .session()
        .hand_off(b"too-large".to_vec(), HandoffCorrelation::new(2))
        .await;

    assert!(matches!(
        outcome,
        HandoffOutcome::NotHandedOff {
            reason: TransportFailure::FrameTooLarge {
                bytes: 9,
                maximum: 4
            },
            ..
        }
    ));
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_remote_close_reaches_every_consumer_as_an_attributed_disconnect() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket.close(None).await.expect("server closes");
    });

    let transport = WebSocketTransport::new();
    let mut request = request(key.clone());
    request.reconnect_attempts = Some(frames(1));
    let lease = transport
        .acquire_session(request)
        .await
        .expect("session opens");
    let identity = lease.session().identity();
    let mut stream = lease.session().messages();

    let item = stream
        .next_inbound()
        .await
        .expect("a lifecycle item arrives");
    assert!(
        matches!(item, RelayInbound::Disconnected { identity: ref reported, .. } if *reported == identity),
        "expected an attributed disconnect, got {item:?}"
    );
    server.await.expect("server joins");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn establishment_refusal_is_scoped_and_never_a_hang() {
    let (listener, key) = listener().await;
    drop(listener);

    let transport = WebSocketTransport::new();
    let mut request = request(key);
    request.deadlines.establish = Duration::from_millis(300);
    let refusal = transport
        .acquire_session(request)
        .await
        .map(|_| ())
        .expect_err("nothing is listening");

    assert!(
        matches!(refusal, TransportError::ConnectionRefused(_)),
        "expected a scoped refusal, got {refusal:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_last_release_closes_deterministically() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        while socket.next().await.is_some() {}
    });

    let transport = WebSocketTransport::new();
    let first = transport
        .acquire_session(request(key.clone()))
        .await
        .expect("session opens");
    let second = transport
        .acquire_session(request(key.clone()))
        .await
        .expect("session is reused");

    assert_eq!(
        second.release().await.expect("releases"),
        ReleaseOutcome::Retained { holders: frames(1) }
    );
    assert_eq!(
        first.release().await.expect("releases"),
        ReleaseOutcome::Closed
    );
    assert_eq!(transport.holders(&key), None);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_closes_every_session_and_refuses_new_work() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        while socket.next().await.is_some() {}
    });

    let transport = WebSocketTransport::new();
    let _lease = transport
        .acquire_session(request(key.clone()))
        .await
        .expect("session opens");

    transport
        .shutdown(Duration::from_secs(2))
        .await
        .expect("shutdown joins every session");

    assert_eq!(transport.holders(&key), None);
    assert_eq!(
        transport
            .acquire_session(request(key))
            .await
            .map(|_| ())
            .expect_err("shutdown refuses new work"),
        TransportError::ShuttingDown
    );
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reconnect_mints_a_new_generation_under_the_same_lease() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        // The first generation is closed by the relay; the second survives.
        let (stream, _) = listener.accept().await.expect("first connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket
            .close(None)
            .await
            .expect("relay closes generation one");
        let (stream, _) = listener.accept().await.expect("second connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        while socket.next().await.is_some() {}
    });

    let transport = WebSocketTransport::new();
    let mut request = request(key);
    request.reconnect_attempts = Some(frames(4));
    let lease = transport
        .acquire_session(request)
        .await
        .expect("session opens");
    let before = lease.session().identity();
    let mut stream = lease.session().messages();

    let item = loop {
        let next = stream
            .next_inbound()
            .await
            .expect("a lifecycle item arrives");
        if !matches!(next, RelayInbound::Disconnected { .. }) {
            break next;
        }
    };
    let RelayInbound::Reconnected { previous, identity } = item else {
        panic!("expected a reconnect item, got {item:?}");
    };

    assert_eq!(previous, before);
    assert_eq!(
        identity.generation,
        before.generation.checked_next().expect("successor exists")
    );
    assert_eq!(lease.session().identity(), identity);
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reconnect_exhaustion_is_an_item_not_a_silent_stop() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        // The relay closes the first generation and then stops listening, so
        // every reconnect attempt fails and the budget runs out.
        let (stream, _) = listener.accept().await.expect("first connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket
            .close(None)
            .await
            .expect("relay closes generation one");
    });

    let transport = WebSocketTransport::new();
    let mut request = request(key);
    request.reconnect_attempts = Some(frames(2));
    request.deadlines.establish = Duration::from_millis(200);
    let lease = transport
        .acquire_session(request)
        .await
        .expect("session opens");
    let mut stream = lease.session().messages();
    server.await.expect("server joins");

    let exhausted = loop {
        if let RelayInbound::ReconnectExhausted { attempts, .. } = stream
            .next_inbound()
            .await
            .expect("a lifecycle item arrives")
        {
            break attempts;
        }
    };
    assert_eq!(exhausted, 2);
}
