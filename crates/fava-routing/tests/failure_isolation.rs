//! Failure isolation on router declaration and replacement.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use nostr::types::RelayUrl;

struct Refusing;

struct Surviving;

impl Router for Refusing {
    fn name(&self) -> &'static str {
        "refusing"
    }
    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Err(RouterError::Refused("no".to_owned()))
    }
    fn preview(
        &self,
        _: &RouteRequest,
        _: &RoutePlan,
        _: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        unreachable!()
    }
    fn open(
        &self,
        _: RouteRequest,
        _: Arc<RoutePlan>,
        _: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        unreachable!()
    }
}

impl Router for Surviving {
    fn name(&self) -> &'static str {
        "surviving"
    }
    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Ok(Vec::new())
    }
    fn preview(
        &self,
        _: &RouteRequest,
        _: &RoutePlan,
        inputs: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        assert!(inputs.is_empty());
        Ok(contribution())
    }
    fn open(
        &self,
        _: RouteRequest,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        assert!(inputs.is_empty());
        Ok(Box::new(SurvivingSession))
    }
}

struct SurvivingSession;

impl RouterSession for SurvivingSession {
    fn current(&self) -> RouteContribution {
        contribution()
    }
    fn replace(
        &mut self,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<RouteContribution, RouterError> {
        assert!(inputs.is_empty());
        Ok(self.current())
    }
    fn close(&mut self) {}
}

#[tokio::test(flavor = "current_thread")]
async fn one_router_fails_while_another_continues() {
    let request = RouteRequest::Read(Query::events());
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(Surviving), Arc::new(Refusing)];
    let session = fava_routing::open(&routers, &request, &[Vec::new(), Vec::new()]).unwrap();
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");
    assert!(
        plan.destinations.contains_key(&relay()),
        "the surviving router keeps its destination"
    );
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("refusing"))
    );
}

fn relay() -> RelayUrl {
    RelayUrl::parse("wss://surviving.example").expect("relay")
}

fn contribution() -> RouteContribution {
    let relay = relay();
    RouteContribution {
        destinations: vec![RouteDestination::new(
            relay.clone(),
            BTreeSet::from([RouteTarget::WholeRequest]),
            "surviving",
        )],
        coverage: BTreeMap::from([(
            RouteTarget::WholeRequest,
            CoverageState::Covered(BTreeSet::from([relay])),
        )]),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}
