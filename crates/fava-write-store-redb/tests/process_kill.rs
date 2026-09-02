//! Actual process-kill evidence for every M5 durable commit/effect boundary.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fava_write::{
    EventBuilder, EventValue, Kind, ReceiptOutcome, RelayDeliveryOutcome, SignatureState,
    WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[path = "process_kill/semantic.rs"]
mod semantic;

const CHILD_BOUNDARY: &str = "FAVA_REDB_KILL_BOUNDARY";
const CHILD_PATH: &str = "FAVA_REDB_KILL_PATH";
const CHILD_MARKER: &str = "FAVA_REDB_KILL_MARKER";
static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

#[test]
fn boundary_child() {
    let Ok(boundary) = env::var(CHILD_BOUNDARY) else {
        return;
    };
    let path = PathBuf::from(env::var(CHILD_PATH).expect("child database path"));
    let marker = PathBuf::from(env::var(CHILD_MARKER).expect("child marker path"));
    let store = RedbWriteStore::open(path).expect("child store opens");
    if boundary != "before-accept" {
        let accepted = store.accept(intent()).expect("child acceptance commits");
        if matches!(boundary.as_str(), "signature" | "attempt" | "outcome") {
            store
                .authorize_signing(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.revision_id,
                    accepted.current.id(),
                )
                .expect("child signing authorization commits");
            let signed = unsigned()
                .finalize(&keys())
                .expect("deterministic event signs");
            store
                .install_signed(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.revision_id,
                    accepted.current.id(),
                    signed,
                )
                .expect("child signature commits");
        }
        if matches!(boundary.as_str(), "attempt" | "outcome") {
            store
                .begin_attempt(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.revision_id,
                    accepted.current.id(),
                    &session(),
                    1,
                )
                .expect("child attempt commits");
        }
        if boundary == "outcome" {
            store
                .record_outcome(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.revision_id,
                    accepted.current.id(),
                    &session(),
                    1,
                    RelayDeliveryOutcome::Acknowledged {
                        message: "stored".to_owned(),
                    },
                )
                .expect("child outcome commits");
        }
        if boundary == "cancel" {
            store
                .cancel(accepted.receipt_id)
                .expect("child cancellation commits");
        }
    }
    fs::write(marker, b"committed").expect("child marker writes");
    loop {
        thread::park();
    }
}

#[test]
fn every_m5_commit_and_effect_boundary_survives_sigkill_exactly() {
    if env::var(CHILD_BOUNDARY).is_ok() {
        return;
    }
    for boundary in [
        "before-accept",
        "acceptance",
        "signature",
        "attempt",
        "outcome",
        "cancel",
    ] {
        let root = unique_root(boundary);
        fs::create_dir_all(&root).expect("boundary directory");
        let database = root.join("writes.redb");
        let marker = root.join("committed.marker");
        let mut child = Command::new(env::current_exe().expect("test executable"))
            .args(["--exact", "boundary_child", "--nocapture"])
            .env(CHILD_BOUNDARY, boundary)
            .env(CHILD_PATH, &database)
            .env(CHILD_MARKER, &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("boundary child starts");
        wait_for(&marker, &mut child);
        child.kill().expect("SIGKILL succeeds");
        let status = child.wait().expect("killed child reaped");
        assert!(!status.success(), "{boundary} child must be hard-killed");

        let store = RedbWriteStore::open(&database).expect("store recovers after kill");
        let receipt = store
            .receipt(fava_write::ReceiptId::try_from(1).expect("nonzero receipt identity"))
            .expect("receipt read");
        assert_boundary(boundary, receipt.as_ref());
    }
}

fn assert_boundary(boundary: &str, receipt: Option<&fava_write::Receipt>) {
    if boundary == "before-accept" {
        assert!(receipt.is_none());
        return;
    }
    let receipt = receipt.expect("committed receipt recovers");
    assert_eq!(receipt.receipt_id.as_u64(), 1);
    match boundary {
        "acceptance" => {
            assert!(matches!(receipt.current.event, EventValue::Unsigned(_)));
            assert_eq!(
                receipt.current.publication.signature,
                SignatureState::Unsigned
            );
            assert_eq!(receipt.outcome, ReceiptOutcome::Open);
        }
        "signature" => {
            assert!(matches!(receipt.current.event, EventValue::Signed(_)));
            assert_eq!(receipt.outcome, ReceiptOutcome::Open);
        }
        "attempt" => {
            assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
            assert!(receipt.destinations().values().all(|outcome| matches!(
                outcome,
                RelayDeliveryOutcome::Unknown { reason }
                    if reason.contains("before outcome commit")
            )));
        }
        "outcome" => {
            assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
            assert!(receipt.destinations().values().all(|outcome| matches!(
                outcome,
                RelayDeliveryOutcome::Acknowledged { message } if message == "stored"
            )));
        }
        "cancel" => assert_eq!(receipt.outcome, ReceiptOutcome::Cancelled),
        _ => panic!("unknown boundary"),
    }
}

fn intent() -> WriteIntent {
    WriteIntent::event(
        unsigned(),
        WriteRouting::explicit([relay()]).expect("explicit route validates"),
    )
    .expect("intent validates")
}

fn unsigned() -> fava_write::UnsignedEvent {
    EventBuilder::new(Kind::TextNote)
        .created_at(fava_write::Timestamp::from(1_700_000_000_u64))
        .content("durability-boundary")
        .by(keys().public_key())
        .build()
        .expect("event builds")
}

fn keys() -> Keys {
    Keys::parse("0000000000000000000000000000000000000000000000000000000000000001")
        .expect("fixed key parses")
}

fn relay() -> RelayUrl {
    RelayUrl::parse("wss://durability.example").expect("relay parses")
}

fn session() -> RelayUrl {
    relay()
}

fn unique_root(boundary: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "fava-redb-kill-{}-{boundary}-{nonce}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn wait_for(marker: &Path, child: &mut std::process::Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !marker.exists() {
        assert!(
            child.try_wait().expect("child status").is_none(),
            "child exited before marker"
        );
        assert!(Instant::now() < deadline, "child marker deadline elapsed");
        thread::sleep(Duration::from_millis(10));
    }
}
