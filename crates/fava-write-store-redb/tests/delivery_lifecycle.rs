//! Durable Redb delivery-generation and budget evidence.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_write::{EventBuilder, Kind, RelayDeliveryOutcome, WriteIntent, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;

#[test]
fn unreachable_generation_can_retry_without_spending_attempt_budget() {
    let store = RedbWriteStore::open(unique_path()).expect("redb opens");
    let relay = RelayUrl::parse("wss://unreachable-retry.example").expect("relay parses");
    let session = RelaySessionKey::new(relay.clone(), RelayAccess::public());
    let keys = Keys::generate();
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("unreachable retry")
        .build()
        .expect("event builds")
        .finalize(&keys)
        .expect("event signs");
    let accepted = store
        .accept(
            WriteIntent::presigned(event, WriteRouting::Explicit(BTreeSet::from([relay])))
                .expect("intent validates"),
        )
        .expect("write is accepted");

    store
        .begin_attempt(
            accepted.write_id,
            accepted.receipt_id,
            accepted.current.publication.materialization_id,
            accepted.current.id(),
            &session,
            1,
        )
        .expect("first generation begins");
    let unreachable = store
        .record_outcome(
            accepted.write_id,
            accepted.receipt_id,
            accepted.current.publication.materialization_id,
            accepted.current.id(),
            &session,
            1,
            RelayDeliveryOutcome::Unreachable {
                reason: "relay is offline".to_owned(),
            },
        )
        .expect("unreachable outcome commits");
    assert_eq!(unreachable.attempts.get(&session), Some(&1));
    assert_eq!(unreachable.spent(&session), 0);

    let retrying = store
        .begin_attempt(
            accepted.write_id,
            accepted.receipt_id,
            accepted.current.publication.materialization_id,
            accepted.current.id(),
            &session,
            2,
        )
        .expect("next generation begins after an unreachable connection");
    assert_eq!(retrying.attempts.get(&session), Some(&2));
    assert_eq!(retrying.spent(&session), 0);
    assert!(matches!(
        retrying.destinations().get(&session),
        Some(RelayDeliveryOutcome::Attempting)
    ));
}

fn unique_path() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fava-redb-unreachable-retry-{}-{nonce}.redb",
        std::process::id()
    ))
}
