//! Per-router failure isolation and collapse reporting for the ordered chain.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use fava_query::Query;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use fava_write::EventId;
use nostr::types::RelayUrl;
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn refusing_router_degrades_the_plan_instead_of_denying_it() {
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(StaticRouter::new("app", covering("app"))),
        Arc::new(BrokenRouter::new("broken", Break::Refuse)),
    ];
    let request = RouteRequest::Read(Query::events());

    let session = fava_routing::open(&routers, &request).expect("chain opens without the refuser");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert!(
        plan.destinations.contains_key(&session_key("app")),
        "a refusing router must not withdraw another router's destinations"
    );
    assert!(
        plan.shortfalls.iter().any(|shortfall| {
            shortfall.contains("broken") && shortfall.contains("router refused work")
        }),
        "the refusal must survive as an attributed shortfall: {:?}",
        plan.shortfalls
    );

    let preview = fava_routing::preview(&routers, &request).expect("preview degrades too");
    assert!(preview.destinations.contains_key(&session_key("app")));
    assert!(
        preview
            .shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("broken"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn panicking_router_open_is_isolated_as_an_attributed_shortfall() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(StaticRouter::new("app", covering("app"))),
        Arc::new(BrokenRouter::new("exploding", Break::Panic)),
    ];
    let request = RouteRequest::Read(Query::events());

    let session = fava_routing::open(&routers, &request).expect("chain opens past the panic");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");
    std::panic::set_hook(previous);

    assert!(plan.destinations.contains_key(&session_key("app")));
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("exploding") && shortfall.contains("panicked")),
        "a panicking router must be attributed, not propagated: {:?}",
        plan.shortfalls
    );
}

#[tokio::test(flavor = "current_thread")]
async fn router_that_settles_and_closes_keeps_its_contributed_demand() {
    let closing = Arc::new(ControlledRouter::new("nip65", covering("stable")));
    let routers: Vec<Arc<dyn Router>> = vec![closing.clone()];
    let request = RouteRequest::Read(Query::events());
    let mut session = fava_routing::open(&routers, &request).expect("chain opens");
    assert!(
        RoutePlan::from_contribution(1, &session.current())
            .unwrap()
            .destinations
            .contains_key(&session_key("stable"))
    );

    closing.finish();
    let changed = tokio::time::timeout(std::time::Duration::from_secs(2), session.next_change())
        .await
        .expect("chain collapse must be reported, not left silent")
        .expect("a settled router close is not a chain failure");
    let plan = RoutePlan::from_contribution(2, &changed).expect("bounded plan");

    assert!(
        plan.destinations.contains_key(&session_key("stable")),
        "unchanged destinations must keep running after a router settles and closes"
    );
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("nip65") && shortfall.contains("closed")),
        "collapse must be a reported fact: {:?}",
        plan.shortfalls
    );

    let quiet = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        Box::pin(session.next_change()),
    )
    .await;
    assert!(
        quiet.is_err(),
        "a collapsed chain must stop changing, not report an error that withdraws demand"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn combined_bound_overflow_is_a_shortfall_not_a_spawned_panic() {
    let controlled = Arc::new(ControlledRouter::new("head", RouteContribution::default()));
    let mut routers: Vec<Arc<dyn Router>> = vec![controlled.clone()];
    for index in 1..32_u32 {
        routers.push(Arc::new(StaticRouter::new(
            format!("filler-{index}"),
            coverage_block(index),
        )));
    }
    let request = RouteRequest::Read(Query::events().ids(request_ids()));
    let mut session = fava_routing::open(&routers, &request).expect("chain opens under bound");

    controlled.replace(coverage_block(0));
    let changed = tokio::time::timeout(std::time::Duration::from_secs(2), session.next_change())
        .await
        .expect("an over-bound update must not kill the chain")
        .expect("an over-bound update must not close the chain");

    assert!(
        changed
            .shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("head") && shortfall.contains("bound")),
        "the rejected contribution must be an exact attributed shortfall: {:?}",
        changed.shortfalls.len()
    );
    assert!(
        fava_routing::RoutePlan::from_contribution(2, &changed).is_ok(),
        "the chain must never publish a contribution that exceeds its own bounds"
    );
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay url")
}

