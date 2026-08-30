//! Public-facade evidence for runtime signer attachment and exact-key wakeup.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use fava::{EventBuilder, EventValue, Fava, FavaBuilder, Kind, ReceiptOutcome, all_terminal};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::RelaySessionKey;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_write::{Event, PublicKey, SignatureState, UnsignedEvent};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn signer_added_after_acceptance_wakes_same_write() {
    let alice = Keys::generate();
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly(Arc::clone(&publisher))
        .build()
        .expect("publication assembly");
    let write = fava
        .to([relay("alice")])
        .expect("route validates")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("accepted before login")
                .by(alice.public_key())
                .build()
                .expect("unsigned event builds"),
        )
        .expect("unsigned write is accepted");
    let write_id = write.write_id();
    let receipt_id = write.receipt_id();

    tokio::task::yield_now().await;
    let parked = write.receipt().expect("parked receipt remains readable");
    assert_eq!(parked.write_id, write_id);
    assert_eq!(parked.receipt_id, receipt_id);
    assert!(matches!(parked.current.event, EventValue::Unsigned(_)));
    assert!(publisher.attempts().is_empty());

    fava.add_signer(Arc::new(LocalSigner::new(alice)))
        .expect("Alice attaches to the running Fava");

    let settled = tokio::time::timeout(Duration::from_secs(1), write.settled(all_terminal()))
        .await
        .expect("runtime signer wakes the parked write")
        .expect("publication settles");
    assert_eq!(write.write_id(), write_id);
    assert_eq!(write.receipt_id(), receipt_id);
    assert_eq!(settled.write_id, write_id);
    assert_eq!(settled.receipt_id, receipt_id);
    assert_eq!(settled.outcome, ReceiptOutcome::Complete);
    assert!(matches!(settled.current.event, EventValue::Signed(_)));
    assert_eq!(publisher.attempts().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn adding_bob_does_not_wake_alice() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let alice_signer = Arc::new(BlockingSigner::new(alice.public_key()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly(Arc::clone(&publisher))
        .signer(Arc::clone(&alice_signer))
        .build()
        .expect("publication assembly");
    let write = fava
        .to([relay("alice-blocked")])
        .expect("route validates")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("Alice remains on her exact signer generation")
                .by(alice.public_key())
                .build()
                .expect("unsigned event builds"),
        )
        .expect("Alice's write is accepted");
    wait_until(|| alice_signer.calls() == 1).await;
    let before = write.receipt().expect("receipt remains readable");
    let bob_signer = Arc::new(CountingSigner::new(bob));

    fava.add_signer(Arc::clone(&bob_signer) as Arc<dyn Signer>)
        .expect("Bob attaches to the running Fava");
    tokio::time::sleep(Duration::from_millis(25)).await;

    assert_eq!(alice_signer.calls(), 1);
    assert_eq!(alice_signer.cancellations(), 0);
    assert_eq!(bob_signer.calls(), 0);
    assert_eq!(write.receipt().expect("receipt remains readable"), before);
    assert!(publisher.attempts().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn removed_signer_stale_valid_completion_is_inert_and_readd_wakes() {
    let alice = Keys::generate();
    let old_signer = Arc::new(GatedValidSigner::new(alice.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly(Arc::clone(&publisher))
        .signer(Arc::clone(&old_signer))
        .build()
        .expect("publication assembly");
    let write = fava
        .to([relay("remove-readd")])
        .expect("route validates")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("same custody survives logout and login")
                .by(alice.public_key())
                .build()
                .expect("unsigned event builds"),
        )
        .expect("Alice's write is accepted");
    let write_id = write.write_id();
    let receipt_id = write.receipt_id();
    wait_until(|| old_signer.calls() == 1).await;

    fava.remove_signer(alice.public_key())
        .expect("Alice signer removes");
    wait_until(|| old_signer.cancellations() == 1).await;
    let parked = write
        .receipt()
        .expect("cancelled authorization remains durable while its future is detached");
    let SignatureState::Retryable(reason) = parked.current.publication.signature else {
        panic!("removed signer left authorized custody orphaned")
    };
    assert!(reason.contains(&write_id.as_u64().to_string()));
    assert!(reason.contains(&receipt_id.as_u64().to_string()));
    assert!(reason.contains("removed"));
    assert!(reason.contains("retry is permitted"));
    old_signer.release();
    wait_until(|| old_signer.completions() == 1).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        write.receipt().unwrap().current.event,
        EventValue::Unsigned(_)
    ));
    assert!(publisher.attempts().is_empty());

    fava.add_signer(Arc::new(LocalSigner::new(alice)))
        .expect("Alice signer reattaches");
    let settled = write
        .settled(all_terminal())
        .await
        .expect("same write settles");
    assert_eq!(settled.write_id, write_id);
    assert_eq!(settled.receipt_id, receipt_id);
    assert_eq!(publisher.attempts().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn replaced_signer_stale_valid_completion_cannot_install_or_deliver() {
    let alice = Keys::generate();
    let old_signer = Arc::new(GatedValidSigner::new(alice.clone()));
    let new_signer = Arc::new(GatedValidSigner::new(alice.clone()));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly(Arc::clone(&publisher))
        .signer(Arc::clone(&old_signer))
        .build()
        .expect("publication assembly");
    let write = fava
        .to([relay("replace")])
        .expect("route validates")
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("only the replacement generation may complete")
                .by(alice.public_key())
                .build()
                .expect("unsigned event builds"),
        )
        .expect("Alice's write is accepted");
    wait_until(|| old_signer.calls() == 1).await;

    fava.replace_signer(Arc::clone(&new_signer) as Arc<dyn Signer>)
        .expect("replacement succeeds");
    wait_until(|| old_signer.cancellations() == 1).await;
    wait_until(|| new_signer.calls() == 1).await;
    old_signer.release();
    wait_until(|| old_signer.completions() == 1).await;
    tokio::task::yield_now().await;
    assert!(matches!(
        write.receipt().unwrap().current.event,
        EventValue::Unsigned(_)
    ));
    assert!(publisher.attempts().is_empty());

    new_signer.release();
    let settled = tokio::time::timeout(Duration::from_secs(1), write.settled(all_terminal()))
        .await
        .unwrap_or_else(|_| panic!("replacement signer stalled: {:?}", write.receipt()))
        .expect("replacement signer settles the write");
    assert!(matches!(settled.current.event, EventValue::Signed(_)));
    assert_eq!(old_signer.completions(), 1);
    assert_eq!(new_signer.completions(), 1);
    assert_eq!(publisher.attempts().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn memory_replacement_between_snapshot_and_invoke_skips_retired_signer() {
    replacement_between_snapshot_and_invoke(Arc::new(MemoryWriteStore::default())).await;
}

#[tokio::test(flavor = "current_thread")]
async fn redb_replacement_between_snapshot_and_invoke_skips_retired_signer() {
    replacement_between_snapshot_and_invoke(Arc::new(
        RedbWriteStore::open(unique_redb_path("pre-invoke-replacement")).unwrap(),
    ))
    .await;
}

async fn replacement_between_snapshot_and_invoke<W>(store: Arc<W>)
where
    W: WriteStore + 'static,
{
    let alice = Keys::generate();
    let public_key = alice.public_key();
    let selected = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let retired = Arc::new(SnapshotWindowSigner::new(
        public_key,
        Arc::clone(&selected),
        Arc::clone(&release),
    ));
    let publisher = Arc::new(RecordingPublisher::default());
    let fava = assembly_with_store(Arc::clone(&publisher), store)
        .signer(Arc::clone(&retired))
        .build()
        .expect("publication assembly");
    let replacement_fava = fava.clone();
    let replacement = std::thread::spawn(move || {
        selected.wait();
        replacement_fava
            .replace_signer(Arc::new(LocalSigner::new(alice)))
            .expect("replacement commits while the old snapshot is paused");
        release.wait();
    });
    let write = fava
        .to([relay("pre-invoke-replacement")])
        .unwrap()
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("replacement wins after snapshot and before invocation")
                .by(public_key)
                .build()
                .unwrap(),
        )
        .unwrap();

    let settled = tokio::time::timeout(Duration::from_secs(1), write.settled(all_terminal()))
        .await
        .expect("replacement signer settles")
        .expect("write remains admitted");
    replacement.join().expect("replacement thread completes");
    assert_eq!(retired.calls(), 0, "retired provider method was invoked");
    assert!(matches!(settled.current.event, EventValue::Signed(_)));
    assert_eq!(publisher.attempts().len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn memory_no_successor_cancellation_leaves_exact_retryable_custody() {
    no_successor_cancellation(Arc::new(MemoryWriteStore::default())).await;
}

#[tokio::test(flavor = "current_thread")]
async fn redb_no_successor_cancellation_leaves_exact_retryable_custody() {
    no_successor_cancellation(Arc::new(
        RedbWriteStore::open(unique_redb_path("no-successor-cancellation")).unwrap(),
    ))
    .await;
}

async fn no_successor_cancellation<W>(store: Arc<W>)
where
    W: WriteStore + 'static,
{
    let alice = Keys::generate();
    let signer = Arc::new(CancelledSigner::new(alice.public_key()));
    let fava = assembly_with_store(Arc::new(RecordingPublisher::default()), store)
        .signer(Arc::clone(&signer))
        .build()
        .expect("publication assembly");
    let write = fava
        .to([relay("cancelled-without-successor")])
        .unwrap()
        .publish(
            EventBuilder::new(Kind::TextNote)
                .content("cancelled authorization remains attributable")
                .by(alice.public_key())
                .build()
                .unwrap(),
        )
        .unwrap();

    let receipt = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let receipt = write.receipt().unwrap();
            if matches!(
                receipt.current.publication.signature,
                SignatureState::Retryable(_)
            ) {
                break receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("authorized cancellation becomes durable retryable custody");
    let SignatureState::Retryable(reason) = receipt.current.publication.signature else {
        unreachable!()
    };
    assert_eq!(signer.calls(), 1);
    assert!(reason.contains("cancelled"));
    assert!(reason.contains("retry is permitted"));
    assert_eq!(receipt.write_id, write.write_id());
    assert_eq!(receipt.receipt_id, write.receipt_id());
}

fn assembly(publisher: Arc<RecordingPublisher>) -> FavaBuilder {
    assembly_with_store(publisher, Arc::new(MemoryWriteStore::default()))
}

fn assembly_with_store<W>(publisher: Arc<RecordingPublisher>, store: Arc<W>) -> FavaBuilder
where
    W: WriteStore + 'static,
{
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
}

fn unique_redb_path(name: &str) -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "fava-r5-{name}-{}-{}.redb",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedReason::new("not used by recording publisher"),
                },
            ))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

#[derive(Default)]
struct RecordingPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl RecordingPublisher {
    fn attempts(&self) -> Vec<PublishAttempt> {
        self.attempts.lock().expect("publisher lock").clone()
    }
}

impl Publisher for RecordingPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.attempts.lock().expect("publisher lock").push(attempt);
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
        })
    }
}

