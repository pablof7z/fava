//! Public-facade evidence for ordered asynchronous automatic routing.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_router_app_relays::AppRelayRouter;
use fava_router_fallback_relays::FallbackRelayRouter;
use fava_router_testkit::DelayedRouter;
use fava_routing::{CoverageState, RouteContribution, RouteDestination, RouteTarget};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::ClientMessage;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

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
            .filter(|session| &session.key.relay == relay)
            .count()
    }

    fn close_seen(&self, relay: &RelayUrl) -> bool {
        self.sessions
            .lock()
            .expect("transport lock")
            .iter()
            .filter(|session| &session.key.relay == relay)
            .any(|session| {
                session
                    .sent
                    .lock()
                    .expect("session lock")
                    .iter()
                    .any(|frame| {
                        matches!(
                            serde_json::from_str::<ClientMessage<'static>>(frame),
                            Ok(ClientMessage::Close { .. })
                        )
                    })
            })
    }
}

impl Transport for RecordingTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Arc<dyn RelaySession>, TransportError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let session = Arc::new(RecordingSession {
                key,
                generation: self.next_generation.fetch_add(1, Ordering::SeqCst) + 1,
                sent: Mutex::new(Vec::new()),
                closed: AtomicBool::new(false),
            });
            self.sessions
                .lock()
                .expect("transport lock")
                .push(Arc::clone(&session));
            Ok(session as Arc<dyn RelaySession>)
        })
    }
}

struct RecordingSession {
    key: RelaySessionKey,
    generation: u64,
    sent: Mutex<Vec<String>>,
    closed: AtomicBool,
}

impl RelaySession for RecordingSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(
        &self,
        frame: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.closed.load(Ordering::SeqCst) {
                HandoffOutcome::NotHandedOff {
                    reason: "closed".to_owned(),
                }
            } else {
                self.sent.lock().expect("session lock").push(frame);
                HandoffOutcome::HandedOff
            }
        })
    }

    fn next_message(
        &self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<String, TransportError>> + Send + '_>,
    > {
        Box::pin(std::future::pending())
    }

    fn close(
        &self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            self.closed.store(true, Ordering::SeqCst);
            Ok(())
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
    assert_eq!(transport.open_count(&app_relay), 1);
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

    assert_eq!(transport.open_count(&explicit), 1);
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
        .observe(Query::events().authors(authors))
        .await
        .expect("automatic query opens");
    wait_until(|| transport.open_count(&stable) == 1 && transport.open_count(&fallback) == 1).await;

    delayed.replace(contribution(&[
        (stable.clone(), RouteTarget::Author(authors[0])),
        (later.clone(), RouteTarget::Author(authors[1])),
    ]));
    wait_until(|| transport.open_count(&later) == 1 && transport.close_seen(&fallback)).await;
    assert_eq!(transport.open_count(&stable), 1);
    observation.close();
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
            let session = RelaySessionKey::new(relay.clone(), RelayAccess::public());
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
