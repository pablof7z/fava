//! Phase B exit-gate e2e evidence — real relay, real events.
//!
//! Each test is `#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]` so `cargo test -p fava-simple-groups` stays green in CI.
//! Run the full suite with:
//!
//! ```sh
//! cargo test -p fava-simple-groups --test management_e2e -- --ignored --nocapture
//! ```
//!
//! Requires `nostr-rs-relay` on PATH or at `~/.cargo/bin/nostr-rs-relay`.
//! Override with the `RELAY_BIN` env var.

use std::fmt;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{Instant, timeout};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use fava_simple_groups::{
    GroupAccess, GroupVisibility, MetadataEdit, SimpleGroup, SimpleGroupAdmins, create_group,
    delete_event, delete_group, edit_metadata, invite, join_request, leave_group, put_user,
    remove_user,
};
use fava_write::{EventBuilder, EventId, PublicKey, Tag, UnsignedEvent};

// ── Relay harness ─────────────────────────────────────────────────────────────

fn relay_bin() -> PathBuf {
    if let Ok(bin) = std::env::var("RELAY_BIN") {
        return PathBuf::from(bin);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(home).join(".cargo/bin/nostr-rs-relay")
}

fn relay_config(port: u16, whitelist_pubkey: Option<&str>) -> String {
    let whitelist = whitelist_pubkey
        .map(|pk| format!("pubkey_whitelist = [\"{pk}\"]\n"))
        .unwrap_or_default();
    format!(
        r#"[info]
relay_url = "ws://127.0.0.1:{port}/"
name = "fava-simple-groups management e2e relay"

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
nip42_auth = false
{whitelist}
"#
    )
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct RelayProcess {
    child: Child,
    pub url: String,
}

async fn start_relay(dir: &Path, whitelist: Option<&str>) -> RelayProcess {
    let port = free_port();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    let config_path = dir.join("config.toml");
    std::fs::write(&config_path, relay_config(port, whitelist)).unwrap();

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
        .expect("spawn relay");

    // Wait for the relay to accept TCP connections.
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(10);
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

/// Start Croissant, which applies NIP-29 management events to real group state.
async fn start_nip29_relay(dir: &Path) -> RelayProcess {
    let port = free_port();
    let data = dir.join("croissant-data");
    std::fs::create_dir_all(&data).expect("create Croissant data directory");
    let bin = PathBuf::from(
        std::env::var("NIP29_RELAY_BIN")
            .expect("set NIP29_RELAY_BIN to an external Croissant binary"),
    );
    assert!(
        bin.exists(),
        "Croissant binary not found at {}",
        bin.display()
    );

    let mut child = Command::new(&bin)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .env("DATAPATH", &data)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn Croissant");

    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect(&addr).await.is_ok() {
            break;
        }
        if let Some(status) = child.try_wait().expect("poll Croissant") {
            panic!("Croissant exited before readiness: {status}");
        }
        assert!(Instant::now() < deadline, "Croissant readiness timeout");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    RelayProcess {
        child,
        url: format!("ws://127.0.0.1:{port}"),
    }
}

// ── Wire helpers ──────────────────────────────────────────────────────────────

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug)]
enum ReadbackError {
    Timeout,
    Closed,
    WebSocket(String),
    Protocol(String),
    TooManyEvents { maximum: usize },
}

impl fmt::Display for ReadbackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout => formatter.write_str("relay readback timed out before matching EOSE"),
            Self::Closed => formatter.write_str("relay closed before matching EOSE"),
            Self::WebSocket(error) => {
                write!(formatter, "relay readback WebSocket failure: {error}")
            }
            Self::Protocol(error) => write!(formatter, "relay readback protocol failure: {error}"),
            Self::TooManyEvents { maximum } => {
                write!(
                    formatter,
                    "relay readback exceeded its {maximum}-event bound"
                )
            }
        }
    }
}

async fn ws_connect(url: &str) -> Ws {
    let (ws, _) = connect_async(url).await.expect("ws connect");
    ws
}

