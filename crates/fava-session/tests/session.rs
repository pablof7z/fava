//! Owner-level evidence for exact-key signer attachment.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use fava_session::{Session, SessionError};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, EventBuilder, Kind, PublicKey, UnsignedEvent};
use nostr::key::Keys;
use tokio::sync::watch;

#[test]
fn empty_and_exact_key_sessions_are_valid() {
    let empty = Session::new(std::iter::empty()).expect("empty session");
    let alice = Arc::new(TestSigner(Keys::generate().public_key()));
    let alice_key = alice.public_key();
    let session = Session::new([alice as Arc<dyn Signer>]).expect("one signer session");

    assert!(empty.signer(alice_key).is_none());
    let (generation, availability) = session.signer(alice_key).expect("Alice is indexed");
    assert_eq!(availability, SignerAvailability::Available);
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

    assert!(forward.signer(alice_key).is_some());
    assert!(reverse.signer(alice_key).is_some());
    assert!(forward.signer(bob_key).is_some());
    assert!(reverse.signer(bob_key).is_some());
}

#[test]
fn duplicate_add_refuses_without_replacing_current_attachment() {
    let alice_key = Keys::generate().public_key();
    let first = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let duplicate = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let session = Session::new([Arc::clone(&first)]).unwrap();
    let generation = session.signer(alice_key).unwrap().0;

    assert_eq!(
        session.add_signer(duplicate),
        Err(SessionError::DuplicateSigner(alice_key))
    );
    let current_generation = session.signer(alice_key).unwrap().0;
    assert_eq!(current_generation, generation);
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
    let replacement_generation = session.signer(alice_key).unwrap().0;
    assert_eq!(replacement_generation, replacement_revision);
    assert!(replacement_generation > original_generation);

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
fn invocation_is_exact_generation_and_returned_future_releases_replacement() {
    let key = Keys::generate().public_key();
    let retired = Arc::new(InvocationSigner::new(key));
    let replacement = Arc::new(InvocationSigner::new(key));
    let session = Session::new([Arc::clone(&retired) as Arc<dyn Signer>]).unwrap();
    let retired_generation = session.signer(key).unwrap().0;
    session
        .replace_signer(Arc::clone(&replacement) as Arc<dyn Signer>)
        .unwrap();
    let event = EventBuilder::new(Kind::TextNote).by(key).build().unwrap();
    let (_, cancel) = watch::channel(false);

    assert!(
        session
            .invoke_signer(key, retired_generation, event.clone(), cancel.clone())
            .is_none(),
        "a generation retired after snapshot reached provider invocation"
    );
    assert_eq!(retired.calls(), 0);

    let replacement_generation = session.signer(key).unwrap().0;
    let pending = session
        .invoke_signer(key, replacement_generation, event, cancel)
        .expect("current generation invokes");
    assert_eq!(replacement.calls(), 1);
    session.remove_signer(key).expect(
        "replacement/removal is excluded only during method invocation, not while awaiting",
    );
    assert!(!session.is_current(key, replacement_generation));
    drop(pending);
}

#[test]
fn account_set_selection_and_revision_are_atomic_and_bounded() {
    let alice_key = Keys::generate().public_key();
    let bob_key = Keys::generate().public_key();
    let alice = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let session = Session::new([alice]).expect("signer-backed account seeds the session");
    let initial_revision = session.revision();
    let mut changes = session.subscribe();
    let mut all_accounts = vec![alice_key, bob_key];
    all_accounts.sort();

    assert_eq!(session.accounts(), vec![alice_key]);
    assert_eq!(session.current_account(), None);

    session
        .add_account(bob_key)
        .expect("pubkey-only account adds");
    assert_eq!(session.accounts(), all_accounts);
    assert_eq!(session.revision(), initial_revision + 1);
    assert!(changes.has_changed().expect("account addition signals"));
    assert_eq!(*changes.borrow_and_update(), session.revision());

    session
        .select_account(alice_key)
        .expect("known account selects");
    assert_eq!(session.current_account(), Some(alice_key));
    assert_eq!(session.revision(), initial_revision + 2);
    assert!(changes.has_changed().expect("selection signals"));
    assert_eq!(*changes.borrow_and_update(), session.revision());

    session
        .clear_current_account()
        .expect("selection clears atomically");
    assert_eq!(session.current_account(), None);
    assert_eq!(session.revision(), initial_revision + 3);
    assert!(changes.has_changed().expect("clear signals"));
    assert_eq!(*changes.borrow_and_update(), session.revision());

    session
        .select_account(alice_key)
        .expect("known account reselects");
    assert_eq!(session.current_account(), Some(alice_key));
    assert_eq!(session.revision(), initial_revision + 4);
    assert!(changes.has_changed().expect("reselection signals"));
    assert_eq!(*changes.borrow_and_update(), session.revision());

    session
        .remove_account(alice_key)
        .expect("selected account removes atomically");
    assert_eq!(session.accounts(), vec![bob_key]);
    assert_eq!(session.current_account(), None);
    assert!(session.signer(alice_key).is_none());
    assert_eq!(session.revision(), initial_revision + 5);
    assert!(changes.has_changed().expect("removal signals once"));
    assert_eq!(*changes.borrow_and_update(), session.revision());
    assert!(!changes.has_changed().expect("one removal has one signal"));

    assert_eq!(
        session.remove_account(alice_key),
        Err(SessionError::MissingAccount(alice_key))
    );
    assert_eq!(
        session.select_account(alice_key),
        Err(SessionError::MissingAccount(alice_key))
    );
    assert_eq!(session.revision(), initial_revision + 5);
    assert!(!changes.has_changed().expect("refusals do not signal"));
}

#[test]
fn signer_attachment_adds_an_account_and_removal_leaves_it_retained() {
    let alice_key = Keys::generate().public_key();
    let signer = Arc::new(TestSigner(alice_key)) as Arc<dyn Signer>;
    let session = Session::new(std::iter::empty()).expect("empty session");

    session
        .add_signer(signer)
        .expect("signer adds its missing account");
    assert_eq!(session.accounts(), vec![alice_key]);
    let attachment_generation = session.signer(alice_key).expect("signer is attached").0;
    assert_eq!(attachment_generation, session.revision());

    session
        .remove_signer(alice_key)
        .expect("signer detaches without removing account");
    assert_eq!(session.accounts(), vec![alice_key]);
    assert!(session.signer(alice_key).is_none());
}

#[test]
fn current_account_snapshot_never_mixes_a_selection_with_another_revision() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let session = Session::new(std::iter::empty()).expect("empty session");
    session.add_account(alice).expect("Alice adds");
    session.add_account(bob).expect("Bob adds");
    session
        .select_account(alice)
        .expect("Alice selects at revision three");
    let writing = Arc::new(AtomicBool::new(true));
    let writer_session = session.clone();
    let writer_active = Arc::clone(&writing);
    let writer = std::thread::spawn(move || {
        for _ in 0..10_000 {
            writer_session.select_account(bob).expect("Bob selects");
            writer_session.select_account(alice).expect("Alice selects");
        }
        writer_active.store(false, Ordering::SeqCst);
    });

    while writing.load(Ordering::SeqCst) {
        let (current, revision) = session.current_account_snapshot();
        let expected = if revision % 2 == 0 { bob } else { alice };
        assert_eq!(current, Some(expected));
    }
    writer.join().expect("selection writer completes");
}

#[test]
fn account_capacity_refuses_without_mutating_existing_selection() {
    let session = Session::new(std::iter::empty()).expect("empty session");
    let mut keys: Vec<_> = (0..64).map(|_| Keys::generate().public_key()).collect();
    keys.sort();
    for key in &keys {
        session.add_account(*key).expect("bounded account adds");
    }
    session
        .select_account(keys[0])
        .expect("selected account remains current");
    let revision = session.revision();
    let overflow = Keys::generate().public_key();

    assert_eq!(
        session.add_account(overflow),
        Err(SessionError::AccountCapacityExceeded { limit: 64 })
    );
    assert_eq!(session.accounts(), keys);
    assert_eq!(session.current_account(), Some(keys[0]));
    assert_eq!(session.revision(), revision);
}

struct TestSigner(PublicKey);

struct InvocationSigner {
    public_key: PublicKey,
    calls: AtomicU64,
}

impl InvocationSigner {
    fn new(public_key: PublicKey) -> Self {
        Self {
            public_key,
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for InvocationSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        _event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::pending())
    }
}

impl Signer for TestSigner {
    fn public_key(&self) -> PublicKey {
        self.0
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        _event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(std::future::pending())
    }
}