struct CountingSigner {
    inner: Arc<LocalSigner>,
    calls: AtomicU64,
}

impl CountingSigner {
    fn new(keys: Keys) -> Self {
        Self {
            inner: Arc::new(LocalSigner::new(keys)),
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for CountingSigner {
    fn public_key(&self) -> PublicKey {
        self.inner.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        self.inner.availability()
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Arc::clone(&self.inner).sign_event(event, cancel)
    }
}

struct BlockingSigner {
    public_key: PublicKey,
    calls: AtomicU64,
    cancellations: AtomicU64,
}

struct SnapshotWindowSigner {
    public_key: PublicKey,
    selected: Arc<Barrier>,
    release: Arc<Barrier>,
    calls: AtomicU64,
}

impl SnapshotWindowSigner {
    fn new(public_key: PublicKey, selected: Arc<Barrier>, release: Arc<Barrier>) -> Self {
        Self {
            public_key,
            selected,
            release,
            calls: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Signer for SnapshotWindowSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        self.selected.wait();
        self.release.wait();
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        _event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Err(SignerError::Cancelled)))
    }
}

struct CancelledSigner {
    public_key: PublicKey,
    calls: AtomicU64,
}

impl CancelledSigner {
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

impl Signer for CancelledSigner {
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
        Box::pin(std::future::ready(Err(SignerError::Cancelled)))
    }
}

struct GatedValidSigner {
    inner: Arc<LocalSigner>,
    calls: AtomicU64,
    cancellations: AtomicU64,
    completions: AtomicU64,
    release: watch::Sender<bool>,
}

impl GatedValidSigner {
    fn new(keys: Keys) -> Self {
        let (release, _) = watch::channel(false);
        Self {
            inner: Arc::new(LocalSigner::new(keys)),
            calls: AtomicU64::new(0),
            cancellations: AtomicU64::new(0),
            completions: AtomicU64::new(0),
            release,
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }

    fn completions(&self) -> u64 {
        self.completions.load(Ordering::SeqCst)
    }

    fn cancellations(&self) -> u64 {
        self.cancellations.load(Ordering::SeqCst)
    }

    fn release(&self) {
        self.release.send_replace(true);
    }
}

impl Signer for GatedValidSigner {
    fn public_key(&self) -> PublicKey {
        self.inner.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut release = self.release.subscribe();
        Box::pin(async move {
            let mut cancellation_recorded = false;
            while !*release.borrow() {
                tokio::select! {
                    changed = release.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                    changed = cancel.changed(), if !cancellation_recorded => {
                        if changed.is_err() || *cancel.borrow_and_update() {
                            self.cancellations.fetch_add(1, Ordering::SeqCst);
                            cancellation_recorded = true;
                        }
                    }
                }
            }
            let (keep_uncancelled, uncancelled) = watch::channel(false);
            let result = Arc::clone(&self.inner).sign_event(event, uncancelled).await;
            drop(keep_uncancelled);
            self.completions.fetch_add(1, Ordering::SeqCst);
            result
        })
    }
}

impl BlockingSigner {
    fn new(public_key: PublicKey) -> Self {
        Self {
            public_key,
            calls: AtomicU64::new(0),
            cancellations: AtomicU64::new(0),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }

    fn cancellations(&self) -> u64 {
        self.cancellations.load(Ordering::SeqCst)
    }
}

impl Signer for BlockingSigner {
    fn public_key(&self) -> PublicKey {
        self.public_key
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        _event: UnsignedEvent,
        mut cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            if !*cancel.borrow() {
                let _ = cancel.changed().await;
            }
            self.cancellations.fetch_add(1, Ordering::SeqCst);
            Err(SignerError::Cancelled)
        })
    }
}