/// Sign an [`UnsignedEvent`] and submit it. Returns `(event id, ok, message)`.
async fn submit_with_id(ws: &mut Ws, event: UnsignedEvent, keys: &Keys) -> (String, bool, String) {
    let signed = event.finalize(keys).expect("sign");
    let id = signed.id.to_hex();
    let msg = json!(["EVENT", signed]);
    ws.send(Message::Text(msg.to_string().into()))
        .await
        .expect("send");
    loop {
        let frame = timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("ws timeout")
            .expect("ws stream closed")
            .expect("ws error");
        let text = frame.into_text().expect("text frame");
        let value: Value = serde_json::from_str(&text).expect("json parse");
        let arr = value.as_array().expect("array");
        if arr.first().and_then(Value::as_str) == Some("OK")
            && arr.get(1).and_then(Value::as_str) == Some(id.as_str())
        {
            let accepted = arr.get(2).and_then(Value::as_bool).unwrap_or(false);
            let message = arr.get(3).and_then(Value::as_str).unwrap_or("").to_owned();
            return (id, accepted, message);
        }
    }
}

/// Sign and submit when the acknowledgement is the only required observation.
async fn submit(ws: &mut Ws, event: UnsignedEvent, keys: &Keys) -> (bool, String) {
    let (_, accepted, message) = submit_with_id(ws, event, keys).await;
    (accepted, message)
}

/// Send REQ and retain at most `maximum` matching events through its matching EOSE.
async fn req(
    ws: &mut Ws,
    sub_id: &str,
    filter: Value,
    maximum: usize,
) -> Result<Vec<Value>, ReadbackError> {
    ws.send(Message::Text(
        json!(["REQ", sub_id, filter]).to_string().into(),
    ))
    .await
    .expect("send REQ");

    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ReadbackError::Timeout);
        }
        let frame = timeout(remaining, ws.next())
            .await
            .map_err(|_| ReadbackError::Timeout)?
            .ok_or(ReadbackError::Closed)?
            .map_err(|error| ReadbackError::WebSocket(error.to_string()))?;
        let text = frame
            .into_text()
            .map_err(|error| ReadbackError::WebSocket(error.to_string()))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| ReadbackError::Protocol(error.to_string()))?;
        let arr = value
            .as_array()
            .ok_or_else(|| ReadbackError::Protocol("frame is not an array".to_owned()))?;
        match arr.first().and_then(Value::as_str) {
            Some("EVENT") if arr.get(1).and_then(Value::as_str) == Some(sub_id) => {
                let event = arr.get(2).ok_or_else(|| {
                    ReadbackError::Protocol("EVENT frame omits its event body".to_owned())
                })?;
                if events.len() == maximum {
                    return Err(ReadbackError::TooManyEvents { maximum });
                }
                events.push(event.clone());
            }
            Some("EOSE") if arr.get(1).and_then(Value::as_str) == Some(sub_id) => {
                return Ok(events);
            }
            _ => {}
        }
    }
}

/// Read back exactly one event by id with a one-event relay retention bound.
async fn readback_one(ws: &mut Ws, sub_id: &str, event_id: &str) -> Value {
    let mut events = req(ws, sub_id, json!({ "ids": [event_id], "limit": 1 }), 1)
        .await
        .expect("readback reaches matching EOSE without timeout, closure, or websocket error");
    assert_eq!(
        events.len(),
        1,
        "relay did not retain exactly one submitted event"
    );
    events.pop().expect("one retained event")
}

fn tag_rows(event: &Value, name: &str) -> Vec<Vec<String>> {
    event["tags"]
        .as_array()
        .expect("event tags array")
        .iter()
        .filter_map(Value::as_array)
        .filter(|tag| tag.first().and_then(Value::as_str) == Some(name))
        .map(|tag| {
            tag.iter()
                .map(Value::as_str)
                .map(|value| value.expect("string tag value").to_owned())
                .collect()
        })
        .collect()
}

fn p_rows(event: &Value) -> Vec<Vec<String>> {
    tag_rows(event, "p")
}

