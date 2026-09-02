//! Standard WebSocket transport conformance evidence, over real sockets.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_relay::Authority;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, ReleaseOutcome, Transport,
    TransportBounds, TransportDeadlines, TransportError, TransportFailure,
};
use fava_transport_testkit::{
    require_acquire_reuses_live_session, require_attributed_handoff,
    require_bounded_outbound_refusal,
};
use futures_util::{SinkExt, StreamExt};
use nostr::event::FinalizeEvent;
use nostr::filter::Filter;
use nostr::types::RelayUrl;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use fava_transport_websocket::WebSocketTransport;

fn frames(count: usize) -> NonZeroUsize {
    NonZeroUsize::new(count).expect("non-zero")
}

async fn listener() -> (TcpListener, RelayUrl) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener binds");
    let url = RelayUrl::parse(&format!("ws://{}", listener.local_addr().expect("address")))
        .expect("relay URL");
    (listener, url)
}

fn request(relay: RelayUrl) -> OpenRelaySession {
    OpenRelaySession {
        relay,
        authority: Authority::Unauthenticated,
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

/// Drain a session's connection stream until one state matches.
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_physical_session_fans_out_every_inbound_frame_to_every_consumer() {
    let (listener, key) = listener().await;
    // The session mints its own subscription ids, so the server cannot name one
    // in advance. It answers the first REQ it is sent, which is the owner's.
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        let first = loop {
            let message = socket.next().await.expect("client speaks").expect("frame");
            let Message::Text(text) = message else {
                continue;
            };
            let frame = nostr::message::ClientMessage::from_json(text.as_str())
                .expect("the client speaks NIP-01");
            if let nostr::message::ClientMessage::Req {
                subscription_id, ..
            } = frame
            {
                break subscription_id.into_owned();
            }
        };
        socket
            .send(Message::Text(
                nostr::message::RelayMessage::EndOfStoredEvents(std::borrow::Cow::Owned(first))
                    .as_json()
                    .into(),
            ))
            .await
            .expect("server sends");
        tokio::time::sleep(Duration::from_millis(300)).await;
    });

    let transport = WebSocketTransport::new();
    let lease = transport
        .acquire_session(request(key))
        .await
        .expect("session opens");
    let session = std::sync::Arc::clone(lease.session());
    let mut owner = fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
        .await
        .expect("subscription opens");
    let mut bystander = fava_transport::RelaySessionExt::subscribe(&session, vec![Filter::new()])
        .await
        .expect("second subscription opens");

    // The server echoes an EOSE naming whatever subscription it was sent.
    let item = owner.next().await;
    assert!(
        matches!(item, fava_transport::SubscriptionItem::EndOfStoredEvents),
        "the owning subscription did not see its own message, got {item:?}"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), bystander.next())
            .await
            .is_err(),
        "a subscription must never receive another subscription's message"
    );
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
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);

    let state = until(&mut connection, |state| {
        matches!(
            state.connectivity,
            fava_transport::Connectivity::Disconnected { .. }
        )
    })
    .await;
    assert!(matches!(
        state.connectivity,
        fava_transport::Connectivity::Disconnected { .. }
    ));
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
    assert_eq!(transport.holders(&key, &Authority::Unauthenticated), None);
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

    assert_eq!(transport.holders(&key, &Authority::Unauthenticated), None);
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
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);

    let state = until(&mut connection, |state| {
        matches!(state.connectivity, fava_transport::Connectivity::Connected)
            && state.identity != before
    })
    .await;
    let identity = state.identity;

    assert_eq!(
        identity.connection,
        before.connection.checked_next().expect("successor exists")
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
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);
    server.await.expect("server joins");

    // The only transition published for this generation is the exhausted
    // budget, `spent: Some(_)`: nothing is written while a reconnect is still
    // possible, so a handle waiting through the drop learns this reason
    // rather than a transient one written and superseded before the budget
    // was known to be spent.
    let state = until(&mut connection, |state| {
        matches!(
            state.connectivity,
            fava_transport::Connectivity::Disconnected { spent: Some(_), .. }
        )
    })
    .await;
    assert!(matches!(
        state.connectivity,
        fava_transport::Connectivity::Disconnected { spent: Some(_), .. }
    ));
    assert!(
        connection.changed().await.is_err(),
        "an exhausted budget ends the connection rather than leaving a reader waiting"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_exhausted_reconnect_budget_reaches_a_publisher_still_waiting() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        // The relay reads the EVENT so the handoff genuinely left, then drops
        // the connection without an `OK` and stops listening: every reconnect
        // attempt fails and the budget runs out while the acknowledgement is
        // still open.
        let (stream, _) = listener.accept().await.expect("first connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket
            .next()
            .await
            .expect("client sends the EVENT frame")
            .expect("frame reads");
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
    let session = std::sync::Arc::clone(lease.session());

    let keys = nostr::key::Keys::generate();
    let event = nostr::event::EventBuilder::new(nostr::event::Kind::TextNote, "gm")
        .finalize(&keys)
        .expect("event signs");
    let mut acknowledgement = fava_transport::RelaySessionExt::publish(&session, event)
        .await
        .expect("the EVENT hands off");

    server.await.expect("server joins");

    let settlement = acknowledgement.settled().await;
    assert!(
        matches!(
            settlement,
            fava_transport::Settlement::Ended(
                fava_transport::SessionEnded::ReconnectExhausted { .. }
            )
        ),
        "a publisher still waiting when the budget runs out should see the \
         exhausted budget, not the transient drop that preceded it: got {settlement:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lease_taken_before_a_reconnect_still_releases_its_entry() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        // The first generation is closed by the relay; the second is the one
        // the release has to close.
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
    let mut request = request(key.clone());
    request.reconnect_attempts = Some(frames(4));
    let authority = request.authority;
    let lease = transport
        .acquire_session(request)
        .await
        .expect("session opens");
    let before = lease.session().identity();
    let session = std::sync::Arc::clone(lease.session());
    let mut connection = fava_transport::RelaySessionExt::connection(&session);
    until(&mut connection, |state| {
        matches!(state.connectivity, fava_transport::Connectivity::Connected)
            && state.identity != before
    })
    .await;
    drop(connection);
    drop(session);

    assert_eq!(
        lease.release().await.expect("the lease finds its entry"),
        ReleaseOutcome::Closed
    );
    assert_eq!(transport.holders(&key, &authority), None);
    assert!(transport.sessions().is_empty());
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .expect("the relay sees the socket close")
        .expect("the server task ends");
}
