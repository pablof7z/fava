//! Compile conformance for the sole public router/session contract.

use std::sync::Arc;

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    RouteContribution, RoutePlan, RouteRequest, Router, RouterError, RouterSession,
};

struct ConformingRouter;

impl Router for ConformingRouter {
    fn name(&self) -> &'static str {
        "conforming"
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
        Ok(Box::new(ConformingSession))
    }
}

struct ConformingSession;

impl RouterSession for ConformingSession {
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
fn router_and_session_have_only_the_approved_required_signatures() {
    let router = ConformingRouter;
    let request = RouteRequest::Read(Query::events());
    let upstream = RoutePlan::default();

    assert!(router.queries(&request, &upstream).unwrap().is_empty());
    assert!(
        router
            .preview(&request, &upstream, &[])
            .unwrap()
            .destinations
            .is_empty()
    );
    let mut session = router
        .open(request, Arc::new(upstream), Vec::new())
        .expect("approved open signature compiles");
    assert!(session.current().destinations.is_empty());
    assert!(
        session
            .replace(Arc::new(RoutePlan::default()), Vec::new())
            .expect("approved replace signature compiles")
            .destinations
            .is_empty()
    );
    session.close();

    let source = include_str!("../src/lib.rs");
    let router_contract = trait_body(source, "pub trait Router: Send + Sync {");
    let session_contract = trait_body(source, "pub trait RouterSession: Send {");
    for (contract, names) in [
        (
            router_contract,
            ["name", "queries", "preview", "open"].as_slice(),
        ),
        (session_contract, ["current", "replace", "close"].as_slice()),
    ] {
        assert!(
            !contract.contains("next_change"),
            "obsolete method remains: {contract}"
        );
        for name in names {
            assert_eq!(
                contract.matches(&format!("fn {name}(")).count(),
                1,
                "{name} must have one required declaration"
            );
        }
        assert!(
            !contract.contains("{\n        "),
            "a router contract method has a default body: {contract}"
        );
    }
}

fn trait_body<'a>(source: &'a str, start: &str) -> &'a str {
    let start = source.find(start).expect("one trait declaration");
    let body = &source[start..];
    let end = body.find("\n}").expect("trait closes");
    &body[..end]
}
