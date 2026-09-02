use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::future::Future;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    EditApplier, Event, EventBuilder, EventEdit, EventValue, Fava, FavaBuilder, Kind, RevisionId,
    Tag, Timestamp, UnsignedEvent,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::Authority;
use fava_signer_local::LocalSigner;
use fava_state::{EventStateMutation, RelayEvent};
use fava_transport::{
    BoundedText, OpenRelaySession, RelaySessionFuture, Transport, TransportError, TransportFailure,
    TransportShutdownFuture,
};
use fava_write::{Receipt, ReceiptId, SignatureState, WriteIntent, WriteIntentError, WriteRouting};
use fava_write_store::WriteStore;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;

use super::{relay, session, unique_root, wait_for};

const SEMANTIC_BOUNDARY: &str = "FAVA_REDB_SEMANTIC_BOUNDARY";
const SEMANTIC_PATH: &str = "FAVA_REDB_SEMANTIC_PATH";
const SEMANTIC_MARKER: &str = "FAVA_REDB_SEMANTIC_MARKER";

fn receipt_id(value: u64) -> ReceiptId {
    ReceiptId::try_from(value).expect("nonzero receipt identity")
}

#[test]
#[allow(clippy::too_many_lines)]
fn semantic_boundary_child() {
    let Ok(boundary) = env::var(SEMANTIC_BOUNDARY) else {
        return;
    };
    let path = PathBuf::from(env::var(SEMANTIC_PATH).expect("semantic child database path"));
    let marker = PathBuf::from(env::var(SEMANTIC_MARKER).expect("semantic child marker path"));
    let store = RedbWriteStore::open(path).expect("semantic child store opens");
    let base = signed_source(10, "base");
    let intent = if matches!(boundary.as_str(), "terminal" | "composed-auto") {
        WriteIntent::edit_as(edit(), keys().public_key(), WriteRouting::Automatic)
            .expect("automatic semantic intent")
    } else {
        edit_intent()
    };
    let accepted = store
        .accept_applied_edit(
            intent,
            revision(11, "generation one"),
            matches!(boundary.as_str(), "successor" | "failed" | "retired")
                .then_some(&EventValue::Signed(base.clone())),
        )
        .expect("semantic child acceptance commits");
    match boundary.as_str() {
        "first" => {}
        "successor" | "retired" => {
            let successor = signed_source(20, "successor source");
            let created_at = if boundary == "successor" { 100 } else { 21 };
            store
                .install_revision(
                    accepted.write_id,
                    accepted.receipt_id,
                    RevisionId::FIRST,
                    Some(base.id),
                    std::slice::from_ref(&edit()),
                    revision(created_at, "generation two"),
                    Some(&EventValue::Signed(successor.clone())),
                    None,
                )
                .expect("semantic successor commits");
        }
        "failed" => {
            let failed = signed_source(20, "failed source");
            store
                .record_revision_failure(
                    accepted.write_id,
                    accepted.receipt_id,
                    RevisionId::FIRST,
                    Some(base.id),
                    Some(&EventValue::Signed(failed.clone())),
                    "child applier failure".to_owned(),
                )
                .expect("semantic failure commits");
        }
        "composed" | "composed-auto" => {
            let second_edit = EventEdit::new(Kind::ContactList, None, vec![2]).unwrap();
            let routing = if boundary == "composed-auto" {
                WriteRouting::Automatic
            } else {
                WriteRouting::explicit([relay()]).unwrap()
            };
            let composed = store
                .accept_applied_edit(
                    WriteIntent::edit_as(second_edit, keys().public_key(), routing).unwrap(),
                    revision(12, "generation one|two"),
                    Some(&accepted.current.event),
                )
                .expect("composed semantic sequence commits");
            assert_eq!(composed.write_id, accepted.write_id);
            assert_eq!(composed.receipt_id, accepted.receipt_id);
            assert_eq!(
                composed.current.publication.revision_id,
                RevisionId::try_from(2).expect("nonzero revision identity")
            );
        }
        "authorized-successor" => {
            store
                .authorize_signing(
                    accepted.write_id,
                    accepted.receipt_id,
                    RevisionId::FIRST,
                    accepted.current.id(),
                )
                .expect("pre-kill signer authorization commits");
            let second = EventEdit::new(Kind::ContactList, None, vec![2]).unwrap();
            let reservation = store.reserve_active(&second, keys().public_key()).unwrap();
            store
                .accept_reserved_applied_edit(
                    reservation,
                    WriteIntent::edit_as(
                        second,
                        keys().public_key(),
                        WriteRouting::explicit([relay()]).unwrap(),
                    )
                    .unwrap(),
                    revision(12, "generation one|two"),
                    Some(&accepted.current.event),
                    None,
                )
                .expect("post-authorization successor commits before SIGKILL");
        }
        "authorized-cancelled" => {
            store
                .authorize_signing(
                    accepted.write_id,
                    accepted.receipt_id,
                    RevisionId::FIRST,
                    accepted.current.id(),
                )
                .expect("pre-kill signer authorization commits");
            store
                .record_signer_retryable(
                    accepted.write_id,
                    accepted.receipt_id,
                    RevisionId::FIRST,
                    accepted.current.id(),
                    "authorized signer invocation cancelled before effect; retry is permitted"
                        .to_owned(),
                )
                .expect("pre-kill cancelled authorization remains retryable");
        }
        "terminal" => {
            store
                .apply_route(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.revision_id,
                    accepted.current.id(),
                    &fava::RoutePlan {
                        revision: 2,
                        destinations: BTreeMap::new(),
                        coverage: BTreeMap::new(),
                        unresolved: BTreeSet::new(),
                        shortfalls: Vec::new(),
                    },
                )
                .expect("semantic terminal state commits");
        }
        "cancelled" => {
            store
                .cancel(accepted.receipt_id)
                .expect("semantic cancellation commits");
        }
        other => panic!("unknown semantic boundary {other}"),
    }
    fs::write(marker, b"semantic-commit-durable").expect("semantic marker writes");
    loop {
        std::thread::park();
    }
}

