//! Settled-absence behavior on the replacement router contract.

use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};

struct EmptyRouter;

struct RefusingRouter;

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

impl Router for RefusingRouter {
    fn name(&self) -> &str {
        "refusing"
    }
    fn queries(&self, _: &RouteRequest, _: &RoutePlan) -> Result<Vec<Query>, RouterError> {
        Err(RouterError::Refused("test refusal".to_owned()))
    }
    fn preview(
        &self,
        _: &RouteRequest,
        _: &RoutePlan,
        _: &[QuerySnapshot],
    ) -> Result<RouteContribution, RouterError> {
        unreachable!("a refusal cannot preview")
    }
    fn open(
        &self,
        _: RouteRequest,
        _: Arc<RoutePlan>,
        _: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        unreachable!("a refusal cannot open")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn router_that_never_answered_keeps_absence_unsettled() {
    let request = RouteRequest::Read(Query::events());
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(EmptyRouter), Arc::new(RefusingRouter)];
    let session = fava_routing::open(&routers, &request, vec![Vec::new(), Vec::new()])
        .expect("one router refusal is isolated");
    let plan = RoutePlan::from_contribution(1, &session.current()).expect("bounded plan");

    assert!(
        !plan.settled(),
        "a router that never answered cannot settle absence"
    );
    assert!(
        plan.unresolved
            .contains(&fava_routing::RouteTarget::WholeRequest)
    );
    assert!(
        plan.shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("refusing"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn empty_chain_settles_absence() {
    let request = RouteRequest::Read(Query::events());
    let plan = fava_routing::preview(&[], &request, &[]).unwrap();
    assert!(plan.settled());
}

#[tokio::test(flavor = "current_thread")]
async fn answered_empty_router_settles_absence() {
    let request = RouteRequest::Read(Query::events());
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(EmptyRouter)];
    let session = fava_routing::open(&routers, &request, vec![Vec::new()]).unwrap();
    assert!(
        RoutePlan::from_contribution(1, &session.current())
            .unwrap()
            .settled()
    );
}
