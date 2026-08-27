//! The task that owns one relay socket, its four deadlines, and its reconnects.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use fava_transport::{
    BoundedReason, HandoffCorrelation, HandoffOutcome, RelayInbound, RelaySessionIdentity,
    TransportAmbiguity, TransportFailure,
};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use nostr::types::Timestamp;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::backoff::ReconnectBackoff;
use crate::mint_generation;
use crate::session::SessionShared;

pub(crate) type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Sink = SplitSink<Socket, Message>;
type Source = SplitStream<Socket>;

/// One frame handed to the driver, with the caller's correlation and the
/// return path for its completion.
pub(crate) struct Outbound {
    pub(crate) frame: Vec<u8>,
    pub(crate) correlation: HandoffCorrelation,
    pub(crate) completion: oneshot::Sender<HandoffOutcome>,
}

/// Establish one socket under the Fava-owned establishment deadline.
pub(crate) async fn establish(shared: &SessionShared) -> Result<Socket, TransportFailure> {
    let url = shared.identity.key().relay.as_str().to_owned();
    let deadline = shared.deadlines.establish;
    match tokio::time::timeout(deadline, connect_async(url)).await {
        Err(_) => Err(TransportFailure::EstablishTimeout { after: deadline }),
        Ok(Err(error)) => Err(TransportFailure::Disconnected {
            detail: BoundedReason::new(error.to_string()),
        }),
        Ok(Ok((socket, _))) => Ok(socket),
    }
}

/// Drive one session across its generations until it is closed or its
/// reconnect budget is exhausted.
pub(crate) async fn drive(
    shared: Arc<SessionShared>,
    mut outbound: mpsc::Receiver<Outbound>,
    first: Socket,
) {
    let mut socket = first;
    let mut backoff = ReconnectBackoff::new(shared.entropy);
    loop {
        let (sink, source) = socket.split();
        let reason = pump(&shared, &mut outbound, sink, source).await;
        drain_unsent(&mut outbound, &shared, &reason);
        shared.consumers.fan_out(&RelayInbound::Disconnected {
            identity: shared.identity.read(),
            reason: reason.clone(),
        });
        if shared.closed.load(Ordering::SeqCst) {
            break;
        }
        match reconnect(&shared, &mut backoff).await {
            Ok((next, generation)) => {
                socket = next;
                backoff.reset();
                let (previous, identity) = shared.identity.advance(generation);
                shared
                    .consumers
                    .fan_out(&RelayInbound::Reconnected { previous, identity });
            }
            Err((attempts, final_reason)) => {
                shared.closed.store(true, Ordering::SeqCst);
                shared.consumers.fan_out(&RelayInbound::ReconnectExhausted {
                    identity: shared.identity.read(),
                    attempts,
                    reason: final_reason,
                });
                break;
            }
        }
    }
    shared.closed.store(true, Ordering::SeqCst);
    shared.consumers.detach_all();
    shared.close_finished.notify_waiters();
}

/// Retry establishment under the caller's attempt budget and this crate's
/// backoff. `None` means retry until every lease is released.
async fn reconnect(
    shared: &SessionShared,
    backoff: &mut ReconnectBackoff,
) -> Result<(Socket, fava_transport::RelaySessionGeneration), (usize, TransportFailure)> {
    let Some(generation) = mint_generation(&shared.generations) else {
        return Err((0, TransportFailure::GenerationExhausted));
    };
    let budget = shared
        .reconnect_attempts
        .map_or(usize::MAX, std::num::NonZeroUsize::get);
    let mut last = TransportFailure::SessionClosed;
    for attempt in 1..=budget {
        tokio::time::sleep(backoff.next_delay()).await;
        if shared.closed.load(Ordering::SeqCst) {
            return Err((attempt, TransportFailure::SessionClosed));
        }
        match establish(shared).await {
            Ok(socket) => return Ok((socket, generation)),
            Err(reason) => last = reason,
        }
    }
    Err((budget, last))
}

/// Answer every frame still queued behind a dead socket. These definitely did
/// not leave Fava, so they are refusals, not ambiguities.
fn drain_unsent(
    outbound: &mut mpsc::Receiver<Outbound>,
    shared: &SessionShared,
    reason: &TransportFailure,
) {
    let identity = shared.identity.read();
    while let Ok(pending) = outbound.try_recv() {
        let _ = pending.completion.send(HandoffOutcome::NotHandedOff {
            identity: identity.clone(),
            correlation: pending.correlation,
            reason: reason.clone(),
        });
    }
}

