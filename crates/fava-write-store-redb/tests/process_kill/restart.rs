use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fava_query::{Query, QuerySnapshot};
use fava_relay::RelayAccess;
use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};
use nostr::types::RelayUrl;

use super::*;

fn restart_builder(
    cache: Arc<MemoryEventCache>,
    store: Arc<RedbWriteStore>,
    applier: Arc<TestApplier>,
) -> FavaBuilder {
    Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .publisher(Arc::new(PendingPublisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .applier(applier)
}

fn content(receipt: &Receipt) -> &str {
    match &receipt.current.event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

fn publish_change(fava: &Fava, change: u8) -> fava::Write {
    fava.by(keys().public_key())
        .to([relay()])
        .unwrap()
        .publish(EventEdit::new(Kind::ContactList, None, vec![change]).unwrap())
        .expect("immediate same-coordinate edit accepts after recovery reconciliation")
}

async fn assert_restart_then_immediate_edit(
    path: PathBuf,
    persisted_edits: u64,
    immediate_change: u8,
) {
    let store = Arc::new(RedbWriteStore::open(path).expect("semantic store reopens"));
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(20, "restart source"),
            session(),
        ))])
        .unwrap();
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = restart_builder(Arc::clone(&cache), Arc::clone(&store), Arc::clone(&applier))
        .build()
        .expect("recovery reconciles before exposing the redb facade");

    let reconciled = fava
        .receipt(ReceiptId::try_from(1).expect("nonzero receipt identity"))
        .unwrap()
        .unwrap();
    assert_eq!(
        reconciled.current.publication.revision_id,
        RevisionId::try_from(persisted_edits + 1).expect("nonzero revision identity")
    );
    let expected_reconciled = (1..=persisted_edits)
        .fold("restart source".to_owned(), |body, change| {
            format!("{body}|{change}")
        });
    assert_eq!(content(&reconciled), expected_reconciled);

    // No await separates build from admission on this current-thread runtime.
    // The recovery task cannot have initialized, so the returned facade is the
    // deterministic admission barrier and must already expose reconciled state.
    let immediate = publish_change(&fava, immediate_change);
    assert_eq!(immediate.write_id().as_u64(), 1);
    assert_eq!(immediate.receipt_id().as_u64(), 1);
    assert_eq!(
        immediate.receipt().unwrap().current.publication.revision_id,
        RevisionId::try_from(persisted_edits + 2).expect("nonzero revision identity")
    );

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(40, "late source"),
            session(),
        ))])
        .unwrap();
    let replayed = wait_for_generation(
        &fava,
        ReceiptId::try_from(1).expect("nonzero receipt identity"),
        persisted_edits + 3,
    )
    .await;
    let expected_late = (1..=persisted_edits)
        .chain(std::iter::once(u64::from(immediate_change)))
        .fold("late source".to_owned(), |body, change| {
            format!("{body}|{change}")
        });
    assert_eq!(content(&replayed), expected_late);
}

struct ComposingRouter {
    store: Arc<RedbWriteStore>,
    stale: RelayUrl,
    current: RelayUrl,
    opens: Arc<AtomicU64>,
    closes: Arc<AtomicU64>,
}

impl ComposingRouter {
    fn new(store: Arc<RedbWriteStore>) -> Self {
        Self {
            store,
            stale: RelayUrl::parse("wss://stale-generation.example").unwrap(),
            current: RelayUrl::parse("wss://current-generation.example").unwrap(),
            opens: Arc::new(AtomicU64::new(0)),
            closes: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Router for ComposingRouter {
    fn name(&self) -> &'static str {
        "redb-generation-composition-barrier"
    }

    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Ok(Vec::new())
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
        _inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        Ok(route_contribution(self.current.clone()))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: Arc<RoutePlan>,
        _inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        let open = self.opens.fetch_add(1, Ordering::SeqCst) + 1;
        let relay = if open == 1 {
            let RouteRequest::Write(source) = &request else {
                panic!("semantic router receives a write request");
            };
            let next_edit = EventEdit::new(Kind::ContactList, None, vec![9]).unwrap();
            let event = EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(source.created_at().as_secs() + 1))
                .content(format!("{}|9", event_content(source)))
                .by(keys().public_key())
                .build()
                .unwrap();
            let reservation = self
                .store
                .reserve_active(&next_edit, keys().public_key())
                .unwrap();
            self.store
                .accept_reserved_applied_edit(
                    reservation,
                    WriteIntent::edit_as(next_edit, keys().public_key(), WriteRouting::Automatic)
                        .unwrap(),
                    event,
                    Some(source),
                    None,
                )
                .unwrap();
            self.stale.clone()
        } else {
            self.current.clone()
        };
        Ok(Box::new(ImmediateSession {
            current: route_contribution(relay),
            closes: Arc::clone(&self.closes),
        }))
    }
}