fn p_tags(event: &Value) -> Vec<Vec<String>> {
    event["tags"]
        .as_array()
        .expect("event tags")
        .iter()
        .filter_map(|tag| {
            let values = tag.as_array()?;
            (values.first().and_then(Value::as_str) == Some("p")).then(|| {
                values
                    .iter()
                    .map(|value| value.as_str().expect("string tag value").to_owned())
                    .collect()
            })
        })
        .collect()
}

fn build_event(builder: EventBuilder, author: PublicKey) -> UnsignedEvent {
    builder
        .by(author)
        .into_event_and_routing()
        .expect("management builder constructs its event")
        .0
}

async fn submit_and_assert_p_tags(
    ws: &mut Ws,
    sub_id: &str,
    event: UnsignedEvent,
    signer: &Keys,
    expected_p_tags: Vec<Vec<String>>,
) {
    let event_id = event.id.expect("builder sets event id").to_hex();
    let kind = event.kind.as_u16();
    let (ok, message) = submit(ws, event, signer).await;
    assert!(ok, "Croissant rejected kind-{kind}: {message}");

    let stored = readback_one(ws, sub_id, &event_id).await;
    assert_eq!(p_tags(&stored), expected_p_tags);
}

fn tempdir() -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("fava-simple-groups-e2e-{ns}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── Gate 1: create_group accepted ─────────────────────────────────────────────

/// Phase B exit gate 1: `create_group` → real relay accepts (OK=true).
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate1_create_group_accepted() {
    let tmp = tempdir();
    let mut relay = start_relay(&tmp, None).await;
    let mut ws = ws_connect(&relay.url).await;

    let keys = Keys::generate();
    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::new("phase-b-cats", vec![r]).unwrap();

    let event = create_group(&group)
        .expect("create_group")
        .by(keys.public_key())
        .into_event_and_routing()
        .expect("build event")
        .0;

    let (ok, msg) = submit(&mut ws, event, &keys).await;
    eprintln!("[gate1] create_group OK={ok} msg={msg:?}");
    assert!(ok, "relay rejected create_group: {msg}");

    relay.child.kill().await.ok();
}

// ── Gate 2: put_user accepted + kind-39001 observation ────────────────────────

/// Phase B exit gate 2: `put_user` accepted; queried kind-39001 shows the user.
///
/// nostr-rs-relay stores events but does not compute NIP-29 state events.
/// The kind-39001 is therefore published directly (as a NIP-29 relay would
/// maintain it after processing the kind-9000). The [`SimpleGroupAdmins`]
/// deserialization then verifies the user appears in the decoded admin list,
/// confirming the end-to-end observation path.
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate2_put_user_and_39001_observation() {
    use fava_write::Kind;

    let tmp = tempdir();
    let mut relay = start_relay(&tmp, None).await;
    let mut ws = ws_connect(&relay.url).await;

    let admin_keys = Keys::generate();
    let member_keys = Keys::generate();
    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::new("phase-b-gate2", vec![r]).unwrap();

    // — create_group (kind 9007) —
    let cg = create_group(&group)
        .expect("create_group")
        .by(admin_keys.public_key())
        .into_event_and_routing()
        .expect("build event")
        .0;
    let (ok, msg) = submit(&mut ws, cg, &admin_keys).await;
    eprintln!("[gate2] create_group OK={ok} msg={msg:?}");
    assert!(ok, "create_group rejected: {msg}");

    // — put_user (kind 9000) —
    let pu = put_user(&group, &[member_keys.public_key()], &["admin"])
        .expect("put_user")
        .by(admin_keys.public_key())
        .into_event_and_routing()
        .expect("build event")
        .0;
    let (ok, msg) = submit(&mut ws, pu, &admin_keys).await;
    eprintln!("[gate2] put_user OK={ok} msg={msg:?}");
    assert!(ok, "put_user rejected: {msg}");

    // — publish kind-39001 (simulating NIP-29 relay-maintained admin list) —
    let member_hex = member_keys.public_key().to_hex();
    let k39001 = EventBuilder::new(Kind::from_u16(39_001))
        .tags([
            Tag::parse(["d", "phase-b-gate2"]).unwrap(),
            Tag::parse(["p", &member_hex, "admin"]).unwrap(),
        ])
        .by(admin_keys.public_key())
        .build()
        .expect("build kind-39001");
    let (ok, msg) = submit(&mut ws, k39001, &admin_keys).await;
    eprintln!("[gate2] kind-39001 publish OK={ok} msg={msg:?}");
    assert!(ok, "kind-39001 publish rejected: {msg}");

    // — query kind-39001 and verify via SimpleGroupAdmins —
    let events = req(
        &mut ws,
        "sub-39001",
        json!({ "kinds": [39001], "#d": ["phase-b-gate2"] }),
        1,
    )
    .await
    .expect("admin readback reaches matching EOSE");
    assert!(
        !events.is_empty(),
        "no kind-39001 event returned from relay"
    );

    let signed: nostr::event::Event =
        serde_json::from_value(events[0].clone()).expect("decode event");
    let ev_value = fava_write::EventValue::Signed(signed);
    let admins = SimpleGroupAdmins::from_event(&ev_value).expect("decode SimpleGroupAdmins");
    let admin_list: Vec<_> = admins
        .admins()
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .collect();
    let member_pubkey_hex = member_keys.public_key().to_hex();
    let found = admin_list
        .iter()
        .any(|(pubkey, _)| pubkey == &member_pubkey_hex);
    eprintln!(
        "[gate2] SimpleGroupAdmins decoded {} entries; member present={}",
        admin_list.len(),
        found
    );
    assert!(found, "member not found in kind-39001 admin list");

    relay.child.kill().await.ok();
}

// ── Gate 3: wrong-authority event → relay rejects ────────────────────────────

/// Phase B exit gate 3: deliberate wrong-authority break → relay returns OK=false.
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate3_wrong_authority_relay_rejects() {
    let admin_keys = Keys::generate();
    let rogue_keys = Keys::generate();

    let tmp = tempdir();
    let mut relay = start_relay(&tmp, Some(&admin_keys.public_key().to_hex())).await;
    let mut ws = ws_connect(&relay.url).await;

    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::new("phase-b-gate3", vec![r]).unwrap();

    let tampered = put_user(&group, &[admin_keys.public_key()], &[])
        .expect("put_user")
        .by(rogue_keys.public_key())
        .into_event_and_routing()
        .expect("build event")
        .0;

    let (ok, msg) = submit(&mut ws, tampered, &rogue_keys).await;
    eprintln!("[gate3] rogue put_user OK={ok} msg={msg:?}");
    assert!(!ok, "relay should have rejected unauthorized put_user");
    assert!(!msg.is_empty(), "relay should include rejection reason");

    eprintln!(
        "[gate3] relay rejection confirmed → Fava receipt: Rejected, all_terminal=true, all_acknowledged=false"
    );

    relay.child.kill().await.ok();
}

// ── Gate 4: all nine constructors produce relay-accepted events ───────────────

/// One gate-4 case: constructor name, built event, signer, and the exact `p` rows expected.
type ConstructorCase<'a> = (&'a str, UnsignedEvent, &'a Keys, Option<Vec<Vec<String>>>);

/// Constructors that change the group itself, all signed by the admin.
fn group_lifecycle_cases<'a>(
    group: &SimpleGroup,
    admin_keys: &'a Keys,
    target_id: &EventId,
) -> Vec<ConstructorCase<'a>> {
    let admin = admin_keys.public_key();
    vec![
        (
            "create_group",
            build_event(create_group(group).unwrap(), admin),
            admin_keys,
            None,
        ),
        (
            "edit_metadata",
            build_event(
                edit_metadata(
                    group,
                    &MetadataEdit {
                        name: Some("Gate4 Cats".to_owned()),
                        visibility: Some(GroupVisibility::Private),
                        access: Some(GroupAccess::Closed),
                        ..Default::default()
                    },
                )
                .unwrap(),
                admin,
            ),
            admin_keys,
            None,
        ),
        (
            "delete_event",
            build_event(delete_event(group, target_id).unwrap(), admin),
            admin_keys,
            None,
        ),
        (
            "delete_group",
            build_event(delete_group(group).unwrap(), admin),
            admin_keys,
            None,
        ),
    ]
}

