//! Public-facade evidence for ordered asynchronous automatic routing.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_app_relays::AppRelayRouter;
use fava_router_fallback_relays::FallbackRelayRouter;
use fava_router_testkit::DelayedRouter;
use fava_routing::{CoverageState, RouteContribution, RouteDestination, RouteTarget};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, OperationGeneration, RelayInboundFuture,
    RelayMessageStream, RelaySession, RelaySessionFuture, RelaySessionIdentity, ReleaseFuture,
    ReleaseOutcome, Transport, TransportFailure, TransportShutdownFuture,
};
use fava_transport_testkit::detached_lease;
use fava_wire::ClientMessage;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[derive(Default)]
struct RecordingTransport {
    next_generation: AtomicU64,
    sessions: Mutex<Vec<Arc<RecordingSession>>>,
}

impl RecordingTransport {
    fn open_count(&self, relay: &RelayUrl) -> usize {
        self.sessions
            .lock()
            .expect("transport lock")
            .iter()
            .filter(|session| &session.identity.key.relay == relay)
            .count()
    }

    fn requested(&self, relay: &RelayUrl) -> bool {
        self.sessions
            .lock()
            .expect("transport lock")
            .iter()
            .filter(|session| &session.identity.key.relay == relay)
            .any(|session| {
                session
                    .sent
                    .lock()
                    .expect("session lock")
                    .iter()
                    .any(|frame| {
                        matches!(
                            serde_json::from_str::<ClientMessage<'static>>(frame),
                            Ok(ClientMessage::Req { .. })
                        )
                    })
            })
    }
}