#[test]
fn semantic_first_generation_survives_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("first");
    let store = RedbWriteStore::open(path).expect("semantic store reopens");
    let receipt = receipt_one(&store);
    assert_eq!(receipt.write_id.as_u64(), 1);
    assert_eq!(receipt.receipt_id.as_u64(), 1);
    assert_eq!(receipt.current.publication.revision_id, RevisionId::FIRST);
    assert_eq!(store.recover_applied_edits().unwrap().len(), 1);
    assert_eq!(
        store.recover_applied_edits().unwrap()[0].2,
        keys().public_key()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_successor_and_failed_source_resume_once() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let successor_path = kill_at("successor");
    let successor_store =
        Arc::new(RedbWriteStore::open(successor_path).expect("successor store reopens"));
    let successor = receipt_one(&successor_store);
    assert_eq!(
        successor.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(successor.write_id.as_u64(), 1);
    assert_eq!(successor.receipt_id.as_u64(), 1);
    let newer_source = signed_source(30, "newer post-kill source");
    let newer_source_id = newer_source.id;
    let successor_cache = Arc::new(MemoryEventCache::default());
    successor_cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            newer_source,
            session(),
        ))])
        .expect("newer source enters canonical cache");
    let successor_applier = Arc::new(TestApplier::new(Kind::ContactList));
    let successor_fava = publication_builder(
        Arc::clone(&successor_cache),
        Arc::clone(&successor_store),
        Arc::clone(&successor_applier),
    )
    .build()
    .expect("successor recovery assembles after applier validation");
    let resumed = wait_for_generation(&successor_fava, receipt_id(1), 3).await;
    assert_eq!(
        resumed.current.publication.revision_source,
        Some(newer_source_id)
    );
    wait_terminal(&successor_fava, receipt_id(1)).await;
    assert_eq!(successor_applier.calls(), 1);
    let inert_applier = Arc::new(TestApplier::new(Kind::ContactList));
    let _inert = publication_builder(successor_cache, successor_store, Arc::clone(&inert_applier))
        .build()
        .expect("settled successor store reassembles");
    tokio::task::yield_now().await;
    assert_eq!(inert_applier.calls(), 0);

    let failed_path = kill_at("failed");
    let store = Arc::new(RedbWriteStore::open(failed_path).expect("failed store reopens"));
    let failed = receipt_one(&store);
    assert!(failed.current.publication.revision_failure.is_some());
    let failed_source = signed_source(20, "failed source");
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            failed_source,
            session(),
        ))])
        .expect("failed source enters canonical cache");

    let unsupported = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(TestApplier::new(Kind::MuteList)),
    )
    .build();
    assert!(unsupported.is_err(), "unsupported durable edit assembled");

    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(Arc::clone(&cache), Arc::clone(&store), Arc::clone(&applier))
        .build()
        .expect("selected applier assembles before recovery");
    let recovered = wait_for_generation(&fava, receipt_id(1), 2).await;
    assert!(recovered.current.publication.revision_failure.is_none());
    wait_terminal(&fava, receipt_id(1)).await;
    assert_eq!(applier.calls(), 1);

    let second_applier = Arc::new(TestApplier::new(Kind::ContactList));
    let _second = publication_builder(cache, store, Arc::clone(&second_applier))
        .build()
        .expect("settled store reassembles");
    tokio::task::yield_now().await;
    assert_eq!(second_applier.calls(), 0);
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_composed_sequence_replays_after_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("composed");
    let store = Arc::new(RedbWriteStore::open(path).expect("composed store reopens"));
    let recovered = store.recover_applied_edits().unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].0.write_id.as_u64(), 1);
    assert_eq!(recovered[0].0.receipt_id.as_u64(), 1);
    assert_eq!(
        recovered[0].0.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(
        recovered[0]
            .1
            .iter()
            .map(EventEdit::change)
            .collect::<Vec<_>>(),
        vec![&[1][..], &[2][..]]
    );

    let before_incomplete = store.receipt(receipt_id(1)).unwrap();
    let incomplete_source = signed_source(20, "incomplete replay source");
    assert!(
        store
            .install_revision(
                recovered[0].0.write_id,
                recovered[0].0.receipt_id,
                recovered[0].0.current.publication.revision_id,
                recovered[0].0.current.publication.revision_source,
                std::slice::from_ref(&recovered[0].1[1]),
                revision(21, "incomplete replay source|2"),
                Some(&EventValue::Signed(incomplete_source)),
                None,
            )
            .is_err(),
        "reopened custody accepted a successor that omitted the first durable edit"
    );
    assert_eq!(store.receipt(receipt_id(1)).unwrap(), before_incomplete);

    let newer_source = signed_source(30, "newer post-kill source");
    let newer_source_id = newer_source.id;
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            newer_source,
            session(),
        ))])
        .unwrap();
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(cache, Arc::clone(&store), Arc::clone(&applier))
        .build()
        .expect("composed recovery assembles");
    let replayed = wait_for_generation(&fava, receipt_id(1), 3).await;
    assert_eq!(replayed.write_id.as_u64(), 1);
    assert_eq!(replayed.receipt_id.as_u64(), 1);
    assert_eq!(
        replayed.current.publication.revision_source,
        Some(newer_source_id)
    );
    let content = match &replayed.current.event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    };
    assert_eq!(content, "newer post-kill source|1|2");
    assert_eq!(applier.calls(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn authorized_signer_window_and_successor_resume_after_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("authorized-successor");
    let store = Arc::new(RedbWriteStore::open(path).expect("authorized store reopens"));
    let recovered = receipt_one(&store);
    assert_eq!(
        recovered.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(
        recovered.current.publication.signature,
        SignatureState::Unsigned
    );
    let successor_id = recovered.current.id();
    assert_eq!(store.recover_applied_edits().unwrap()[0].1.len(), 2);

    let fava = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::new(TestApplier::new(Kind::ContactList)),
    )
    .build()
    .expect("post-kill authorization recovery assembles");
    let successor = wait_terminal(&fava, receipt_id(1)).await;
    assert_eq!(successor.write_id.as_u64(), 1);
    assert_eq!(successor.receipt_id.as_u64(), 1);
    assert_eq!(successor.current.id(), successor_id);
    assert_eq!(
        successor.current.publication.signature,
        SignatureState::Signed
    );
}

#[tokio::test(flavor = "current_thread")]
async fn authorized_signer_window_and_successor_resume_after_clean_restart() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let root = unique_root("semantic-authorized-clean-restart");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("writes.redb");
    {
        let store = RedbWriteStore::open(&path).unwrap();
        let accepted = store
            .accept_applied_edit(edit_intent(), revision(11, "generation one"), None)
            .unwrap();
        store
            .authorize_signing(
                accepted.write_id,
                accepted.receipt_id,
                RevisionId::FIRST,
                accepted.current.id(),
            )
            .unwrap();
        let second = EventEdit::new(Kind::ContactList, None, vec![2]).unwrap();
        let reservation = store.reserve_active(&second, keys().public_key()).unwrap();
        store
            .accept_reserved_applied_edit(
                reservation,
                WriteIntent::edit_as(
                    second,
                    keys().public_key(),
                    WriteRouting::explicit([relay()]).unwrap(),
                )
                .unwrap(),
                revision(12, "generation one|two"),
                Some(&accepted.current.event),
                None,
            )
            .unwrap();
    }

    let store = Arc::new(RedbWriteStore::open(&path).unwrap());
    let recovered = receipt_one(&store);
    assert_eq!(
        recovered.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(
        recovered.current.publication.signature,
        SignatureState::Unsigned
    );
    let successor_id = recovered.current.id();
    assert_eq!(store.recover_applied_edits().unwrap()[0].1.len(), 2);
    let fava = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::new(TestApplier::new(Kind::ContactList)),
    )
    .build()
    .unwrap();
    let successor = wait_terminal(&fava, receipt_id(1)).await;
    assert_eq!(successor.current.publication.retired_revisions.len(), 1);
    assert_eq!(successor.current.id(), successor_id);
    assert_eq!(
        successor.current.publication.signature,
        SignatureState::Signed
    );
}