/// Run one generation until it ends. The return value is why it ended.
async fn pump(
    shared: &SessionShared,
    outbound: &mut mpsc::Receiver<Outbound>,
    mut sink: Sink,
    mut source: Source,
) -> TransportFailure {
    let idle = shared.deadlines.idle;
    let mut last_inbound = Instant::now();
    let mut keepalive = tokio::time::interval(idle / 2);
    keepalive.tick().await;
    loop {
        let remaining = idle.saturating_sub(last_inbound.elapsed());
        if remaining.is_zero() {
            return TransportFailure::IdleTimeout { after: idle };
        }
        tokio::select! {
            () = shared.close_requested.notified() => {
                return close_gracefully(shared, &mut sink).await;
            }
            pending = outbound.recv() => {
                let Some(pending) = pending else {
                    return close_gracefully(shared, &mut sink).await;
                };
                if let Some(failure) = write_frame(shared, &mut sink, pending).await {
                    return failure;
                }
            }
            _ = keepalive.tick() => {
                if sink.send(Message::Ping(Vec::new().into())).await.is_err() {
                    return TransportFailure::Disconnected {
                        detail: BoundedReason::new("keepalive probe could not be written"),
                    };
                }
            }
            inbound = tokio::time::timeout(remaining, source.next()) => {
                let Ok(message) = inbound else {
                    return TransportFailure::IdleTimeout { after: idle };
                };
                last_inbound = Instant::now();
                if let Some(failure) = admit(shared, message) {
                    return failure;
                }
            }
        }
    }
}

/// Write one complete frame under the Fava-owned write deadline and answer its
/// caller with the exact completion.
async fn write_frame(
    shared: &SessionShared,
    sink: &mut Sink,
    pending: Outbound,
) -> Option<TransportFailure> {
    let identity = shared.identity.read();
    let correlation = pending.correlation;
    let deadline = shared.deadlines.write;
    let text = String::from_utf8_lossy(&pending.frame).into_owned();
    let written = tokio::time::timeout(deadline, sink.send(Message::Text(text.into()))).await;
    let (outcome, failure) = match written {
        Ok(Ok(())) => (
            HandoffOutcome::HandedOff {
                identity,
                correlation,
            },
            None,
        ),
        Ok(Err(error)) => {
            let detail = BoundedReason::new(error.to_string());
            (
                HandoffOutcome::Ambiguous {
                    identity,
                    correlation,
                    reason: TransportAmbiguity::FlushUnconfirmed {
                        detail: detail.clone(),
                    },
                },
                Some(TransportFailure::Disconnected { detail }),
            )
        }
        Err(_) => (
            HandoffOutcome::Ambiguous {
                identity,
                correlation,
                reason: TransportAmbiguity::WriteTimeout { after: deadline },
            },
            Some(TransportFailure::Disconnected {
                detail: BoundedReason::new("write deadline expired with bytes in the socket"),
            }),
        ),
    };
    let _ = pending.completion.send(outcome);
    failure
}

/// Admit one inbound WebSocket message. Ping and Pong are liveness, not data:
/// they refresh the idle deadline and are never handed to a consumer.
fn admit(
    shared: &SessionShared,
    message: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> Option<TransportFailure> {
    let identity = shared.identity.read();
    match message {
        Some(Ok(Message::Text(text))) => admit_frame(shared, &identity, text.as_bytes().to_vec()),
        Some(Ok(Message::Binary(bytes))) => admit_frame(shared, &identity, bytes.to_vec()),
        Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => None,
        Some(Ok(Message::Close(frame))) => Some(TransportFailure::Disconnected {
            detail: BoundedReason::new(frame.map_or_else(
                || "relay closed the session".to_owned(),
                |frame| frame.reason.to_string(),
            )),
        }),
        Some(Err(error)) => Some(TransportFailure::Disconnected {
            detail: BoundedReason::new(error.to_string()),
        }),
        None => Some(TransportFailure::Disconnected {
            detail: BoundedReason::new("relay ended the stream"),
        }),
    }
}

fn admit_frame(
    shared: &SessionShared,
    identity: &RelaySessionIdentity,
    frame: Vec<u8>,
) -> Option<TransportFailure> {
    let maximum = shared.bounds.max_frame_bytes.get();
    if frame.len() > maximum {
        return Some(TransportFailure::Disconnected {
            detail: BoundedReason::new(format!(
                "relay sent {} bytes, exceeding the declared bound {maximum}",
                frame.len()
            )),
        });
    }
    shared.consumers.fan_out(&RelayInbound::Frame {
        identity: identity.clone(),
        frame,
        received_at: Timestamp::now(),
    });
    None
}

/// Close the handshake under the Fava-owned close deadline. Afterwards the
/// session is reported closed regardless of what the peer does.
async fn close_gracefully(shared: &SessionShared, sink: &mut Sink) -> TransportFailure {
    shared.closed.store(true, Ordering::SeqCst);
    let deadline = shared.deadlines.close;
    if tokio::time::timeout(deadline, sink.close()).await.is_err() {
        return TransportFailure::Disconnected {
            detail: BoundedReason::new("close deadline expired; session dropped"),
        };
    }
    TransportFailure::SessionClosed
}