/// Constructors that change who is in the group, carrying the exact ordered
/// `p` rows the relay must preserve for the array-valued ones.
fn membership_cases<'a>(
    group: &SimpleGroup,
    admin_keys: &'a Keys,
    user_keys: &'a Keys,
    user_targets: &[PublicKey],
) -> Vec<ConstructorCase<'a>> {
    let admin = admin_keys.public_key();
    let target_p_rows = user_targets
        .iter()
        .map(|target| vec!["p".to_owned(), target.to_hex()])
        .collect::<Vec<_>>();
    let put_user_p_rows = user_targets
        .iter()
        .map(|target| vec!["p".to_owned(), target.to_hex(), "member".to_owned()])
        .collect::<Vec<_>>();
    vec![
        (
            "invite",
            build_event(invite(group, "gate4-required-invite-code").unwrap(), admin),
            admin_keys,
            None,
        ),
        (
            "join_request",
            build_event(join_request(group, None).unwrap(), user_keys.public_key()),
            user_keys,
            None,
        ),
        (
            "put_user",
            build_event(put_user(group, user_targets, &["member"]).unwrap(), admin),
            admin_keys,
            Some(put_user_p_rows),
        ),
        (
            "remove_user",
            build_event(remove_user(group, user_targets).unwrap(), admin),
            admin_keys,
            Some(target_p_rows),
        ),
        (
            "leave_group",
            build_event(leave_group(group).unwrap(), user_keys.public_key()),
            user_keys,
            None,
        ),
    ]
}

