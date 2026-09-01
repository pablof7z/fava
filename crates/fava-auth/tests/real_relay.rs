//! Real-relay NIP-42 wire proof.
//!
//! `handshake.rs` proves the `Authenticator`'s challenge/response/verdict
//! logic against an in-process fake relay. This test proves the exact same
//! owner against a real third-party relay implementation -- `nostr-rs-relay`
//! 0.8.12, configured with `nip42_auth = true` -- reached over the real
//! `fava-transport-websocket` transport. Fava's own connection is routed
//! through a small transparent local WebSocket proxy so this test can assert
//! the relay's `AUTH` challenge and Fava's kind-22242 response as exact wire
//! frames, not just as `Authenticator::state` transitions.
//!
//! `nostr-rs-relay` 0.8.12 sends an explicit `OK false "restricted: ..."`
//! reply to a *malformed* AUTH event (verified by hand against this exact
//! binary), but sends no reply at all -- valid or otherwise -- to a
//! correctly signed one; it simply starts treating the connection as
//! authenticated. So `Authenticator::state` for this session settles at
//! `Attempted` and stays there: there is no verdict frame for a passing test
//! to wait for. This test proves what the wire actually carries -- the real
//! challenge and Fava's real signed response -- and says plainly that this
//! relay implementation gives no acknowledgement to assert past that. Never
//! call that permissiveness protocol proof of relay-side enforcement: it
//! only proves the challenge/response leg. The enforcing case -- a relay
//! that visibly demands, accepts, and sometimes still refuses -- is proved
//! separately by the harness-owned relay in
//! `examples/crates/e2e-support/live/nip42_relay.py`, exercised end to end by
//! `examples/relay-auth/live/harness.py`.
//!
//! Ignored by default so `cargo test --workspace` stays green without the
//! external binary. Run explicitly with:
//!
//! ```sh
//! cargo test -p fava-auth --test real_relay -- --ignored --nocapture
//! ```
//!
//! Requires `nostr-rs-relay` on PATH or at `~/.cargo/bin/nostr-rs-relay`.
//! Override with the `RELAY_BIN` env var.

use std::net::TcpListener as StdTcpListener;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy, Authenticator,
};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::Session;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_transport_websocket::WebSocketTransport;
use futures_util::{SinkExt, StreamExt};
use nostr::key::Keys;
use nostr::types::RelayUrl;
use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;