struct ImmediateSession {
    current: RouteContribution,
    closes: Arc<AtomicU64>,
}

impl RouterSession for ImmediateSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn replace(
        &mut self,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        if inputs.is_empty() {
            Ok(self.current())
        } else {
            Err(RouterError::Refused("unexpected router input".to_owned()))
        }
    }

    fn close(&mut self) {
        self.closes.fetch_add(1, Ordering::SeqCst);
    }
}

fn route_contribution(relay: RelayUrl) -> RouteContribution {
    RouteContribution {
        destinations: vec![RouteDestination::new(
            RelaySessionKey {
                relay,
                access: RelayAccess::Public,
            },
            BTreeSet::default(),
            "generation-bound route",
        )],
        coverage: BTreeMap::default(),
        unresolved: BTreeSet::default(),
        shortfalls: Vec::new(),
    }
}

fn event_content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

async fn assert_router_reopens_for_current_generation(path: PathBuf, generation: u64) {
    let store = Arc::new(RedbWriteStore::open(path).unwrap());
    let router = Arc::new(ComposingRouter::new(Arc::clone(&store)));
    let fava = restart_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::new(TestApplier::new(Kind::ContactList)),
    )
    .router(Arc::clone(&router))
    .build()
    .unwrap();

    let receipt = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let receipt = fava
                .receipt(ReceiptId::try_from(1).expect("nonzero receipt identity"))
                .unwrap()
                .unwrap();
            if receipt.route_revision > 0 {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("generation-bound route commits");
    assert_eq!(
        receipt.current.publication.revision_id,
        RevisionId::try_from(generation + 1).expect("nonzero revision identity")
    );
    assert!(receipt.destinations().contains_key(&RelaySessionKey {
        relay: router.current.clone(),
        access: RelayAccess::Public,
    }));
    assert!(!receipt.destinations().contains_key(&RelaySessionKey {
        relay: router.stale.clone(),
        access: RelayAccess::Public,
    }));
    assert_eq!(router.opens.load(Ordering::SeqCst), 2);
    assert_eq!(router.closes.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn redb_restart_reconciles_before_immediate_edit_and_late_source() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let root = unique_root("semantic-clean-restart-admission");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("writes.redb");
    let store = RedbWriteStore::open(&path).unwrap();
    store
        .accept_applied_edit(edit_intent(), revision(1, "1"), None)
        .unwrap();
    drop(store);

    assert_restart_then_immediate_edit(path, 1, 2).await;
}

#[tokio::test(flavor = "current_thread")]
async fn sigkill_restart_reconciles_before_immediate_edit_and_late_source() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    assert_restart_then_immediate_edit(kill_at("composed"), 2, 3).await;
}

#[tokio::test(flavor = "current_thread")]
async fn redb_restart_reopens_router_if_generation_changes_during_session_open() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    let root = unique_root("semantic-clean-restart-generation-route");
    fs::create_dir_all(&root).unwrap();
    let path = root.join("writes.redb");
    let store = RedbWriteStore::open(&path).unwrap();
    store
        .accept_applied_edit(
            WriteIntent::edit_as(edit(), keys().public_key(), WriteRouting::Automatic).unwrap(),
            revision(1, "1"),
            None,
        )
        .unwrap();
    drop(store);

    assert_router_reopens_for_current_generation(path, 1).await;
}

#[tokio::test(flavor = "current_thread")]
async fn sigkill_restart_reopens_router_if_generation_changes_during_session_open() {
    if env::var(SEMANTIC_BOUNDARY).is_ok() {
        return;
    }
    assert_router_reopens_for_current_generation(kill_at("composed-auto"), 2).await;
}