#[test]
fn authorized_cancellation_without_successor_survives_sigkill_exactly() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("authorized-cancelled");
    let store = RedbWriteStore::open(path).expect("cancelled authorization store reopens");
    let recovered = receipt_one(&store);
    assert_eq!(recovered.write_id.as_u64(), 1);
    assert_eq!(recovered.receipt_id.as_u64(), 1);
    assert_eq!(recovered.current.publication.revision_id, RevisionId::FIRST);
    let SignatureState::Retryable(reason) = recovered.current.publication.signature else {
        panic!("cancelled authorization reopened without retry disposition")
    };
    assert!(reason.contains("cancelled"));
    assert!(reason.contains("retry is permitted"));
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_builder_refusal_after_sigkill_preserves_every_existing_identity() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let path = kill_at("first");
    let store = Arc::new(RedbWriteStore::open(path).expect("semantic store reopens"));
    let before = receipt_one(&store);
    let cache = Arc::new(MemoryEventCache::default());
    let source = signed_source(20, "post-kill source");
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            source,
            session(),
        ))])
        .expect("post-kill source enters canonical cache");
    let applier = Arc::new(TestApplier::with_tag_count(Kind::ContactList, 2_001));
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::clone(&store))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(LocalSigner::new(keys())))
        .publisher(Arc::new(PendingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .applier(Arc::clone(&applier))
        .build()
        .expect("recovery assembles with the selected applier mode");

    let observed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(error) = applier.observed_error() {
                return error;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("recovery invokes the applier");
    assert_eq!(
        observed,
        WriteIntentError::TooManyTags {
            actual: 2_001,
            maximum: 2_000,
        }
    );
    let after = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let receipt = receipt_one(&store);
            if receipt.current.publication.revision_failure.is_some() {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("string failure evidence is recorded against the existing generation");

    assert_eq!(after.write_id, before.write_id);
    assert_eq!(after.receipt_id, before.receipt_id);
    assert_eq!(after.current.id(), before.current.id());
    assert_eq!(
        after.current.publication.revision_id,
        before.current.publication.revision_id
    );
    assert_eq!(
        after.current.publication.revision_source,
        before.current.publication.revision_source
    );
    assert_eq!(
        after.current.publication.retired_revisions,
        before.current.publication.retired_revisions
    );
    assert_eq!(
        after.current.publication.revision_id,
        RevisionId::FIRST,
        "failed reapplication installed a successor generation"
    );
    drop(fava);
}

#[test]
fn semantic_retired_and_terminal_work_stays_inert_after_sigkill() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let retired_path = kill_at("retired");
    let retired_store = RedbWriteStore::open(retired_path).expect("retired store reopens");
    let before = receipt_one(&retired_store);
    let late_source = signed_source(30, "late retired source");
    assert!(
        retired_store
            .install_revision(
                before.write_id,
                before.receipt_id,
                RevisionId::FIRST,
                before.current.publication.revision_source,
                std::slice::from_ref(&edit()),
                revision(31, "late retired completion"),
                Some(&EventValue::Signed(late_source.clone())),
                None,
            )
            .is_err()
    );
    assert_eq!(receipt_one(&retired_store), before);

    for boundary in ["terminal", "cancelled"] {
        let path = kill_at(boundary);
        let store = RedbWriteStore::open(path).expect("terminal store reopens");
        let receipt = receipt_one(&store);
        assert!(receipt.is_terminal());
        assert!(store.recover_applied_edits().unwrap().is_empty());
        assert!(
            store
                .record_revision_failure(
                    receipt.write_id,
                    receipt.receipt_id,
                    receipt.current.publication.revision_id,
                    receipt.current.publication.revision_source,
                    None,
                    "late after process death".to_owned(),
                )
                .is_err()
        );
        assert_eq!(receipt_one(&store), receipt);
    }
}

fn kill_at(boundary: &str) -> PathBuf {
    let root = unique_root(&format!("semantic-{boundary}"));
    fs::create_dir_all(&root).expect("semantic boundary directory");
    let database = root.join("writes.redb");
    let marker = root.join("semantic-committed.marker");
    let mut child = Command::new(env::current_exe().expect("test executable"))
        .args([
            "--exact",
            "semantic::semantic_boundary_child",
            "--nocapture",
        ])
        .env(SEMANTIC_BOUNDARY, boundary)
        .env(SEMANTIC_PATH, &database)
        .env(SEMANTIC_MARKER, &marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("semantic boundary child starts");
    wait_for(&marker, &mut child);
    child.kill().expect("semantic SIGKILL succeeds");
    let status = child.wait().expect("semantic child reaped");
    assert!(!status.success(), "{boundary} child must be hard-killed");
    database
}

fn receipt_one(store: &RedbWriteStore) -> Receipt {
    store
        .receipt(receipt_id(1))
        .expect("receipt read")
        .expect("receipt one survives")
}

fn edit() -> EventEdit {
    EventEdit::new(Kind::ContactList, None, vec![1]).expect("semantic edit")
}

fn edit_intent() -> WriteIntent {
    WriteIntent::edit_as(
        edit(),
        keys().public_key(),
        WriteRouting::explicit([relay()]).expect("explicit route validates"),
    )
    .expect("semantic intent")
}

fn revision(created_at: u64, body: &str) -> UnsignedEvent {
    EventBuilder::new(Kind::ContactList)
        .created_at(Timestamp::from(created_at))
        .content(body)
        .by(keys().public_key())
        .build()
        .expect("semantic revision")
}

fn signed_source(created_at: u64, body: &str) -> Event {
    revision(created_at, body)
        .finalize(&keys())
        .expect("semantic source signs")
}

fn keys() -> Keys {
    Keys::parse("0000000000000000000000000000000000000000000000000000000000000001")
        .expect("fixed semantic key")
}

fn relay_event(event: Event, _session: RelayUrl) -> RelayEvent {
    RelayEvent::new(
        event,
        session(),
        Authority::Unauthenticated,
        Timestamp::from(1),
    )
}

struct TestApplier {
    kind: Kind,
    calls: AtomicU64,
    tag_count: usize,
    observed_error: Mutex<Option<WriteIntentError>>,
}

impl TestApplier {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            calls: AtomicU64::new(0),
            tag_count: 0,
            observed_error: Mutex::new(None),
        }
    }

    fn with_tag_count(kind: Kind, tag_count: usize) -> Self {
        Self {
            kind,
            calls: AtomicU64::new(0),
            tag_count,
            observed_error: Mutex::new(None),
        }
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }

    fn observed_error(&self) -> Option<WriteIntentError> {
        self.observed_error.lock().unwrap().clone()
    }
}

impl EditApplier for TestApplier {
    fn kind(&self) -> Kind {
        self.kind
    }

    fn supports(&self, edit: &EventEdit) -> bool {
        self.kind == edit.kind()
    }

    fn apply(
        &self,
        edit: &EventEdit,
        author: fava::PublicKey,
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let source_content = source.map_or("", |event| match event {
            EventValue::Unsigned(event) => event.content.as_str(),
            EventValue::Signed(event) => event.content.as_str(),
        });
        let change = edit.change().first().copied().unwrap_or_default();
        let content = if source_content.is_empty() {
            change.to_string()
        } else {
            format!("{source_content}|{change}")
        };
        let result =
            EventBuilder::new(Kind::ContactList)
                .created_at(created_at)
                .content(content)
                .tags((0..self.tag_count).map(|index| {
                    Tag::parse(["x", &index.to_string()]).expect("ordinary applier tag")
                }))
                .by(author)
                .build()
                .map_err(WriteIntentError::from);
        if let Err(error) = &result {
            *self.observed_error.lock().unwrap() = Some(error.clone());
        }
        result
    }
}

fn publication_builder(
    cache: Arc<MemoryEventCache>,
    store: Arc<RedbWriteStore>,
    applier: Arc<TestApplier>,
) -> FavaBuilder {
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .signer(Arc::new(LocalSigner::new(keys())))
        .publisher(Arc::new(AcknowledgingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .applier(applier)
}

struct AcknowledgingPublisher;

impl Publisher for AcknowledgingPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
        })
    }
}