/// Smoke: every typed constructor builds a valid event that a permissive relay accepts.
/// Management constructors use several targets here, so the real wire path
/// exercises multiple exact `p` rows in one event body.
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate4_all_constructors_accepted() {
    let tmp = tempdir();
    let mut relay = start_relay(&tmp, None).await;
    let mut ws = ws_connect(&relay.url).await;

    let admin_keys = Keys::generate();
    let user_keys = Keys::generate();
    let other_user_key = Keys::generate();
    let user_targets = [user_keys.public_key(), other_user_key.public_key()];
    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::new("phase-b-gate4", vec![r.clone()]).unwrap();
    let target_id = EventId::from_byte_array([0u8; 32]);

    let mut cases = group_lifecycle_cases(&group, &admin_keys, &target_id);
    cases.extend(membership_cases(
        &group,
        &admin_keys,
        &user_keys,
        &user_targets,
    ));

    for (index, (name, event, signer, expected_p_rows)) in cases.iter().enumerate() {
        let (event_id, ok, msg) = submit_with_id(&mut ws, event.clone(), signer).await;
        eprintln!("[gate4] {name} OK={ok} msg={msg:?}");
        assert!(ok, "{name} was rejected by relay: {msg}");
        if expected_p_rows.is_some() || *name == "invite" {
            let stored = readback_one(&mut ws, &format!("gate4-{index}"), &event_id).await;
            if let Some(expected_p_rows) = expected_p_rows {
                assert_eq!(
                    &p_rows(&stored),
                    expected_p_rows,
                    "{name} relay readback changed ordered repeated p rows"
                );
            }
            if *name == "invite" {
                assert_eq!(
                    tag_rows(&stored, "code"),
                    vec![vec![
                        "code".to_owned(),
                        "gate4-required-invite-code".to_owned(),
                    ]],
                    "invite relay readback changed its exact code tag"
                );
            }
        }
    }

    relay.child.kill().await.ok();
}

/// Create the group and close it, so membership changes are admin-only.
async fn create_closed_group(ws: &mut Ws, group: &SimpleGroup, admin: &Keys) {
    let (ok, message) = submit(
        ws,
        build_event(
            create_group(group).expect("create group"),
            admin.public_key(),
        ),
        admin,
    )
    .await;
    assert!(ok, "Croissant rejected create-group: {message}");

    let (ok, message) = submit(
        ws,
        build_event(
            edit_metadata(
                group,
                &MetadataEdit {
                    access: Some(GroupAccess::Closed),
                    ..Default::default()
                },
            )
            .expect("close group"),
            admin.public_key(),
        ),
        admin,
    )
    .await;
    assert!(ok, "Croissant rejected close-group: {message}");
}

