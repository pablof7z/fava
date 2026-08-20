//! Hostile relay witnesses for admission-path falsifiers.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelayUrl, Timestamp};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use futures_util::{SinkExt, StreamExt};
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::message::{ClientMessage, RelayMessage};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use crate::{CanaryError, CanaryResult};

pub(crate) async fn refuse_forged_event(seed: &str) -> CanaryResult<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let relay = RelayUrl::parse(&format!("ws://{}", listener.local_addr()?)).map_err(error)?;
    let keys = crate::deterministic_keys(&format!("hostile\0{seed}"))?;
    let mut forged = EventBuilder::new(Kind::TextNote, "signed body")
        .custom_created_at(Timestamp::now())
        .finalize(&keys)
        .map_err(error)?;
    "forged after signing".clone_into(&mut forged.content);
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let mut socket = accept_async(stream).await?;
        let request = next_text(&mut socket).await?;
        let subscription = match serde_json::from_str::<ClientMessage<'static>>(&request)? {
            ClientMessage::Req {
                subscription_id, ..
            } => subscription_id.into_owned(),
            message => {
                return Err(CanaryError::new(format!(
                    "hostile witness expected REQ, got {message:?}"
                )));
            }
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&RelayMessage::event(subscription.clone(), forged))?.into(),
            ))
            .await?;
        socket
            .send(Message::Text(
                serde_json::to_string(&RelayMessage::eose(subscription))?.into(),
            ))
            .await?;
        while socket.next().await.is_some() {}
        Ok::<_, CanaryError>(())
    });

    let cache = Arc::new(MemoryEventCache::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .build()
        .map_err(error)?;
    let query = Query::events()
        .authors([keys.public_key()])
        .only_from_relays([relay])
        .map_err(error)?;
    let observation = fava.observe(query).await.map_err(error)?;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let diagnostics = fava.diagnostics();
            if !diagnostics.failures.is_empty() && !diagnostics.eose.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("forged-event refusal was not diagnosed"))?;
    if !observation.current().events.is_empty() || !cache.is_empty().map_err(error)? {
        return Err(CanaryError::new(
            "forged relay event became application-visible",
        ));
    }
    observation.close();
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .map_err(|_| CanaryError::new("hostile relay did not close"))?
        .map_err(|join| CanaryError::new(format!("hostile relay task failed: {join}")))??;
    Ok(())
}

async fn next_text<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> CanaryResult<String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    loop {
        let message = socket
            .next()
            .await
            .ok_or_else(|| CanaryError::new("hostile witness socket closed"))??;
        if let Message::Text(text) = message {
            return Ok(text.to_string());
        }
    }
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