struct PendingPublisher;

impl Publisher for PendingPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(std::future::pending())
    }
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedText::new("publisher does not use transport"),
                },
            ))
        })
    }

    fn authentication_requests(
        &self,
    ) -> tokio::sync::broadcast::Receiver<std::sync::Arc<dyn fava_transport::RelaySession>> {
        // This double never carries a relay's demand.
        tokio::sync::broadcast::Sender::new(1).subscribe()
    }

    fn holders(&self, _relay: &RelayUrl, _authority: &Authority) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

async fn wait_for_generation(fava: &Fava, receipt_id: ReceiptId, generation: u64) -> Receipt {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let receipt = fava
                .receipt(receipt_id)
                .expect("receipt read")
                .expect("receipt retained");
            if receipt.current.publication.revision_id
                == RevisionId::try_from(generation).expect("nonzero revision identity")
            {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("recovery advances once")
}

async fn wait_terminal(fava: &Fava, receipt_id: ReceiptId) -> Receipt {
    let mut changes = fava.receipt_changes();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(receipt) = fava.receipt(receipt_id).expect("receipt read")
                && receipt.is_terminal()
            {
                return receipt;
            }
            match changes.recv().await {
                Ok((changed_id, Some(receipt)))
                    if changed_id == receipt_id && receipt.is_terminal() =>
                {
                    return receipt;
                }
                Ok((changed_id, None)) if changed_id == receipt_id => {
                    panic!("recovered receipt removed before terminal state")
                }
                Ok(_) => {}
                Err(error) => panic!("receipt change delivery failed: {error}"),
            }
        }
    })
    .await
    .expect("recovered publication settles")
}

mod restart;
