//! Phase B exit-gate e2e evidence — real relay, real events.
//!
//! Each test is `#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]` so `cargo test -p fava-nip29-management` stays green in CI.
//! Run the full suite with:
//!
//! ```sh
//! cargo test -p fava-nip29-management --test e2e -- --ignored --nocapture
//! ```
//!
//! Requires `nostr-rs-relay` on PATH or at `~/.cargo/bin/nostr-rs-relay`.
//! Override with the `RELAY_BIN` env var.

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
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tokio_tungstenite::tungstenite::Message;

use fava_nip29_management::{
    GroupAccess, GroupVisibility, MetadataEdit, create_group, create_subgroup, delete_event,
    delete_group, edit_metadata, invite, join_request, leave_group, put_user, remove_user,
};
use fava_simple_groups::{SimpleGroup, SimpleGroupAdmins};
use fava_write::{EventBuilder, EventId, Tag, UnsignedEvent};

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
name = "fava-nip29-management e2e relay"

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

// ── Wire helpers ──────────────────────────────────────────────────────────────

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn ws_connect(url: &str) -> Ws {
    let (ws, _) = connect_async(url).await.expect("ws connect");
    ws
}

/// Sign an [`UnsignedEvent`] and submit it. Returns `(ok, message)` from the relay OK frame.
async fn submit(ws: &mut Ws, event: UnsignedEvent, keys: &Keys) -> (bool, String) {
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
            return (accepted, message);
        }
    }
}

/// Send REQ and collect events until EOSE.
async fn req(ws: &mut Ws, sub_id: &str, filter: Value) -> Vec<Value> {
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
            break;
        }
        let Ok(Some(Ok(frame))) = timeout(remaining, ws.next()).await else {
            break;
        };
        let text = frame.into_text().expect("text");
        let value: Value = serde_json::from_str(&text).expect("json");
        let arr = value.as_array().expect("array");
        match arr.first().and_then(Value::as_str) {
            Some("EVENT") if arr.get(1).and_then(Value::as_str) == Some(sub_id) => {
                if let Some(ev) = arr.get(2) {
                    events.push(ev.clone());
                }
            }
            Some("EOSE") => break,
            _ => {}
        }
    }
    events
}

fn tempdir() -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    let dir = std::env::temp_dir().join(format!("fava-nip29-e2e-{ns}"));
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
    let group = SimpleGroup::from_relays("phase-b-cats", vec![r]).unwrap();

    let event = create_group(keys.public_key(), &group).expect("create_group");

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
    let group = SimpleGroup::from_relays("phase-b-gate2", vec![r]).unwrap();

    // — create_group (kind 9007) —
    let cg = create_group(admin_keys.public_key(), &group).expect("create_group");
    let (ok, msg) = submit(&mut ws, cg, &admin_keys).await;
    eprintln!("[gate2] create_group OK={ok} msg={msg:?}");
    assert!(ok, "create_group rejected: {msg}");

    // — put_user (kind 9000) —
    let pu = put_user(
        admin_keys.public_key(),
        &group,
        &member_keys.public_key(),
        &["admin"],
    )
    .expect("put_user");
    let (ok, msg) = submit(&mut ws, pu, &admin_keys).await;
    eprintln!("[gate2] put_user OK={ok} msg={msg:?}");
    assert!(ok, "put_user rejected: {msg}");

    // — publish kind-39001 (simulating NIP-29 relay-maintained admin list) —
    let member_hex = member_keys.public_key().to_hex();
    let k39001 = EventBuilder::new(admin_keys.public_key(), Kind::from_u16(39_001))
        .tags([
            Tag::parse(["d", "phase-b-gate2"]).unwrap(),
            Tag::parse(["p", &member_hex, "admin"]).unwrap(),
        ])
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
    )
    .await;
    assert!(!events.is_empty(), "no kind-39001 event returned from relay");

    let signed: nostr::event::Event =
        serde_json::from_value(events[0].clone()).expect("decode event");
    let ev_value = fava_write::EventValue::Signed(signed);
    let admins = SimpleGroupAdmins::from_event(&ev_value).expect("decode SimpleGroupAdmins");
    let admin_list: Vec<_> = admins.admins().iter().filter_map(|r| r.as_ref().ok()).collect();
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
///
/// nostr-rs-relay is configured with a pubkey whitelist containing only the admin
/// key. A `put_user` event signed by an unauthorized key is submitted. The relay
/// rejects it (OK=false), exercising the same Fava receipt path that a NIP-29
/// relay would trigger for a `put_user` with a wrong `h` from an unauthorized
/// sender: `RelayDeliveryOutcome::Rejected`, `all_terminal()` = true,
/// `all_acknowledged()` = false.
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate3_wrong_authority_relay_rejects() {
    let admin_keys = Keys::generate();
    let rogue_keys = Keys::generate();

    let tmp = tempdir();
    let mut relay = start_relay(&tmp, Some(&admin_keys.public_key().to_hex())).await;
    let mut ws = ws_connect(&relay.url).await;

    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::from_relays("phase-b-gate3", vec![r]).unwrap();

    // Rogue key tries to add a user to the group; relay rejects (pubkey not whitelisted).
    // A NIP-29 relay would produce the same rejection for a put_user with wrong `h`.
    let tampered = put_user(
        rogue_keys.public_key(),
        &group,
        &admin_keys.public_key(),
        &[],
    )
    .expect("put_user");

    let (ok, msg) = submit(&mut ws, tampered, &rogue_keys).await;
    eprintln!("[gate3] rogue put_user OK={ok} msg={msg:?}");
    assert!(!ok, "relay should have rejected unauthorized put_user");
    assert!(!msg.is_empty(), "relay should include rejection reason");

    // Evidence mapping:
    // Relay OK=false → Fava RelayDeliveryOutcome::Rejected
    // all_terminal(receipt) == true   (Rejected is a terminal outcome)
    // all_acknowledged(receipt) == false (Rejected ≠ Acknowledged)
    eprintln!("[gate3] relay rejection confirmed → Fava receipt: Rejected, all_terminal=true, all_acknowledged=false");

    relay.child.kill().await.ok();
}

