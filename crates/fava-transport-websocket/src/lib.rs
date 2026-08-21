//! WebSocket implementation of the Fava transport contract.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fava_state::RelaySessionKey;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async_with_config};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type SocketSink = SplitSink<Socket, Message>;
type SocketStream = SplitStream<Socket>;

/// WebSocket relay transport with one exact text-frame size bound.
///
/// The bound applies in both directions. An outbound frame over the bound is a
/// definite pre-handoff refusal; an inbound frame over the bound is refused by
/// the WebSocket layer and reported as an exact scoped invalid frame. A hostile
/// relay cannot make Fava buffer more than the declared bound.
pub struct WebSocketTransport {
    max_frame_bytes: NonZeroUsize,
    next_generation: AtomicU64,
}

impl Default for WebSocketTransport {
    fn default() -> Self {
        Self::bounded(NonZeroUsize::new(1_048_576).expect("constant is non-zero"))
    }
}

impl WebSocketTransport {
    /// Construct a WebSocket transport with one exact text-frame size bound.
    #[must_use]
    pub const fn bounded(max_frame_bytes: NonZeroUsize) -> Self {
        Self {
            max_frame_bytes,
            next_generation: AtomicU64::new(0),
        }
    }
}

impl Transport for WebSocketTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Arc<dyn RelaySession>, TransportError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let generation = self
                .next_generation
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                    current.checked_add(1)
                })
                .map_err(|_| {
                    TransportError::ConnectionRefused(
                        "relay session generation exhausted".to_owned(),
                    )
                })?
                + 1;
            let config = WebSocketConfig::default()
                .max_message_size(Some(self.max_frame_bytes.get()))
                .max_frame_size(Some(self.max_frame_bytes.get()));
            let (socket, _) = connect_async_with_config(key.relay.as_str(), Some(config), false)
                .await
                .map_err(|error| TransportError::ConnectionRefused(error.to_string()))?;
            let (sink, stream) = socket.split();
            Ok(Arc::new(WebSocketRelaySession {
                key,
                generation,
                max_frame_bytes: self.max_frame_bytes,
                closed: AtomicBool::new(false),
                sink: Mutex::new(sink),
                stream: Mutex::new(stream),
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct WebSocketRelaySession {
    key: RelaySessionKey,
    generation: u64,
    max_frame_bytes: NonZeroUsize,
    closed: AtomicBool,
    sink: Mutex<SocketSink>,
    stream: Mutex<SocketStream>,
}

impl RelaySession for WebSocketRelaySession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(
        &self,
        frame: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    reason: "relay session is closed".to_owned(),
                };
            }
            if frame.len() > self.max_frame_bytes.get() {
                return HandoffOutcome::NotHandedOff {
                    reason: format!(
                        "frame size {} exceeds bound {}",
                        frame.len(),
                        self.max_frame_bytes
                    ),
                };
            }
            match self
                .sink
                .lock()
                .await
                .send(Message::Text(frame.into()))
                .await
            {
                Ok(()) => HandoffOutcome::HandedOff,
                Err(error) => HandoffOutcome::Ambiguous {
                    reason: error.to_string(),
                },
            }
        })
    }

    fn next_message(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, TransportError>> + Send + '_>,
    > {
        Box::pin(async move {
            loop {
                if self.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed);
                }
                let message = self.stream.lock().await.next().await;
                match message {
                    Some(Ok(Message::Text(text))) => {
                        if text.len() > self.max_frame_bytes.get() {
                            return Err(TransportError::InvalidFrame(format!(
                                "inbound frame uses {} bytes but this transport allows {}",
                                text.len(),
                                self.max_frame_bytes
                            )));
                        }
                        return Ok(text.to_string());
                    }
                    Some(Ok(Message::Close(frame))) => {
                        self.closed.store(true, Ordering::SeqCst);
                        return Err(TransportError::Disconnected(format!("{frame:?}")));
                    }
                    Some(Ok(Message::Binary(_))) => {
                        return Err(TransportError::InvalidFrame(
                            "binary WebSocket frame".to_owned(),
                        ));
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                    Some(Err(error)) => {
                        self.closed.store(true, Ordering::SeqCst);
                        return Err(TransportError::Disconnected(error.to_string()));
                    }
                    None => {
                        self.closed.store(true, Ordering::SeqCst);
                        return Err(TransportError::Disconnected(
                            "WebSocket stream ended".to_owned(),
                        ));
                    }
                }
            }
        })
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            if self.closed.swap(true, Ordering::SeqCst) {
                return Ok(());
            }
            self.sink
                .lock()
                .await
                .close()
                .await
                .map_err(|error| TransportError::Disconnected(error.to_string()))
        })
    }
}
