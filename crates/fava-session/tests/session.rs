//! Owner-level evidence for bounded exact-key signer attachment.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_session::{Session, SessionError};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, PublicKey, UnsignedEvent};
use nostr::key::Keys;
use tokio::sync::watch;

#[test]
fn empty_and_exact_key_sessions_are_valid() {
    let empty = Session::new(std::iter::empty()).expect("empty session");
    let alice = Arc::new(TestSigner(Keys::generate().public_key()));
    let alice_key = alice.public_key();
    let session = Session::new([alice as Arc<dyn Signer>]).expect("one signer session");

    assert!(empty.signer(alice_key).is_none());
    let (generation, selected) = session.signer(alice_key).expect("Alice is indexed");
    assert_eq!(selected.public_key(), alice_key);
    assert!(session.is_current(alice_key, generation));
}

#[test]
fn lookup_is_independent_of_insertion_order() {
    let alice_key = Keys::generate().public_key();
    let bob_key = Keys::generate().public_key();
    let alice = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let bob = Arc::new(TestSigner(bob_key)) as Arc<dyn Signer>;
    let forward = Session::new([Arc::clone(&alice), Arc::clone(&bob)]).unwrap();
    let reverse = Session::new([bob, alice]).unwrap();

    assert_eq!(forward.signer(alice_key).unwrap().1.public_key(), alice_key);
    assert_eq!(reverse.signer(alice_key).unwrap().1.public_key(), alice_key);
    assert_eq!(forward.signer(bob_key).unwrap().1.public_key(), bob_key);
    assert_eq!(reverse.signer(bob_key).unwrap().1.public_key(), bob_key);
}

#[test]
fn duplicate_add_refuses_without_replacing_current_attachment() {
    let alice_key = Keys::generate().public_key();
    let first = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let duplicate = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let session = Session::new([Arc::clone(&first)]).unwrap();
    let (generation, selected) = session.signer(alice_key).unwrap();

    assert_eq!(
        session.add_signer(duplicate),
        Err(SessionError::DuplicateSigner(alice_key))
    );
    let (current_generation, current) = session.signer(alice_key).unwrap();
    assert_eq!(current_generation, generation);
    assert!(Arc::ptr_eq(&selected, &current));
}

struct TestSigner(PublicKey);

impl Signer for TestSigner {
    fn public_key(&self) -> PublicKey {
        self.0
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        &self,
        _event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + '_>> {
        Box::pin(std::future::pending())
    }
}
