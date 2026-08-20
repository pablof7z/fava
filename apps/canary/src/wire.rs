//! Independent NIP-01 seeder and witness.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::event::{Event, EventId};
use nostr::filter::Filter;
use nostr::message::{ClientMessage, SubscriptionId};
use serde::Serialize;
use serde_json::Value;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use crate::{CanaryError, CanaryResult};

#[derive(Clone, Copy, Debug, Serialize)]
pub(crate) struct QueryWitness {
    pub(crate) found_event: bool,
    pub(crate) saw_eose: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReconWitness {
    pub(crate) frames: Vec<Value>,
    pub(crate) terminal: &'static str,
}

pub(crate) async fn publish(url: &str, event: &Event) -> CanaryResult<String> {
    let mut socket = connect(url).await?;
    send(&mut socket, ClientMessage::event(event.clone())).await?;
    loop {
        let value = next_json(&mut socket).await?;
        let Some(kind) = value.get(0).and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "OK" => {
                let id = value.get(1).and_then(Value::as_str).unwrap_or_default();
                if id != event.id.to_hex() {
                    continue;
                }
                let accepted = value.get(2).and_then(Value::as_bool).unwrap_or(false);
                let message = value.get(3).and_then(Value::as_str).unwrap_or_default();
                if !accepted {
                    return Err(CanaryError::new(format!(
                        "relay rejected event {}: {message}",
                        event.id
                    )));
                }
                socket.close(None).await?;
                return Ok(message.to_owned());
            }
            "NOTICE" | "CLOSED" => {
                return Err(CanaryError::new(format!("relay refused publish: {value}")));
            }
            _ => {}
        }
    }
}

pub(crate) async fn query_exact(
    url: &str,
    event_id: EventId,
    subscription: &str,
) -> CanaryResult<QueryWitness> {
    let mut socket = connect(url).await?;
    let subscription_id = SubscriptionId::new(subscription);
    send(
        &mut socket,
        ClientMessage::req(subscription_id.clone(), Filter::new().id(event_id)),
    )
    .await?;
    let mut found_event = false;
    loop {
        let value = next_json(&mut socket).await?;
        let Some(kind) = value.get(0).and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "EVENT" if value.get(1).and_then(Value::as_str) == Some(subscription) => {
                let event_value = value
                    .get(2)
                    .cloned()
                    .ok_or_else(|| CanaryError::new("EVENT frame omitted event body"))?;
                let event: Event = serde_json::from_value(event_value)?;
                event.verify()?;
                if event.id == event_id {
                    found_event = true;
                }
            }
            "EOSE" if value.get(1).and_then(Value::as_str) == Some(subscription) => {
                send(&mut socket, ClientMessage::close(subscription_id)).await?;
                socket.close(None).await?;
                return Ok(QueryWitness {
                    found_event,
                    saw_eose: true,
                });
            }
            "CLOSED" if value.get(1).and_then(Value::as_str) == Some(subscription) => {
                return Err(CanaryError::new(format!(
                    "relay closed exact query before EOSE: {value}"
                )));
            }
            _ => {}
        }
    }
}

pub(crate) async fn reconnaissance(url: &str, subscription: &str) -> CanaryResult<ReconWitness> {
    const FRAME_LIMIT: usize = 64;
    let mut socket = connect(url).await?;
    let subscription_id = SubscriptionId::new(subscription);
    send(
        &mut socket,
        ClientMessage::req(subscription_id.clone(), Filter::new().limit(1)),
    )
    .await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut frames = Vec::new();
    let terminal = loop {
        if frames.len() == FRAME_LIMIT {
            break "frame-limit";
        }
        let value = match tokio::time::timeout_at(deadline, next_json(&mut socket)).await {
            Ok(result) => result?,
            Err(_) => break "deadline",
        };
        let kind = value
            .get(0)
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        if kind == "EVENT"
            && let Some(event_value) = value.get(2).cloned()
        {
            let event: Event = serde_json::from_value(event_value)?;
            event.verify()?;
        }
        frames.push(value);
        if kind == "EOSE" {
            break "eose";
        }
        if matches!(kind.as_str(), "CLOSED" | "NOTICE") {
            break "relay-terminal";
        }
    };
    send(&mut socket, ClientMessage::close(subscription_id)).await?;
    socket.close(None).await?;
    Ok(ReconWitness { frames, terminal })
}

async fn send<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    message: ClientMessage<'_>,
) -> CanaryResult<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Text(serde_json::to_string(&message)?.into()))
        .await?;
    Ok(())
}

async fn connect(
    url: &str,
) -> CanaryResult<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
    let (socket, _) = timeout(Duration::from_secs(10), connect_async(url))
        .await
        .map_err(|_| CanaryError::new("relay connection deadline elapsed"))??;
    Ok(socket)
}

async fn next_json<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> CanaryResult<Value>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let message = timeout(Duration::from_secs(5), socket.next())
            .await
            .map_err(|_| CanaryError::new("relay frame deadline elapsed"))?
            .ok_or_else(|| CanaryError::new("relay closed WebSocket unexpectedly"))??;
        match message {
            Message::Text(text) => return Ok(serde_json::from_str(text.as_str())?),
            Message::Binary(bytes) => return Ok(serde_json::from_slice(&bytes)?),
            Message::Close(frame) => {
                return Err(CanaryError::new(format!(
                    "relay closed WebSocket unexpectedly: {frame:?}"
                )));
            }
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
}
