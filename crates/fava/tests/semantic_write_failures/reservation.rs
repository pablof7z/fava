use std::num::NonZeroUsize;
use std::sync::Arc;

use fava::Kind;
use fava_event_cache_memory::MemoryEventCache;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

use super::failure_support::{assembly, edit_intent};
use super::{ControlledMaterializer, ERROR, PANIC, VALID, WRONG_TIMESTAMP};

#[tokio::test(flavor = "current_thread")]
async fn every_pre_custody_provider_failure_releases_active_reservation() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::bounded(NonZeroUsize::new(1).unwrap()));
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::new(MemoryEventCache::default()),
        store,
        vec![Arc::clone(&materializer)],
    );

    for failure in [ERROR, PANIC, WRONG_TIMESTAMP] {
        materializer.set(failure);
        assert!(
            fava.publish(edit_intent(keys.public_key(), Kind::ContactList))
                .is_err()
        );
        materializer.set(VALID);
        let accepted = fava
            .publish(edit_intent(keys.public_key(), Kind::ContactList))
            .expect("the same sole capacity slot is reusable");
        assert!(
            fava.cancel_write(accepted.receipt_id)
                .expect("accepted generation cancels")
        );
    }
}
