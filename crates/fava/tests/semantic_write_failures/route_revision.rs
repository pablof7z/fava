use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use fava::{Fava, Kind, Receipt, ReceiptId};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};
use fava_state::{CacheMutation, CachedEvent, RelayAccess, RelaySessionKey, RelayUrl};
use nostr::key::Keys;
use tokio::sync::{broadcast, watch};

use super::faults::FaultingWriteStore;
use super::support::{
    BlockingSigner, RecordingPublisher, TestMaterializer, automatic_intent, publication_builder,
    relay_evidence, signed_source, wait_for_signer,
};

#[tokio::test(flavor = "current_thread")]
async fn successful_reads_reconcile_dropped_materialization_and_route_changes() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let initial = relay("wss://initial-route.example");
    let router = Arc::new(QueuedRouter::new(contribution(&[initial])));
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .router(Arc::clone(&router))
    .materializer(Arc::new(TestMaterializer::new(Kind::ContactList)))
    .build()
    .unwrap();
    let accepted = fava
        .publish(automatic_intent(keys.public_key(), Kind::ContactList))
        .unwrap();
    wait_for_signer(&signer, 1).await;
    wait_for_opens(&router, 1).await;
    wait_for_receipt(&fava, accepted.receipt_id, |receipt| {
        receipt.route_revision >= 1
    })
    .await;

    let later = relay("wss://later-route.example");
    let later_contribution = contribution(std::slice::from_ref(&later));
    let barrier = Arc::new(Barrier::new(2));
    store.pause_after_next_route(Arc::clone(&barrier));
    store.drop_receipt_changes();
    store.fail_receipt_reads_after_route(1);
    let queued_router = Arc::clone(&router);
    let release = std::thread::spawn(move || {
        barrier.wait();
        queued_router.send(later_contribution);
        barrier.wait();
    });
    let successor = signed_source(&keys, Kind::ContactList, 20, "successor", &[]);
    cache
        .commit(vec![CacheMutation::Upsert(CachedEvent::new(
            successor,
            relay_evidence(),
        ))])
        .unwrap();
    wait_for_receipt(&fava, accepted.receipt_id, |receipt| {
        receipt.current.publication.materialization_id == fava::MaterializationId::from_u64(2)
    })
    .await;
    release.join().unwrap();
    wait_for_signer(&signer, 2).await;
    wait_for_opens(&router, 2).await;
    let later_session = RelaySessionKey::new(later, RelayAccess::public());
    let rematerialized = wait_for_receipt(&fava, accepted.receipt_id, |receipt| {
        receipt.destinations().contains_key(&later_session)
    })
    .await;

    let second = relay("wss://second-later-route.example");
    let baseline_commits = store.route_commits();
    router.send(contribution(&[later_session.relay.clone(), second.clone()]));
    wait_for_route_commits(&store, baseline_commits.saturating_add(1)).await;

    let second_session = RelaySessionKey::new(second, RelayAccess::public());
    let updated = wait_for_receipt(&fava, accepted.receipt_id, |receipt| {
        receipt.destinations().contains_key(&second_session)
    })
    .await;
    assert!(updated.route_revision > rematerialized.route_revision);
}

struct QueuedRouter {
    changes: broadcast::Sender<RouteContribution>,
    current: Mutex<RouteContribution>,
    opens: AtomicU64,
}

impl QueuedRouter {
    fn new(initial: RouteContribution) -> Self {
        let (changes, _) = broadcast::channel(8);
        Self {
            changes,
            current: Mutex::new(initial),
            opens: AtomicU64::new(0),
        }
    }

    fn send(&self, contribution: RouteContribution) {
        *self.current.lock().unwrap() = contribution.clone();
        let _ = self.changes.send(contribution);
    }
}

impl Router for QueuedRouter {
    fn name(&self) -> &'static str {
        "queued-route-revision"
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.current.lock().unwrap().clone())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(QueuedSession {
            current: self.current.lock().unwrap().clone(),
            changes: self.changes.subscribe(),
        }))
    }
}

struct QueuedSession {
    current: RouteContribution,
    changes: broadcast::Receiver<RouteContribution>,
}

impl RouterSession for QueuedSession {
    fn current(&self) -> RouteContribution {
        self.current.clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(async move { self.changes.recv().await.map_err(|_| RouterError::Closed) })
    }

    fn close(&mut self) {}
}

fn contribution(relays: &[RelayUrl]) -> RouteContribution {
    RouteContribution {
        destinations: relays
            .iter()
            .cloned()
            .map(|relay| {
                RouteDestination::new(
                    RelaySessionKey::new(relay, RelayAccess::public()),
                    BTreeSet::new(),
                    "controlled route revision",
                )
            })
            .collect(),
        coverage: BTreeMap::new(),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}

fn relay(value: &str) -> RelayUrl {
    RelayUrl::parse(value).unwrap()
}

async fn wait_for_opens(router: &QueuedRouter, count: u64) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while router.opens.load(Ordering::SeqCst) < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("router did not reopen for successor materialization");
}

async fn wait_for_route_commits(store: &FaultingWriteStore, count: u64) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while store.route_commits() < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("route plan did not commit");
}

async fn wait_for_receipt(
    fava: &Fava,
    receipt_id: ReceiptId,
    predicate: impl Fn(&Receipt) -> bool,
) -> Receipt {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let receipt = fava.receipt(receipt_id).unwrap().unwrap();
            if predicate(&receipt) {
                return receipt;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("receipt did not reach controlled route state")
}
