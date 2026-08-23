//! Settled absence is a routing answer, never a routing failure.
//!
//! `CoverageState::SettledAbsent` is a positive fact: relevant routing
//! knowledge settled and produced no destination. A router that refused,
//! panicked, or was never asked declared nothing, so a target it never
//! mentioned stays `Unresolved` and the plan stays unsettled.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_routing::{
    CoverageState, RouteContribution, RoutePlan, RouteRequest, RouteTarget, Router, RouterError,
    RouterSession,
};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey};
use tokio::sync::watch;

#[tokio::test(flavor = "current_thread")]
async fn a_router_that_never_answered_cannot_produce_settled_absence() {
    let request = write_request();
    let target = author_target();
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(StaticRouter::new("quiet", RouteContribution::default())),
        Arc::new(RefusingRouter::new("refuser")),
    ];

    let session = fava_routing::open(&routers, &request).expect("chain opens past the refusal");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert_eq!(
        plan.coverage.get(&target),
        Some(&CoverageState::Unresolved),
        "a target no router answered for is outstanding, not settled absent"
    );
    assert!(
        !plan.settled,
        "the chain cannot settle while a configured router never answered"
    );
    assert!(
        plan.unresolved.contains(&target),
        "the outstanding target must also appear in the unresolved set"
    );
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("refuser") && shortfall.contains("refused work")),
        "the failure must stay attributed to its router instance: {:?}",
        plan.shortfalls
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_chain_whose_routers_all_answer_absence_still_settles() {
    let request = write_request();
    let target = author_target();
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(StaticRouter::new("absent-a", answering_absence(&target))),
        Arc::new(StaticRouter::new("absent-b", answering_absence(&target))),
    ];

    let session = fava_routing::open(&routers, &request).expect("chain opens");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert_eq!(
        plan.coverage.get(&target),
        Some(&CoverageState::SettledAbsent),
        "an answered absence is settled absence"
    );
    assert!(
        plan.settled,
        "every configured router answered, so the plan settles"
    );
    assert!(plan.unresolved.is_empty());
    assert!(plan.shortfalls.is_empty(), "{:?}", plan.shortfalls);
}

#[tokio::test(flavor = "current_thread")]
async fn a_chain_with_no_configured_routers_settles_absent() {
    let request = write_request();
    let target = author_target();

    let plan = fava_routing::preview(&[], &request).expect("an empty chain is a valid chain");

    assert_eq!(
        plan.coverage.get(&target),
        Some(&CoverageState::SettledAbsent),
        "no configured router is vacuously every router answering"
    );
    assert!(plan.settled);
    assert!(plan.unresolved.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn the_upstream_plan_never_reports_an_unasked_target_as_settled_absent() {
    let request = write_request();
    let target = author_target();
    let observer = Arc::new(UpstreamObserver::new("downstream"));
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(StaticRouter::new("quiet", RouteContribution::default())),
        observer.clone(),
    ];

    let session = fava_routing::open(&routers, &request).expect("chain opens");
    drop(session);
    let upstream = observer.seen().expect("the observer router was opened");

    assert_eq!(
        upstream.coverage.get(&target),
        Some(&CoverageState::Unresolved),
        "the observing router has not answered yet, so its upstream view is outstanding"
    );
    assert!(
        !upstream.settled,
        "an upstream view built from earlier routers only can never claim settlement"
    );
}

fn author() -> PublicKey {
    PublicKey::parse(&format!("{:064x}", 7_u32)).expect("author key")
}

fn author_target() -> RouteTarget {
    RouteTarget::Author(author())
}

/// One write with exactly one coverage target, so every assertion is exact.
fn write_request() -> RouteRequest {
    let event = EventBuilder::new(author(), Kind::TextNote)
        .build()
        .expect("event");
    let request = RouteRequest::Write(EventValue::Unsigned(event));
    assert_eq!(
        request.targets(),
        BTreeSet::from([author_target()]),
        "the fixture must carry exactly one nameable target"
    );
    request
}

/// A contribution that *answers* absence, as opposed to an empty default.
fn answering_absence(target: &RouteTarget) -> RouteContribution {
    RouteContribution {
        destinations: Vec::new(),
        coverage: BTreeMap::from([(target.clone(), CoverageState::SettledAbsent)]),
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

/// A router that records the upstream plan the chain handed it at `open`.
struct UpstreamObserver {
    name: String,
    seen: Mutex<Option<RoutePlan>>,
}

impl UpstreamObserver {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            seen: Mutex::new(None),
        }
    }

    fn seen(&self) -> Option<RoutePlan> {
        self.seen.lock().expect("upstream lock").clone()
    }
}

impl Router for UpstreamObserver {
    fn name(&self) -> &str {
        &self.name
    }

    fn preview(
        &self,
        _request: &RouteRequest,
        upstream: &RoutePlan,
    ) -> Result<RouteContribution, RouterError> {
        *self.seen.lock().expect("upstream lock") = Some(upstream.clone());
        Ok(RouteContribution::default())
    }

    fn open(
        &self,
        _request: RouteRequest,
        upstream: watch::Receiver<Arc<RoutePlan>>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        *self.seen.lock().expect("upstream lock") = Some(upstream.borrow().as_ref().clone());
        Ok(Box::new(StaticSession {
            contribution: RouteContribution::default(),
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