fn session_key(name: &str) -> RelaySessionKey {
    RelaySessionKey {
        relay: relay(name),
        access: RelayAccess::Public,
    }
}

fn covering(name: &str) -> RouteContribution {
    RouteContribution {
        destinations: vec![RouteDestination::new(
            session_key(name),
            BTreeSet::from([RouteTarget::WholeRequest]),
            "test route",
        )],
        coverage: BTreeMap::from([(
            RouteTarget::WholeRequest,
            CoverageState::Covered(BTreeSet::from([session_key(name)])),
        )]),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}

fn event_id(index: u32) -> EventId {
    EventId::parse(&format!("{index:064x}")).expect("event id")
}

fn request_ids() -> Vec<EventId> {
    (0..257_u32)
        .map(|index| event_id(1_000_000 + index))
        .collect()
}

fn coverage_block(router: u32) -> RouteContribution {
    RouteContribution {
        destinations: Vec::new(),
        coverage: (0..256_u32)
            .map(|entry| {
                (
                    RouteTarget::ReferencedEvent(event_id(router * 256 + entry)),
                    CoverageState::SettledAbsent,
                )
            })
            .collect(),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}

struct StaticRouter {
    name: String,
    contribution: RouteContribution,
}

impl StaticRouter {
    fn new(name: impl Into<String>, contribution: RouteContribution) -> Self {
        Self {
            name: name.into(),
            contribution,
        }
    }
}

impl Router for StaticRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.contribution.clone())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        Ok(Box::new(StaticSession {
            contribution: self.contribution.clone(),
        }))
    }
}

struct StaticSession {
    contribution: RouteContribution,
}

impl RouterSession for StaticSession {
    fn current(&self) -> RouteContribution {
        self.contribution.clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(std::future::pending())
    }

    fn close(&mut self) {}
}

enum Break {
    Refuse,
    Panic,
}

struct BrokenRouter {
    name: String,
    failure: Break,
}

impl BrokenRouter {
    fn new(name: impl Into<String>, failure: Break) -> Self {
        Self {
            name: name.into(),
            failure,
        }
    }
}

impl Router for BrokenRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        match self.failure {
            Break::Refuse => Err(RouterError::Refused("test refusal".to_owned())),
            Break::Panic => panic!("deliberate router preview panic"),
        }
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        match self.failure {
            Break::Refuse => Err(RouterError::Refused("test refusal".to_owned())),
            Break::Panic => panic!("deliberate router open panic"),
        }
    }
}

struct ControlledRouter {
    name: String,
    current: watch::Sender<Arc<RouteContribution>>,
    finished: watch::Sender<bool>,
}

impl ControlledRouter {
    fn new(name: impl Into<String>, initial: RouteContribution) -> Self {
        let (current, _) = watch::channel(Arc::new(initial));
        let (finished, _) = watch::channel(false);
        Self {
            name: name.into(),
            current,
            finished,
        }
    }

    fn replace(&self, contribution: RouteContribution) {
        self.current.send_replace(Arc::new(contribution));
    }

    fn finish(&self) {
        self.finished.send_replace(true);
    }
}

impl Router for ControlledRouter {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        _upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        Ok(self.current.borrow().as_ref().clone())
    }

    fn open(
        &self,
        _request: RouteRequest,
        _upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        Ok(Box::new(ControlledSession {
            current: self.current.subscribe(),
            finished: self.finished.subscribe(),
        }))
    }
}

struct ControlledSession {
    current: watch::Receiver<Arc<RouteContribution>>,
    finished: watch::Receiver<bool>,
}

impl RouterSession for ControlledSession {
    fn current(&self) -> RouteContribution {
        self.current.borrow().as_ref().clone()
    }

    fn next_change(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<RouteContribution, RouterError>> + Send + '_>> {
        Box::pin(async move {
            tokio::select! {
                changed = self.finished.changed() => {
                    if changed.is_err() || *self.finished.borrow_and_update() {
                        return Err(RouterError::Closed);
                    }
                    Err(RouterError::Closed)
                }
                changed = self.current.changed() => {
                    changed.map_err(|_| RouterError::Closed)?;
                    Ok(self.current.borrow_and_update().as_ref().clone())
                }
            }
        })
    }

    fn close(&mut self) {}
}
