//! Settled-absence behavior on the replacement router contract.

use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};

struct EmptyRouter;

impl Router for EmptyRouter {
    fn name(&self) -> &str {
        "empty"
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
        Ok(RouteContribution::default())
    }
    fn open(
        &self,
        _: RouteRequest,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        assert!(inputs.is_empty());
        Ok(Box::new(EmptySession))
    }
}

struct EmptySession;

impl RouterSession for EmptySession {
    fn current(&self) -> RouteContribution {
        RouteContribution::default()
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

#[test]
fn empty_chain_settles_absence() {
    let request = RouteRequest::Read(Query::events());
    let plan = fava_routing::preview(&[], &request, &[]).unwrap();
    assert!(plan.settled());
}

#[test]
fn answered_empty_router_settles_absence() {
    let request = RouteRequest::Read(Query::events());
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(EmptyRouter)];
    let session = fava_routing::open(&routers, &request, vec![Vec::new()]).unwrap();
    assert!(
        RoutePlan::from_contribution(1, &session.current())
            .unwrap()
            .settled()
    );
}
