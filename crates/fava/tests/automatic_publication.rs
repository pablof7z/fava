//! Public-facade evidence for partial automatic publication routing.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{EventBuilder, EventValue, Fava, ReceiptId, ReceiptOutcome, all};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_router_app_relays::AppRelayRouter;
use fava_router_testkit::DelayedRouter;
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_signer_local::LocalSigner;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_write::{Kind, Tag};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn known_destinations_deliver_now_and_later_route_uses_same_receipt() {
    let author = Keys::generate();
    let recipients = [Keys::generate(), Keys::generate(), Keys::generate()];
    let known_a = relay("known-a");
    let known_b = relay("known-b");
    let later = relay("later");
    let app = relay("app");
    let initial = contribution(
        &[
            (known_a.clone(), RouteTarget::Author(author.public_key())),
            (
                known_a.clone(),
                RouteTarget::Recipient(recipients[0].public_key()),
            ),
            (
                known_b.clone(),
                RouteTarget::Recipient(recipients[1].public_key()),
            ),
        ],
        [RouteTarget::Recipient(recipients[2].public_key())],
    );
    let delayed = Arc::new(DelayedRouter::new("outbox-test", initial));
    let publisher = Arc::new(RecordingPublisher::default());
    let store = Arc::new(MemoryWriteStore::default());
    let routers: Vec<Arc<dyn Router>> = vec![
        delayed.clone(),
        Arc::new(AppRelayRouter::new("app-relays", [app.clone()])),
    ];
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::clone(&store))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .routers(routers.clone())
        .signer(Arc::new(LocalSigner::new(author.clone())))
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("assembly");
    let event = EventBuilder::new(author.public_key(), Kind::TextNote)
        .tags(
            recipients
                .iter()
                .map(|keys| Tag::parse(["p", &keys.public_key().to_hex()]).expect("p tag")),
        )
        .build()
        .expect("event");
    let preview = fava_routing::preview(
        &routers,
        &RouteRequest::Write(EventValue::Unsigned(event.clone())),
    )
    .expect("routing-provider preview");
    assert!(!preview.settled());
    assert_eq!(preview.destinations.len(), 3);
    assert_eq!(delayed.open_count(), 0);
    assert_eq!(publisher.count(), 0);
    assert!(store.is_empty().expect("store readable"));

    let write = fava.publish(event).expect("accepted");
    wait_until(|| publisher.count() == 3).await;
    let partial = write.receipt().expect("receipt exists");
    assert_eq!(partial.receipt_id, write.receipt_id());
    assert_eq!(partial.outcome, ReceiptOutcome::Open);
    assert!(!partial.route_settled);
    assert_eq!(partial.route_revision, 1);
    assert_eq!(
        partial.desired_destinations,
        preview.destinations.keys().cloned().collect()
    );

    delayed.replace(contribution(
        &[
            (known_a.clone(), RouteTarget::Author(author.public_key())),
            (known_a, RouteTarget::Recipient(recipients[0].public_key())),
            (known_b, RouteTarget::Recipient(recipients[1].public_key())),
            (later, RouteTarget::Recipient(recipients[2].public_key())),
        ],
        [],
    ));
    let terminal = tokio::time::timeout(Duration::from_secs(1), write.settled(all()))
        .await
        .expect("terminal deadline elapsed")
        .expect("receipt settles");

    assert_eq!(terminal.receipt_id, write.receipt_id());
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert!(terminal.route_settled);
    assert_eq!(terminal.route_revision, 2);
    assert_eq!(terminal.destinations().len(), 4);
    assert_eq!(publisher.count(), 4);
    assert!(publisher.all_once_under(write.receipt_id()));
}

#[tokio::test(flavor = "current_thread")]
async fn a_router_refusing_to_open_leaves_the_write_open_with_an_attributed_shortfall() {
    let author = Keys::generate();
    let recipient = Keys::generate();
    let quiet = Arc::new(DelayedRouter::new(
        "quiet-test",
        RouteContribution::default(),
    ));
    let refusing = Arc::new(RefusingRouter::new("refusing-test"));
    let publisher = Arc::new(RecordingPublisher::default());
    let routers: Vec<Arc<dyn Router>> = vec![quiet.clone(), refusing];
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .routers(routers)
        .signer(Arc::new(LocalSigner::new(author.clone())))
        .publisher(Arc::clone(&publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("assembly");
    let event = EventBuilder::new(author.public_key(), Kind::TextNote)
        .tags([Tag::parse(["p", &recipient.public_key().to_hex()]).expect("p tag")])
        .build()
        .expect("event");

    let write = fava.publish(event).expect("accepted");
    wait_until(|| {
        write
            .receipt()
            .is_ok_and(|receipt| receipt.route_revision >= 1)
    })
    .await;
    let receipt = write.receipt().expect("receipt exists");

    assert_eq!(
        receipt.outcome,
        ReceiptOutcome::Open,
        "a router that never answered cannot make the write terminal: {:?}",
        receipt.route_shortfalls
    );
    assert!(
        !receipt.route_settled,
        "settled absence requires every configured router to have answered"
    );
    assert!(
        receipt
            .route_shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("refusing-test")),
        "the refusal must be attributed to the router instance: {:?}",
        receipt.route_shortfalls
    );
    assert_eq!(publisher.count(), 0);
    assert_eq!(quiet.open_count(), 1);
}

struct RefusingRouter {
    name: String,
}

impl RefusingRouter {
    fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Router for RefusingRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Err(RouterError::Refused("test refusal".to_owned()))
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        Err(RouterError::Refused("test refusal".to_owned()))
    }
}

fn contribution(
    values: &[(RelayUrl, RouteTarget)],
    unresolved: impl IntoIterator<Item = RouteTarget>,
) -> RouteContribution {
    let mut coverage = BTreeMap::new();
    let mut destinations = Vec::new();
    for (relay, target) in values {
        let session = RelaySessionKey::new(relay.clone(), RelayAccess::public());
        coverage.insert(
            target.clone(),
            CoverageState::Covered(BTreeSet::from([session.clone()])),
        );
        destinations.push(RouteDestination::new(
            session,
            BTreeSet::from([target.clone()]),
            "test route",
        ));
    }
    let unresolved: BTreeSet<_> = unresolved.into_iter().collect();
    for target in &unresolved {
        coverage.insert(target.clone(), CoverageState::Unresolved);
    }
    RouteContribution {
        destinations,
        coverage,
        unresolved,
        shortfalls: Vec::new(),
    }
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

#[derive(Default)]
struct RecordingPublisher {
    attempts: Mutex<Vec<PublishAttempt>>,
}

impl RecordingPublisher {
    fn count(&self) -> usize {
        self.attempts.lock().expect("attempt lock").len()
    }

    fn all_once_under(&self, receipt_id: ReceiptId) -> bool {
        let attempts = self.attempts.lock().expect("attempt lock");
        attempts
            .iter()
            .all(|attempt| attempt.receipt_id == receipt_id)
            && attempts
                .iter()
                .map(|attempt| attempt.session.clone())
                .collect::<BTreeSet<_>>()
                .len()
                == attempts.len()
    }
}

impl Publisher for RecordingPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        self.attempts.lock().expect("attempt lock").push(attempt);
        Box::pin(async {
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            }
        })
    }
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
