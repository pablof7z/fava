//! Transparent WebSocket frame witness.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, watch};
use tokio::task::{JoinHandle, JoinSet};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async, connect_async};

use crate::artifacts::unix_ms;
use crate::{CanaryError, CanaryResult};

const CONNECTION_DRAIN_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) struct WireProxy {
    address: SocketAddr,
    stop: watch::Sender<bool>,
    inject: broadcast::Sender<String>,
    task: Option<JoinHandle<CanaryResult<()>>>,
}

impl WireProxy {
    pub(crate) async fn start(upstream: SocketAddr, path: &Path) -> CanaryResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let log = Arc::new(WireLog::new(path)?);
        let connection_sequence = Arc::new(AtomicU64::new(0));
        let (stop, mut stop_rx) = watch::channel(false);
        let (inject, _) = broadcast::channel(16);
        let task_inject = inject.clone();
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    changed = stop_rx.changed() => {
                        if changed.is_err() || *stop_rx.borrow_and_update() {
                            break;
                        }
                    }
                    accepted = listener.accept() => {
                        let (stream, _) = accepted?;
                        let connection = connection_sequence.fetch_add(1, Ordering::Relaxed) + 1;
                        let connection_log = Arc::clone(&log);
                        let connection_inject = task_inject.subscribe();
                        connections.spawn(async move {
                            let result = handle_connection(
                                stream,
                                upstream,
                                connection,
                                connection_log,
                                connection_inject,
                            ).await;
                            if let Err(error) = &result {
                                log_proxy_error(connection, &error);
                            }
                            result
                        });
                    }
                    Some(joined) = connections.join_next(), if !connections.is_empty() => {
                        joined.map_err(|error| CanaryError::new(format!("proxy connection task failed: {error}")))??;
                    }
                }
            }
            drain_connections(&mut connections).await?;
            Ok(())
        });
        Ok(Self {
            address,
            stop,
            inject,
            task: Some(task),
        })
    }

    pub(crate) fn url(&self) -> String {
        format!("ws://{}", self.address)
    }

    pub(crate) fn inject_relay_text(&self, payload: String) -> CanaryResult<()> {
        self.inject
            .send(payload)
            .map(|_| ())
            .map_err(|_| CanaryError::new("proxy has no active client connection"))
    }

    pub(crate) async fn shutdown(mut self) -> CanaryResult<()> {
        self.stop.send_replace(true);
        if let Some(task) = self.task.take() {
            task.await
                .map_err(|error| CanaryError::new(format!("proxy task failed: {error}")))??;
        }
        Ok(())
    }
}

async fn drain_connections(connections: &mut JoinSet<CanaryResult<()>>) -> CanaryResult<()> {
    let drain = async {
        while let Some(joined) = connections.join_next().await {
            joined.map_err(|error| {
                CanaryError::new(format!(
                    "proxy connection task failed during drain: {error}"
                ))
            })??;
        }
        Ok::<(), CanaryError>(())
    };
    if let Ok(result) = tokio::time::timeout(CONNECTION_DRAIN_DEADLINE, drain).await {
        result
    } else {
        connections.abort_all();
        while connections.join_next().await.is_some() {}
        Err(CanaryError::new("proxy connection drain deadline elapsed"))
    }
}

impl Drop for WireProxy {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

struct WireLog {
    file: Mutex<File>,
    sequence: AtomicU64,
}

impl WireLog {
    fn new(path: &Path) -> CanaryResult<Self> {
        let file = OpenOptions::new().create_new(true).write(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
            sequence: AtomicU64::new(0),
        })
    }

    fn record(&self, connection: u64, direction: &str, message: &Message) -> CanaryResult<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let (frame_type, payload) = describe(message);
        let entry = WireEntry {
            sequence,
            unix_ms: unix_ms()?,
            connection,
            direction,
            frame_type,
            payload,
        };
        let mut file = self
            .file
            .lock()
            .map_err(|_| CanaryError::new("wire log mutex poisoned"))?;
        serde_json::to_writer(&mut *file, &entry)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }
}

#[derive(Serialize)]
struct WireEntry<'a> {
    sequence: u64,
    unix_ms: u128,
    connection: u64,
    direction: &'a str,
    frame_type: &'a str,
    payload: String,
}

async fn handle_connection(
    downstream: TcpStream,
    upstream: SocketAddr,
    connection: u64,
    log: Arc<WireLog>,
    inject: broadcast::Receiver<String>,
) -> CanaryResult<()> {
    let downstream = accept_async(downstream).await?;
    let (upstream, _) = connect_async(format!("ws://{upstream}")).await?;
    bridge(downstream, upstream, connection, log, inject).await
}

async fn bridge(
    downstream: WebSocketStream<TcpStream>,
    upstream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    connection: u64,
    log: Arc<WireLog>,
    mut inject: broadcast::Receiver<String>,
) -> CanaryResult<()> {
    let (mut downstream_sink, mut downstream_stream) = downstream.split();
    let (mut upstream_sink, mut upstream_stream) = upstream.split();
    loop {
        tokio::select! {
            message = downstream_stream.next() => {
                let Some(message) = message else { break };
                let message = message?;
                log.record(connection, "client_to_relay", &message)?;
                let closes = message.is_close();
                upstream_sink.send(message).await?;
                if closes { break; }
            }
            message = upstream_stream.next() => {
                let Some(message) = message else { break };
                let message = message?;
                log.record(connection, "relay_to_client", &message)?;
                let closes = message.is_close();
                downstream_sink.send(message).await?;
                if closes { break; }
            }
            payload = inject.recv() => {
                match payload {
                    Ok(payload) => {
                        let message = Message::Text(payload.into());
                        log.record(connection, "proxy_to_client", &message)?;
                        downstream_sink.send(message).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

fn describe(message: &Message) -> (&'static str, String) {
    match message {
        Message::Text(text) => ("text", text.to_string()),
        Message::Binary(bytes) => ("binary", hex::encode(bytes)),
        Message::Ping(bytes) => ("ping", hex::encode(bytes)),
        Message::Pong(bytes) => ("pong", hex::encode(bytes)),
        Message::Close(frame) => ("close", format!("{frame:?}")),
        Message::Frame(frame) => ("frame", format!("{frame:?}")),
    }
}

fn log_proxy_error(connection: u64, error: &dyn std::error::Error) {
    eprintln!("proxy connection {connection} failed: {error}");
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use tokio::task::JoinSet;

    use super::drain_connections;

    #[tokio::test]
    async fn drain_finishes_completed_bridges_and_reaps_timed_out_bridges() {
        let mut completed = JoinSet::new();
        completed.spawn(async { Ok(()) });
        drain_connections(&mut completed)
            .await
            .expect("completed bridge drains");
        assert!(completed.is_empty());

        let mut stalled = JoinSet::new();
        stalled.spawn(pending::<crate::CanaryResult<()>>());
        let error = drain_connections(&mut stalled)
            .await
            .expect_err("stalled bridge is refused");
        assert_eq!(error.to_string(), "proxy connection drain deadline elapsed");
        assert!(stalled.is_empty());

        let mut failed = JoinSet::new();
        failed.spawn(async { Err(crate::CanaryError::new("capture write failed")) });
        let error = drain_connections(&mut failed)
            .await
            .expect_err("capture failure is propagated");
        assert_eq!(error.to_string(), "capture write failed");
        assert!(failed.is_empty());
    }
}
