//! Standard WebSocket transport conformance evidence.

use std::num::NonZeroUsize;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::Transport;
use fava_transport_testkit::{
    require_disconnect, require_handoff_refusal, require_handoff_success, require_idempotent_close,
};
use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

use fava_transport_websocket::WebSocketTransport;

async fn listener() -> (TcpListener, RelaySessionKey) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener binds");
    let url = RelayUrl::parse(&format!("ws://{}", listener.local_addr().expect("address")))
        .expect("relay URL");
    (listener, RelaySessionKey::new(url, RelayAccess::public()))
}

#[tokio::test(flavor = "current_thread")]
async fn complete_text_frame_handoff_succeeds() {
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
    let transport = WebSocketTransport::default();
    let session = transport.open_session(key).await.expect("session opens");

    require_handoff_success(session.as_ref(), "[\"REQ\",\"one\",{}]".to_owned())
        .await
        .expect("handoff succeeds");
    let received = server.await.expect("server joins");
    assert_eq!(received.into_text().expect("text"), "[\"REQ\",\"one\",{}]");
}

#[tokio::test(flavor = "current_thread")]
async fn oversized_frame_is_definitely_not_handed_off() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let _socket = accept_async(stream).await.expect("WebSocket accepts");
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    });
    let transport = WebSocketTransport::bounded(NonZeroUsize::new(4).expect("non-zero"));
    let session = transport.open_session(key).await.expect("session opens");

    require_handoff_refusal(session.as_ref(), "too-large".to_owned())
        .await
        .expect("handoff is refused");
    session.close().await.expect("session closes");
    server.await.expect("server joins");
}

#[tokio::test(flavor = "current_thread")]
async fn remote_disconnect_is_reported_exactly() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        socket.close(None).await.expect("server closes");
    });
    let transport = WebSocketTransport::default();
    let session = transport.open_session(key).await.expect("session opens");

    require_disconnect(session.as_ref())
        .await
        .expect("disconnect is reported");
    server.await.expect("server joins");
}

#[tokio::test(flavor = "current_thread")]
async fn close_is_idempotent_and_refuses_later_handoff() {
    let (listener, key) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("connection");
        let mut socket = accept_async(stream).await.expect("WebSocket accepts");
        while socket.next().await.is_some() {}
    });
    let transport = WebSocketTransport::default();
    let session = transport.open_session(key).await.expect("session opens");

    require_idempotent_close(session.as_ref())
        .await
        .expect("close conformance passes");
    server.await.expect("server joins");
}