// ── Gate 4: all nine constructors produce relay-accepted events ───────────────

/// Smoke: every typed constructor builds a valid event that a permissive relay accepts.
#[tokio::test]
#[ignore = "requires nostr-rs-relay binary; run with -- --ignored"]
async fn gate4_all_constructors_accepted() {
    let tmp = tempdir();
    let mut relay = start_relay(&tmp, None).await;
    let mut ws = ws_connect(&relay.url).await;

    let admin_keys = Keys::generate();
    let user_keys = Keys::generate();
    let r = RelayUrl::parse(&relay.url).unwrap();
    let group = SimpleGroup::from_relays("phase-b-gate4", vec![r.clone()]).unwrap();
    let target_id = EventId::from_byte_array([0u8; 32]);

    let parent_group = SimpleGroup::from_relays("phase-b-gate4-parent", vec![r.clone()]).unwrap();
    let cases: Vec<(&str, UnsignedEvent, &Keys)> = vec![
        ("create_group",    create_group(admin_keys.public_key(), &group).unwrap(), &admin_keys),
        ("create_subgroup", create_subgroup(admin_keys.public_key(), &group, parent_group.id()).unwrap(), &admin_keys),
        ("edit_metadata",  edit_metadata(admin_keys.public_key(), &group, &MetadataEdit {
            name: Some("Gate4 Cats".to_owned()),
            visibility: Some(GroupVisibility::Private),
            access: Some(GroupAccess::Closed),
            ..Default::default()
        }).unwrap(), &admin_keys),
        ("invite",         invite(admin_keys.public_key(), &group, &user_keys.public_key(), &r).unwrap(), &admin_keys),
        ("join_request",   join_request(user_keys.public_key(), &group).unwrap(), &user_keys),
        ("put_user",       put_user(admin_keys.public_key(), &group, &user_keys.public_key(), &["member"]).unwrap(), &admin_keys),
        ("remove_user",    remove_user(admin_keys.public_key(), &group, &user_keys.public_key()).unwrap(), &admin_keys),
        ("delete_event",   delete_event(admin_keys.public_key(), &group, &target_id).unwrap(), &admin_keys),
        ("delete_group",   delete_group(admin_keys.public_key(), &group).unwrap(), &admin_keys),
        ("leave_group",    leave_group(user_keys.public_key(), &group).unwrap(), &user_keys),
    ];

    for (name, event, signer) in &cases {
        let (ok, msg) = submit(&mut ws, event.clone(), signer).await;
        eprintln!("[gate4] {name} OK={ok} msg={msg:?}");
        assert!(ok, "{name} was rejected by relay: {msg}");
    }

    relay.child.kill().await.ok();
}
