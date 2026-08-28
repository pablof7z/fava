//! Failure isolation on router declaration and replacement.

use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};

struct Refusing;

impl Router for Refusing {
    fn name(&self) -> &str {
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

#[test]
fn refusal_is_a_scoped_route_shortfall() {
    let request = RouteRequest::Read(Query::events());
    let routers: Vec<Arc<dyn Router>> = vec![Arc::new(Refusing)];
    let session = fava_routing::open(&routers, &request, vec![Vec::new()]).unwrap();
    assert!(
        session
            .current()
            .shortfalls
            .iter()
            .any(|shortfall| shortfall.contains("refusing"))
    );
}