const READY_TIMEOUT: Duration = Duration::from_secs(10);
const FRAME_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn nostr_rs_relay_challenge_and_kind_22242_response_are_exact_wire_frames() {
    let workdir = TempDir::new();
    let relay = start_relay(workdir.path()).await;
    let (proxy_url, transcript) = start_proxy(relay.url.clone()).await;

    let keys = Keys::generate();
    let account = keys.public_key();
    let signers = Session::new([Arc::new(LocalSigner::new(keys)) as Arc<dyn Signer>])
        .expect("one signer attaches");
    let transport: Arc<dyn fava_transport::Transport> = Arc::new(WebSocketTransport::new());
    let runtime = Runtime::new(RuntimeConfig {
        default_channel_depth: nonzero(1_024),
        max_tasks: nonzero(1_024),
        max_provider_operations: nonzero(256),
    });
    let authenticator = Authenticator::new(signers, Arc::new(AlwaysAuthenticate), runtime);
    authenticator
        .answer_requests(transport.as_ref())
        .expect("the owner begins answering");
    let key = RelaySessionKey {
        relay: RelayUrl::parse(&proxy_url).expect("proxy url parses"),
        access: RelayAccess::Authenticated(account),
    };
    let lease = connect(transport.as_ref(), key.clone()).await;

    wait_for_round_trip(&transcript, &lease).await;

    let frames = transcript
        .lock()
        .expect("transcript lock is not poisoned")
        .clone();

    let challenge_text = frames
        .iter()
        .filter(|(direction, _)| *direction == Direction::RelayToClient)
        .find_map(|(_, text)| {
            let frame: Value = serde_json::from_str(text).ok()?;
            (frame.get(0).and_then(Value::as_str) == Some("AUTH"))
                .then(|| frame.get(1).and_then(Value::as_str).map(str::to_owned))
                .flatten()
        })
        .expect("nostr-rs-relay sent an AUTH challenge frame on Fava's own connection");
    assert!(
        !challenge_text.is_empty(),
        "challenge text must not be empty"
    );

    let response = frames
        .iter()
        .filter(|(direction, _)| *direction == Direction::ClientToRelay)
        .find_map(|(_, text)| {
            let frame: Value = serde_json::from_str(text).ok()?;
            (frame.get(0).and_then(Value::as_str) == Some("AUTH")).then_some(frame)
        })
        .expect("Fava sent an AUTH response frame on its own connection");
    let event = response.get(1).expect("AUTH frame carries the event body");
    assert_eq!(event["kind"], 22242, "the response is exactly kind 22242");
    assert_eq!(
        event["pubkey"].as_str(),
        Some(account.to_hex().as_str()),
        "the response is signed by the authenticating account"
    );
    let tags = event["tags"].as_array().expect("tags is an array");
    let challenge_tag = tags
        .iter()
        .find(|tag| tag.get(0).and_then(Value::as_str) == Some("challenge"))
        .expect("response carries a challenge tag");
    assert_eq!(
        challenge_tag.get(1).and_then(Value::as_str),
        Some(challenge_text.as_str()),
        "the challenge is echoed byte-exact"
    );
    let relay_tag = tags
        .iter()
        .find(|tag| tag.get(0).and_then(Value::as_str) == Some("relay"))
        .expect("response carries a relay tag");
    assert_eq!(
        relay_tag.get(1).and_then(Value::as_str),
        Some(proxy_url.as_str()),
        "the relay tag names the exact session Fava connected to"
    );

    // Fava genuinely attempted this exact real handshake: the response was
    // handed to the transport. `Authenticating` is where this stays -- see the
    // module doc for why `nostr-rs-relay` never rules on it.
    assert!(
        matches!(
            fava_transport::RelaySessionExt::connection(lease.session())
                .borrow()
                .authentication,
            fava_relay::Authentication::Authenticating { .. }
        ),
        "the connection records the real attempt"
    );

    let mut relay_process = relay.child;
    let _ = relay_process.kill().await;
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum Direction {
    ClientToRelay,
    RelayToClient,
}

type Transcript = Arc<Mutex<Vec<(Direction, String)>>>;

fn has_two_frames(transcript: &Transcript) -> bool {
    let frames = transcript.lock().expect("transcript lock is not poisoned");
    let challenge = frames.iter().any(|(direction, text)| {
        *direction == Direction::RelayToClient
            && serde_json::from_str::<Value>(text)
                .is_ok_and(|frame| frame.get(0).and_then(Value::as_str) == Some("AUTH"))
    });
    let response = frames.iter().any(|(direction, text)| {
        *direction == Direction::ClientToRelay
            && serde_json::from_str::<Value>(text)
                .is_ok_and(|frame| frame.get(0).and_then(Value::as_str) == Some("AUTH"))
    });
    challenge && response
}

struct AlwaysAuthenticate;

impl AuthenticationPolicy for AlwaysAuthenticate {
    fn decide(&self, _demand: &AuthenticationDemand) -> AuthenticationDecision {
        AuthenticationDecision::Authenticate
    }
}

fn nonzero_usize(value: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).expect("constant is non-zero")
}

/// Something has to want this relay. The owner answers connections other
/// components open; it opens none itself.
async fn connect(
    transport: &dyn fava_transport::Transport,
    key: RelaySessionKey,
) -> fava_transport::RelaySessionLease {
    fava_transport::Transport::acquire_session(
        transport,
        fava_transport::OpenRelaySession {
            key,
            deadlines: fava_transport::TransportDeadlines {
                establish: FRAME_TIMEOUT,
                write: FRAME_TIMEOUT,
                idle: FRAME_TIMEOUT,
                close: FRAME_TIMEOUT,
            },
            bounds: fava_transport::TransportBounds {
                inbound_frames: nonzero_usize(64),
                outbound_frames: nonzero_usize(4),
                max_frame_bytes: nonzero_usize(1_048_576),
            },
            reconnect_attempts: None,
        },
    )
    .await
    .expect("the proxied relay accepts a connection")
}

fn nonzero(value: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).expect("constant is non-zero")
}

struct RelayProcess {
    child: Child,
    url: String,
}