impl Transport for RecordingTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        Box::pin(async move {
            let session = Arc::new(RecordingSession {
                identity: RelaySessionIdentity {
                    key: request.key,
                    generation: OperationGeneration(
                        self.next_generation.fetch_add(1, Ordering::SeqCst) + 1,
                    ),
                },
                sent: Mutex::new(Vec::new()),
                closed: AtomicBool::new(false),
            });
            self.sessions
                .lock()
                .expect("transport lock")
                .push(Arc::clone(&session));
            Ok(detached_lease(session as Arc<dyn RelaySession>))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

struct RecordingSession {
    identity: RelaySessionIdentity,
    sent: Mutex<Vec<String>>,
    closed: AtomicBool,
}

/// The recording fake never delivers inbound items.
struct SilentStream;

impl RelayMessageStream for SilentStream {
    fn next_inbound(&mut self) -> RelayInboundFuture<'_> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}

impl RelaySession for RecordingSession {
    fn identity(&self) -> RelaySessionIdentity {
        self.identity.clone()
    }

    fn send(
        &self,
        frame: Vec<u8>,
        correlation: HandoffCorrelation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                HandoffOutcome::NotHandedOff {
                    identity: self.identity.clone(),
                    correlation,
                    reason: TransportFailure::SessionClosed,
                }
            } else {
                self.sent
                    .lock()
                    .expect("session lock")
                    .push(String::from_utf8_lossy(&frame).into_owned());
                HandoffOutcome::HandedOff {
                    identity: self.identity.clone(),
                    correlation,
                }
            }
        })
    }

    fn messages(&self) -> Box<dyn RelayMessageStream> {
        Box::new(SilentStream)
    }

    fn close(&self) -> ReleaseFuture<'_> {
        Box::pin(async move {
            self.closed.store(true, Ordering::SeqCst);
            Ok(ReleaseOutcome::Closed)
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn immediate_route_starts_before_delayed_router_and_preview_opens_nothing() {
    let app_relay = relay("app");
    let later_relay = relay("later");
    let delayed = Arc::new(DelayedRouter::new("delayed", RouteContribution::default()));
    let transport = Arc::new(RecordingTransport::default());
    let fava = assembly(Arc::clone(&transport))
        .router(Arc::new(AppRelayRouter::new(
            "app-relays",
            [app_relay.clone()],
        )))
        .router(Arc::clone(&delayed))
        .build()
        .expect("assembly");
    let query = Query::events();

    let preview = fava.preview_routes(&query).expect("preview");
    assert_eq!(preview.destinations.len(), 1);
    assert_eq!(delayed.open_count(), 0);
    assert_eq!(transport.open_count(&app_relay), 0);

    let observation = tokio::time::timeout(Duration::from_millis(100), fava.observe(query))
        .await
        .expect("known route must not await delayed router")
        .expect("automatic query opens");
    wait_until(|| transport.open_count(&app_relay) == 1).await;
    assert_eq!(transport.open_count(&later_relay), 0);
    delayed.replace(contribution(&[(
        later_relay.clone(),
        RouteTarget::WholeRequest,
    )]));
    wait_until(|| transport.open_count(&later_relay) == 1).await;
    assert_eq!(transport.open_count(&app_relay), 1);
    observation.close();
}

#[test]
fn identical_relay_contributions_deduplicate_and_retain_both_reasons() {
    let relay = relay("shared");
    let fava = assembly(Arc::new(RecordingTransport::default()))
        .router(Arc::new(AppRelayRouter::new("first", [relay.clone()])))
        .router(Arc::new(AppRelayRouter::new("second", [relay.clone()])))
        .build()
        .expect("assembly");

    let preview = fava.preview_routes(&Query::events()).expect("preview");
    assert_eq!(preview.destinations.len(), 1);
    let planned = preview.destinations.values().next().expect("planned relay");
    assert_eq!(planned.reasons.len(), 2);
    assert!(planned.targets.contains(&RouteTarget::WholeRequest));
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_query_bypasses_every_automatic_router() {
    let explicit = relay("explicit");
    let delayed = Arc::new(DelayedRouter::new(
        "must-not-open",
        RouteContribution::default(),
    ));
    let transport = Arc::new(RecordingTransport::default());
    let fava = assembly(Arc::clone(&transport))
        .router(Arc::clone(&delayed))
        .build()
        .expect("assembly");
    let observation = fava
        .observe(
            Query::events()
                .from_relays([explicit.clone()])
                .expect("explicit query"),
        )
        .await
        .expect("explicit query opens");

    wait_until(|| transport.open_count(&explicit) == 1).await;
    assert_eq!(delayed.open_count(), 0);
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn fallback_retracts_when_upstream_coverage_arrives_without_restarting_other_relays() {
    let authors = [Keys::generate().public_key(), Keys::generate().public_key()];
    let stable = relay("stable");
    let later = relay("later");
    let fallback = relay("fallback");
    let initial = contribution(&[(stable.clone(), RouteTarget::Author(authors[0]))]);
    let delayed = Arc::new(DelayedRouter::new("coverage", initial));
    let transport = Arc::new(RecordingTransport::default());
    let fava = assembly(Arc::clone(&transport))
        .router(Arc::clone(&delayed))
        .router(Arc::new(FallbackRelayRouter::new(
            "fallback",
            [fallback.clone()],
            NonZeroUsize::new(1).expect("non-zero"),
        )))
        .build()
        .expect("assembly");
    let observation = fava
        .observe(
            Query::events()
                .authors(authors)
                .expect("two authors are bounded"),
        )
        .await
        .expect("automatic query opens");
    wait_until(|| transport.requested(&stable) && transport.requested(&fallback)).await;

    delayed.replace(contribution(&[
        (stable.clone(), RouteTarget::Author(authors[0])),
        (later.clone(), RouteTarget::Author(authors[1])),
    ]));
    wait_until(|| transport.requested(&later) && !holds(&fava, &fallback)).await;
    assert_eq!(
        transport.open_count(&stable),
        1,
        "a retraction elsewhere never restarts a relay Fava already holds"
    );
    observation.close();
}

/// Whether the owner still publishes a held session for one relay.
fn holds(fava: &Fava, relay: &RelayUrl) -> bool {
    fava.diagnostics()
        .relays
        .iter()
        .any(|entry| &entry.session.relay == relay)
}

fn assembly(transport: Arc<RecordingTransport>) -> fava::FavaBuilder {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(transport)
}

fn contribution(values: &[(RelayUrl, RouteTarget)]) -> RouteContribution {
    let mut coverage = BTreeMap::new();
    let destinations = values
        .iter()
        .map(|(relay, target)| {
            let session = RelaySessionKey {
                relay: relay.clone(),
                access: RelayAccess::Public,
            };
            coverage.insert(
                target.clone(),
                CoverageState::Covered(BTreeSet::from([session.clone()])),
            );
            RouteDestination::new(session, BTreeSet::from([target.clone()]), "test route")
        })
        .collect();
    RouteContribution {
        destinations,
        coverage,
        unresolved: BTreeSet::new(),
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
