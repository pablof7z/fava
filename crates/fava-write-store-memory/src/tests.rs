use std::num::NonZeroU64;

use fava_write::{EventBuilder, Kind, WriteIntent, WriteRouting};
use fava_write_store::{WriteStore, WriteStoreError};
use nostr::key::Keys;

use super::MemoryWriteStore;

#[test]
fn exhausted_write_identity_refuses_without_state_or_notification() {
    let store = MemoryWriteStore::default();
    store.state.lock().expect("write state lock").next_identity =
        NonZeroU64::new(u64::MAX).expect("maximum is nonzero");
    let mut changes = store.receipt_changes();
    let keys = Keys::parse("0101010101010101010101010101010101010101010101010101010101010101")
        .expect("fixed test key");
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content("identity exhaustion")
        .build()
        .expect("bounded event");

    assert_eq!(
        store.accept(WriteIntent::event(event, WriteRouting::Automatic).expect("valid intent")),
        Err(WriteStoreError::Refused(
            "write identity exhausted".to_owned()
        ))
    );
    let state = store.state.lock().expect("write state lock");
    assert_eq!(state.next_identity.get(), u64::MAX);
    assert_eq!(state.revision, 0);
    assert!(state.writes.is_empty());
    assert!(changes.try_recv().is_err());
}