fn relay_bin() -> PathBuf {
    if let Ok(bin) = std::env::var("RELAY_BIN") {
        return PathBuf::from(bin);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(home).join(".cargo/bin/nostr-rs-relay")
}

fn free_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("read the bound address")
        .port()
}

async fn start_relay(dir: &std::path::Path) -> RelayProcess {
    let port = free_port();
    std::fs::create_dir_all(dir.join("data")).expect("create relay data directory");
    let config_path = dir.join("config.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"[info]
relay_url = "ws://127.0.0.1:{port}/"
name = "fava-auth real-relay NIP-42 proof"

[database]
engine = "sqlite"
in_memory = false

[network]
address = "127.0.0.1"
port = {port}
ping_interval = 30

[options]
reject_future_seconds = 1800

[limits]
max_event_bytes = 131072
max_ws_message_bytes = 131072
max_ws_frame_bytes = 131072
broadcast_buffer = 1024
event_persist_buffer = 1024

[authorization]
nip42_auth = true
"#
        ),
    )
    .expect("write relay config");

    let bin = relay_bin();
    assert!(
        bin.exists(),
        "nostr-rs-relay not found at {}; set RELAY_BIN env var or install it",
        bin.display()
    );

    let child = Command::new(&bin)
        .arg("--config")
        .arg(&config_path)
        .arg("--db")
        .arg(dir.join("data"))
        .env("RUST_LOG", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn nostr-rs-relay");

    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if TcpStream::connect(&addr).await.is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "relay readiness timeout");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    RelayProcess {
        child,
        url: format!("ws://127.0.0.1:{port}"),
    }
}

/// A one-shot transparent WebSocket proxy in front of the real relay,
/// recording every text frame exchanged on Fava's own connection.
///
/// Fava never learns it is not talking directly to `nostr-rs-relay`: every
/// frame is forwarded unchanged in both directions, and only cloned into the
/// bounded in-memory transcript this test reads afterward.
async fn start_proxy(upstream: String) -> (String, Transcript) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind the proxy listener");
    let port = listener.local_addr().expect("read proxy address").port();
    let transcript: Transcript = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&transcript);
    tokio::spawn(async move {
        let Ok(Ok((stream, _))) = timeout(READY_TIMEOUT, listener.accept()).await else {
            return;
        };
        let Ok(client_ws) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };
        let Ok((upstream_ws, _)) = tokio_tungstenite::connect_async(&upstream).await else {
            return;
        };
        let (mut client_write, mut client_read) = client_ws.split();
        let (mut upstream_write, mut upstream_read) = upstream_ws.split();

        let to_upstream = {
            let recorded = Arc::clone(&recorded);
            async move {
                while let Some(Ok(message)) = client_read.next().await {
                    if let Message::Text(text) = &message {
                        recorded
                            .lock()
                            .expect("transcript lock is not poisoned")
                            .push((Direction::ClientToRelay, text.to_string()));
                    }
                    if upstream_write.send(message).await.is_err() {
                        break;
                    }
                }
            }
        };
        let to_client = {
            let recorded = Arc::clone(&recorded);
            async move {
                while let Some(Ok(message)) = upstream_read.next().await {
                    if let Message::Text(text) = &message {
                        recorded
                            .lock()
                            .expect("transcript lock is not poisoned")
                            .push((Direction::RelayToClient, text.to_string()));
                    }
                    if client_write.send(message).await.is_err() {
                        break;
                    }
                }
            }
        };
        tokio::join!(to_upstream, to_client);
    });
    (format!("ws://127.0.0.1:{port}"), transcript)
}

/// A tiny disposable-directory helper so this test needs no extra crate.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "fava-auth-real-relay-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock is after the epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("create disposable test directory");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Wait for the real wire round trip: the challenge Fava received, and the
/// response Fava sent for it. A bounded wait on a real condition, not a sleep.
async fn wait_for_round_trip(transcript: &Transcript, lease: &fava_transport::RelaySessionLease) {
    let deadline = Instant::now() + FRAME_TIMEOUT;
    loop {
        if has_two_frames(transcript) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the challenge/response wire round trip did not complete before the deadline; \
             transcript so far: {:?}; connection state: {:?}",
            transcript
                .lock()
                .expect("transcript lock is not poisoned")
                .clone(),
            fava_transport::RelaySessionExt::connection(lease.session())
                .borrow()
                .authentication
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
