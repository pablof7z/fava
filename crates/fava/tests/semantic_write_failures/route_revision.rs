use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use fava::{Fava, Kind, Receipt, ReceiptId};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{Query, QuerySnapshot};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::{
    RouteContribution, RouteDestination, RoutePlan, RouteRequest, Router, RouterError,
    RouterSession,
};
use fava_state::EventStateMutation;
use fava_write::SignatureState;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use tokio::sync::{broadcast, watch};

use super::failure_support::edit;
use super::faults::FaultingWriteStore;
use super::support::{
    BlockingSigner, RecordingPublisher, TestMaterializer, WindowSigner, publication_builder,
    relay_event, relay_occurrence, signed_source,
};

#[tokio::test(flavor = "current_thread")]
async fn initial_route_commits_before_semantic_revision() {
    let keys = Keys::generate();
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(FaultingWriteStore::new());
    let initial = relay("wss://initial-route.example");
    let router = Arc::new(QueuedRouter::new(contribution(&[initial])));
    let signer = Arc::new(WindowSigner::new(keys.clone()));
    let fava = publication_builder(
        Arc::clone(&cache),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .router(Arc::clone(&router))
    .applyr(Arc::new(TestMaterializer::new(Kind::ContactList)))
    .build()
    .unwrap();
    let accepted = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .unwrap();
    wait_for_signer_calls(&signer, 1).await;
    wait_for_opens(&router, 1).await;
    wait_for_receipt(&fava, accepted.receipt_id(), |receipt| {
        receipt.route_revision >= 1
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn activation_retry_exhaustion_is_durable_attributable_and_retryable() {
    let keys = Keys::generate();
    let store = Arc::new(FaultingWriteStore::new());
    store.refuse_routes(true);
    let signer = Arc::new(BlockingSigner::new(keys.public_key()));
    let router = Arc::new(QueuedRouter::new(contribution(&[relay(
        "wss://activation-bound.example",
    )])));
    let fava = publication_builder(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&store),
        Arc::clone(&signer),
        Arc::new(RecordingPublisher::default()),
    )
    .router(router)
    .applyr(Arc::new(TestMaterializer::new(Kind::ContactList)))
    .build()
    .unwrap();
    let accepted = fava
        .by(keys.public_key())
        .publish(edit(Kind::ContactList))
        .unwrap();

    let receipt = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let receipt = fava.receipt(accepted.receipt_id()).unwrap().unwrap();
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
    .expect("activation exhaustion becomes durable");
    let SignatureState::Retryable(reason) = receipt.current.publication.signature else {
        unreachable!()
    };
    assert!(reason.contains("generation activation retry bound 257 exhausted"));
    assert!(reason.contains(&accepted.write_id().as_u64().to_string()));
    assert!(reason.contains("retry is permitted"));
    assert_eq!(signer.calls(), 0);
}

async fn wait_for_signer_calls(signer: &WindowSigner, count: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while signer.calls().len() != count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("signer window did not open");
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

    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Ok(Vec::new())
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
        _inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.current.lock().unwrap().clone())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: Arc<RoutePlan>,
        _inputs: Vec<QuerySnapshot>,
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

    fn close(&mut self) {}
}

fn contribution(relays: &[RelayUrl]) -> RouteContribution {
    RouteContribution {
        destinations: relays
            .iter()
            .cloned()
            .map(|relay| {
                RouteDestination::new(
                    RelaySessionKey {
                        relay,
                        access: RelayAccess::Public,
                    },
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
    .expect("router did not reopen for successor revision");
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
