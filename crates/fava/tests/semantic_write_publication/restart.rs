use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use fava::{EventBuilder, EventEdit, EventValue, Kind, RevisionId, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{Query, QuerySnapshot};
use fava_state::EventStateMutation;
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;

use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};

use super::support::{
    BlockingSigner, RecordingPublisher, TestApplier, publication_builder, relay_event,
    relay_occurrence, relay_url, signed_source, wait_for_revision,
};

fn edit(change: u8) -> EventEdit {
    EventEdit::new(Kind::ContactList, None, vec![change]).unwrap()
}

fn content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn memory_restart_reconciles_before_immediate_edit_and_late_source_replays_all_edits() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let first = store
        .accept_applied_edit(
            WriteIntent::edit_as(
                edit(1),
                keys.public_key(),
                fava::WriteRouting::explicit([relay_url()]).unwrap(),
            )
            .unwrap(),
            EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("edit")
                .by(keys.public_key())
                .build()
                .unwrap(),
            None,
        )
        .expect("pre-restart custody commits");
    let cache = Arc::new(MemoryEventCache::default());
    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(&keys, Kind::ContactList, 10, "restart source", &[]),
            relay_occurrence(),
        ))])
        .unwrap();
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::new(BlockingSigner::new(keys.public_key())),
        Arc::new(RecordingPublisher::default()),
    )
    .applier(Arc::clone(&applier))
    .build()
    .expect("memory recovery reconciles before exposing the facade");

    let reconciled = fava.receipt(first.receipt_id).unwrap().unwrap();
    assert_eq!(
        reconciled.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert_eq!(content(&reconciled.current.event), "restart source|edit");

    // A current-thread runtime cannot poll the spawned recovery runner between
    // build and this synchronous admission. This is the deterministic restart
    // barrier: the facade itself must already be reconciled.
    let second = fava
        .with_account(keys.public_key())
        .to([relay_url()])
        .unwrap()
        .publish(edit(2))
        .expect("immediate same-coordinate edit composes after reconciliation");
    let composed = second.receipt().unwrap();
    assert_eq!(second.write_id(), first.write_id);
    assert_eq!(second.receipt_id(), first.receipt_id);
    assert_eq!(
        composed.current.publication.revision_id,
        RevisionId::try_from(3).expect("nonzero revision identity")
    );
    assert_eq!(content(&composed.current.event), "restart source|edit|edit");

    cache
        .commit(vec![EventStateMutation::Upsert(relay_event(
            signed_source(&keys, Kind::ContactList, 20, "late source", &[]),
            relay_occurrence(),
        ))])
        .unwrap();
    let replayed = wait_for_revision(&fava, first.receipt_id, 4).await;
    assert_eq!(content(&replayed.current.event), "late source|edit|edit");
}

#[tokio::test(flavor = "current_thread")]
async fn memory_restart_reopens_router_if_generation_changes_during_session_open() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    let first = store
        .accept_applied_edit(
            WriteIntent::edit_as(edit(1), keys.public_key(), fava::WriteRouting::Automatic)
                .unwrap(),
            EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("edit")
                .by(keys.public_key())
                .build()
                .unwrap(),
            None,
        )
        .unwrap();
    let applier = Arc::new(TestApplier::new(Kind::ContactList));
    let stale = RelayUrl::parse("wss://stale-generation.example").unwrap();
    let current = RelayUrl::parse("wss://current-generation.example").unwrap();
    let router = Arc::new(ComposingRouter::new(
        Arc::clone(&store),
        keys.public_key(),
        stale.clone(),
        current.clone(),
    ));
    let fava = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::new(BlockingSigner::new(keys.public_key())),
        Arc::new(RecordingPublisher::default()),
    )
    .router(Arc::clone(&router))
    .applier(applier)
    .build()
    .unwrap();

    let receipt = wait_for_route(&fava, first.receipt_id).await;
    assert_eq!(
        receipt.current.publication.revision_id,
        RevisionId::try_from(2).expect("nonzero revision identity")
    );
    assert!(
        receipt
            .destinations()
            .contains_key(&public_session(current))
    );
    assert!(!receipt.destinations().contains_key(&public_session(stale)));
    assert_eq!(router.opens.load(Ordering::SeqCst), 2);
    assert_eq!(router.closes.load(Ordering::SeqCst), 1);
}

struct ComposingRouter {
    store: Arc<MemoryWriteStore>,
    author: fava::PublicKey,
    stale: RelayUrl,
    current: RelayUrl,
    opens: Arc<AtomicU64>,
    closes: Arc<AtomicU64>,
}

impl ComposingRouter {
    fn new(
        store: Arc<MemoryWriteStore>,
        author: fava::PublicKey,
        stale: RelayUrl,
        current: RelayUrl,
    ) -> Self {
        Self {
            store,
            author,
            stale,
            current,
            opens: Arc::new(AtomicU64::new(0)),
            closes: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl Router for ComposingRouter {
    fn name(&self) -> &'static str {
        "generation-composition-barrier"
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
        Ok(contribution(self.current.clone()))
    }

    fn open(
        &self,
        request: RouteRequest,
        _upstream: Arc<RoutePlan>,
        _inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        let open = self.opens.fetch_add(1, Ordering::SeqCst) + 1;
        let relay = if open == 1 {
            let RouteRequest::Write { event: source, .. } = &request else {
                panic!("semantic router receives a write request");
            };
            let edit = edit(2);
            let event = EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(source.created_at().as_secs() + 1))
                .content(format!("{}|edit", content(source)))
                .by(self.author)
                .build()
                .unwrap();
            let reservation = self.store.reserve_active(&edit, self.author).unwrap();
            self.store
                .accept_reserved_applied_edit(
                    reservation,
                    WriteIntent::edit_as(edit, self.author, fava::WriteRouting::Automatic).unwrap(),
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
            current: contribution(relay),
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

fn contribution(relay: RelayUrl) -> RouteContribution {
    RouteContribution {
        destinations: vec![RouteDestination::new(
            public_session(relay),
            BTreeSet::default(),
            "generation-bound route",
        )],
        coverage: BTreeMap::default(),
        unresolved: BTreeSet::default(),
        shortfalls: Vec::new(),
    }
}

fn public_session(relay: RelayUrl) -> RelayUrl {
    relay
}

async fn wait_for_route(fava: &fava::Fava, receipt_id: fava::ReceiptId) -> fava::Receipt {
    let mut changes = fava.receipt_changes();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let receipt = fava.receipt(receipt_id).unwrap().unwrap();
            if receipt.route_revision > 0 {
                return receipt;
            }
            changes.recv().await.unwrap();
        }
    })
    .await
    .expect("generation-bound route commits")
}
