//! Owner-level evidence for bounded exact-key signer attachment.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Barrier;
use std::thread;

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

#[test]
fn replace_remove_and_missing_mutations_are_exact() {
    let alice_key = Keys::generate().public_key();
    let first = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let replacement = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let session = Session::new([Arc::clone(&first)]).unwrap();
    let mut changes = session.subscribe();
    let original_generation = session.signer(alice_key).unwrap().0;

    session
        .replace_signer(Arc::clone(&replacement))
        .expect("explicit replacement succeeds");
    assert!(changes.has_changed().unwrap());
    let replacement_revision = *changes.borrow_and_update();
    let (replacement_generation, current) = session.signer(alice_key).unwrap();
    assert_eq!(replacement_generation, replacement_revision);
    assert!(replacement_generation > original_generation);
    assert!(Arc::ptr_eq(&replacement, &current));

    session.remove_signer(alice_key).expect("removal succeeds");
    assert!(changes.has_changed().unwrap());
    let removal_revision = *changes.borrow_and_update();
    assert!(removal_revision > replacement_revision);
    assert!(session.signer(alice_key).is_none());

    assert_eq!(
        session.remove_signer(alice_key),
        Err(SessionError::MissingSigner(alice_key))
    );
    assert_eq!(
        session.replace_signer(first),
        Err(SessionError::MissingSigner(alice_key))
    );
    assert!(!changes.has_changed().unwrap());
}

#[test]
fn sixty_fourth_succeeds_sixty_fifth_refuses_and_replace_still_succeeds() {
    let keys: Vec<_> = (0..65).map(|_| Keys::generate().public_key()).collect();
    let initial = keys[..63]
        .iter()
        .copied()
        .map(|key| Arc::new(TestSigner(key)) as Arc<dyn Signer>);
    let session = Session::new(initial).expect("63 signer session");

    session
        .add_signer(Arc::new(TestSigner(keys[63])))
        .expect("64th signer succeeds");
    assert_eq!(
        session.add_signer(Arc::new(TestSigner(keys[64]))),
        Err(SessionError::SignerCapacityExceeded { limit: 64 })
    );
    assert!(session.signer(keys[64]).is_none());

    let replacement = Arc::new(TestSigner(keys[0])) as Arc<dyn Signer>;
    session
        .replace_signer(Arc::clone(&replacement))
        .expect("replacement does not grow capacity");
    assert!(Arc::ptr_eq(
        &session.signer(keys[0]).unwrap().1,
        &replacement
    ));
}

#[test]
fn concurrent_final_slot_growth_never_exceeds_capacity() {
    let keys: Vec<_> = (0..65).map(|_| Keys::generate().public_key()).collect();
    let initial = keys[..63]
        .iter()
        .copied()
        .map(|key| Arc::new(TestSigner(key)) as Arc<dyn Signer>);
    let session = Session::new(initial).expect("63 signer session");
    let barrier = Arc::new(Barrier::new(3));
    let contenders: Vec<_> = keys[63..]
        .iter()
        .copied()
        .map(|key| {
            let session = session.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                (key, session.add_signer(Arc::new(TestSigner(key))))
            })
        })
        .collect();
    barrier.wait();
    let outcomes: Vec<_> = contenders
        .into_iter()
        .map(|contender| contender.join().expect("contender does not panic"))
        .collect();

    assert_eq!(
        outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|(_, result)| {
                *result == Err(SessionError::SignerCapacityExceeded { limit: 64 })
            })
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|(key, _)| session.signer(*key).is_some())
            .count(),
        1
    );
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
