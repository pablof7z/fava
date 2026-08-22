use std::num::NonZeroUsize;
use std::sync::Arc;

use fava::{Kind, PublicationError, PublishError};
use fava_event_cache_memory::MemoryEventCache;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

use super::failure_support::{assembly, edit, publish_edit};
use super::faults::FaultingWriteStore;
use super::support::relay_url;
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
            fava.by(keys.public_key())
                .to([relay_url()])
                .expect("explicit route validates")
                .publish(edit(Kind::ContactList))
                .is_err()
        );
        materializer.set(VALID);
        let accepted = publish_edit(&fava, keys.public_key(), Kind::ContactList);
        assert!(
            fava.cancel_write(accepted.receipt_id())
                .expect("accepted generation cancels")
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn release_failure_preserves_preparation_context_and_does_not_hide_capacity_state() {
    let keys = Keys::generate();
    let store = Arc::new(FaultingWriteStore::new());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        vec![Arc::clone(&materializer)],
    );
    materializer.set(ERROR);
    store.fail_reservation_release(true);

    let error = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .expect_err("preparation and reservation release both refuse");
    let PublishError::Publication(PublicationError::Store(error)) = error else {
        panic!("release mutation failure was not typed as store evidence");
    };
    let message = error.to_string();
    assert!(message.contains("semantic preparation failed"));
    assert!(message.contains("reservation release"));

    store.fail_reservation_release(false);
    materializer.set(VALID);
    fava.by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .expect("released capacity remains reusable");
}

#[tokio::test(flavor = "current_thread")]
async fn initial_automatic_route_commits_with_acceptance_or_returns_without_custody() {
    let keys = Keys::generate();
    let store = Arc::new(FaultingWriteStore::new());
    let materializer = Arc::new(ControlledMaterializer::new(Kind::ContactList));
    let fava = assembly(
        &keys,
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        vec![materializer],
    );
    store.fail_initial_route_acceptance(true);

    assert!(matches!(
        fava.by(keys.public_key()).publish(edit(Kind::ContactList)),
        Err(PublishError::Publication(PublicationError::Store(_)))
    ));
    assert_eq!(store.len().expect("store remains readable"), 0);

    store.fail_initial_route_acceptance(false);
    let write = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .expect("atomic route acceptance succeeds after fault clears");
    let receipt = write.receipt().expect("accepted receipt remains readable");
    assert!(receipt.route_revision > 0);
    assert!(receipt.route_settled);
}