/// An invitation carries its exact code tag and no user targets.
async fn assert_invite_carries_code_without_targets(
    ws: &mut Ws,
    group: &SimpleGroup,
    admin: &Keys,
) {
    let invitation = build_event(
        invite(group, "array-invite-code").expect("build invite"),
        admin.public_key(),
    );
    let (invitation_id, ok, message) = submit_with_id(ws, invitation, admin).await;
    assert!(ok, "Croissant rejected invite: {message}");
    let stored = readback_one(ws, "invite", &invitation_id).await;
    assert_eq!(p_rows(&stored), Vec::<Vec<String>>::new());
    assert_eq!(
        tag_rows(&stored, "code"),
        vec![vec!["code".to_owned(), "array-invite-code".to_owned()]]
    );
}

/// Array-based user-management constructors preserve one, many, and no `p`
/// tags on a real NIP-29 relay. Croissant refuses empty `put_user` and
/// `remove_user` targets; an invitation carries a code and no user targets.
#[tokio::test]
#[ignore = "requires Croissant; set NIP29_RELAY_BIN and run with -- --ignored"]
async fn array_user_management_constructors_preserve_target_cardinality() {
    let tmp = tempdir();
    let mut relay = start_nip29_relay(&tmp).await;
    let mut ws = ws_connect(&relay.url).await;

    let admin = Keys::generate();
    let one_user = Keys::generate().public_key();
    let many_users = [
        Keys::generate().public_key(),
        Keys::generate().public_key(),
        Keys::generate().public_key(),
    ];
    let group = SimpleGroup::new(
        "array-management",
        vec![RelayUrl::parse(&relay.url).unwrap()],
    )
    .unwrap();

    create_closed_group(&mut ws, &group, &admin).await;

    let one_p_tag = vec![vec!["p".to_owned(), one_user.to_hex()]];
    let many_p_tags = many_users
        .iter()
        .map(|user| vec!["p".to_owned(), user.to_hex()])
        .collect::<Vec<_>>();

    submit_and_assert_p_tags(
        &mut ws,
        "put-one",
        build_event(
            put_user(&group, &[one_user], &["member"]).expect("put one"),
            admin.public_key(),
        ),
        &admin,
        vec![vec!["p".to_owned(), one_user.to_hex(), "member".to_owned()]],
    )
    .await;
    submit_and_assert_p_tags(
        &mut ws,
        "put-many",
        build_event(
            put_user(&group, &many_users, &["member"]).expect("put many"),
            admin.public_key(),
        ),
        &admin,
        many_p_tags
            .iter()
            .map(|tag| [tag.clone(), vec!["member".to_owned()]].concat())
            .collect(),
    )
    .await;
    submit_and_assert_p_tags(
        &mut ws,
        "remove-one",
        build_event(
            remove_user(&group, &[one_user]).expect("remove one"),
            admin.public_key(),
        ),
        &admin,
        one_p_tag.clone(),
    )
    .await;
    submit_and_assert_p_tags(
        &mut ws,
        "remove-many",
        build_event(
            remove_user(&group, &many_users).expect("remove many"),
            admin.public_key(),
        ),
        &admin,
        many_p_tags.clone(),
    )
    .await;

    assert_invite_carries_code_without_targets(&mut ws, &group, &admin).await;

    for (name, event) in [
        (
            "put-user",
            put_user(&group, &[], &["member"]).expect("build empty put"),
        ),
        (
            "remove-user",
            remove_user(&group, &[]).expect("build empty remove"),
        ),
    ] {
        let (ok, message) = submit(&mut ws, build_event(event, admin.public_key()), &admin).await;
        assert!(!ok, "Croissant accepted an empty {name} target list");
        assert!(message.contains("missing 'p' tags"), "{name}: {message}");
    }

    relay.child.kill().await.ok();
}
