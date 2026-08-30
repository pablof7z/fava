//! Public-facade evidence for session-owned account lifecycle delegation.

use std::sync::Arc;

use fava::{Fava, SessionError, SignerAvailability};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

#[test]
fn facade_reports_exact_signer_attachment_status() {
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("local Fava assembly");
    let alice = Keys::generate();
    let public_key = alice.public_key();
    assert_eq!(fava.signer_status(public_key), None);

    fava.add_signer(Arc::new(LocalSigner::new(alice.clone())))
        .expect("Alice attaches");
    let first = fava.signer_status(public_key).expect("Alice status");
    assert_eq!(first.1, SignerAvailability::Available);

    fava.replace_signer(Arc::new(LocalSigner::new(alice)))
        .expect("Alice replaces");
    let replacement = fava.signer_status(public_key).expect("replacement status");
    assert!(replacement.0 > first.0);
    assert_eq!(replacement.1, SignerAvailability::Available);

    fava.remove_signer(public_key).expect("Alice detaches");
    assert_eq!(fava.signer_status(public_key), None);
}

#[test]
fn facade_delegates_the_bounded_current_account_lifecycle() {
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .expect("local Fava assembly");
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let mut accounts = vec![alice, bob];
    accounts.sort();

    assert_eq!(fava.session_revision(), 0);
    fava.add_account(alice).expect("Alice adds");
    fava.add_account(bob).expect("Bob adds");
    fava.select_account(alice).expect("Alice selects");
    assert_eq!(fava.accounts(), accounts);
    assert_eq!(fava.current_account(), Some(alice));
    assert_eq!(
        fava.current_account_snapshot(),
        (Some(alice), fava.session_revision())
    );

    fava.clear_current_account().expect("selection clears");
    assert_eq!(fava.current_account(), None);
    fava.remove_account(alice).expect("Alice removes");
    assert_eq!(fava.accounts(), vec![bob]);
    assert_eq!(
        fava.select_account(alice),
        Err(SessionError::MissingAccount(alice))
    );
}
