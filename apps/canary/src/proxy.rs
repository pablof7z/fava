//! Transparent WebSocket frame witness.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, accept_async, connect_async};

use crate::artifacts::unix_ms;
use crate::{CanaryError, CanaryResult};

pub(crate) struct WireProxy {
    address: SocketAddr,
    stop: watch::Sender<bool>,
    task: Option<JoinHandle<CanaryResult<()>>>,
}

impl WireProxy {
    pub(crate) async fn start(upstream: SocketAddr, path: &Path) -> CanaryResult<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let log = Arc::new(WireLog::new(path)?);
        let connection_sequence = Arc::new(AtomicU64::new(0));
        let (stop, mut stop_rx) = watch::channel(false);
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
                        connections.spawn(async move {
                            if let Err(error) = handle_connection(stream, upstream, connection, connection_log).await {
                                log_proxy_error(connection, &error);
                            }
                        });
                    }
                    Some(joined) = connections.join_next(), if !connections.is_empty() => {
                        joined.map_err(|error| CanaryError::new(format!("proxy connection task failed: {error}")))?;
                    }
                }
            }
            connections.abort_all();
            while connections.join_next().await.is_some() {}
            Ok(())
        });
        Ok(Self {
            address,
            stop,
            task: Some(task),
        })
    }

    pub(crate) fn url(&self) -> String {
        format!("ws://{}", self.address)
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
) -> CanaryResult<()> {
    let downstream = accept_async(downstream).await?;
    let (upstream, _) = connect_async(format!("ws://{upstream}")).await?;
    bridge(downstream, upstream, connection, log).await
}

async fn bridge(
    downstream: WebSocketStream<TcpStream>,
    upstream: WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
    connection: u64,
    log: Arc<WireLog>,
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
