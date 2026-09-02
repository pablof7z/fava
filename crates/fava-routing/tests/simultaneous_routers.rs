//! Shared relay ownership remains until the final router withdraws it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use fava_query::{Query, QuerySnapshot};
use fava_routing::{
    CoverageState, RouteContribution, RouteDestination, RoutePlan, RouteRequest, RouteTarget,
    Router, RouterError, RouterSession,
};
use nostr::types::RelayUrl;

#[tokio::test(flavor = "current_thread")]
async fn shared_destination_survives_first_then_second_withdrawal() {
    assert_withdrawal_order([0, 1]);
}

#[tokio::test(flavor = "current_thread")]
async fn shared_destination_survives_second_then_first_withdrawal() {
    assert_withdrawal_order([1, 0]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_router_keeps_its_destination_beside_another_router_full_of_shortfalls() {
    let first = session("first");
    let second = session("second");
    let routers: Vec<Arc<dyn Router>> = vec![
        Arc::new(ControlledRouter::new("first", complaining(first.clone()))),
        Arc::new(ControlledRouter::new("second", complaining(second.clone()))),
    ];
    let request = RouteRequest::Read(Query::events());
    let plan = fava_routing::preview(&routers, &request, &[Vec::new(), Vec::new()])
        .expect("both routers contribute");

    assert!(
        plan.destinations.contains_key(&first) && plan.destinations.contains_key(&second),
        "diagnostic volume costs no router its destination: {:?}",
        plan.destinations.keys().collect::<Vec<_>>()
    );
}

fn assert_withdrawal_order(order: [usize; 2]) {
    let shared = session("shared");
    let first = Arc::new(ControlledRouter::new("first", covering(shared.clone())));
    let second = Arc::new(ControlledRouter::new("second", covering(shared.clone())));
    let routers: Vec<Arc<dyn Router>> = vec![first.clone(), second.clone()];
    let request = RouteRequest::Read(Query::events());
    let mut chain = fava_routing::open(&routers, &request, &[Vec::new(), Vec::new()])
        .expect("both routers open");

    assert!(
        RoutePlan::from_contribution(1, &chain.current())
            .expect("initial plan")
            .destinations
            .contains_key(&shared),
        "the shared destination starts with two contributors"
    );

    let controllers = [first, second];
    controllers[order[0]].replace(RouteContribution::default());
    let after_first = chain
        .replace(Arc::new(RoutePlan::default()), Vec::new())
        .expect("first replacement");
    assert!(
        RoutePlan::from_contribution(2, &after_first)
            .expect("plan after first withdrawal")
            .destinations
            .contains_key(&shared),
        "the remaining router keeps the exact shared destination alive"
    );

    controllers[order[1]].replace(RouteContribution::default());
    let after_second = chain
        .replace(Arc::new(RoutePlan::default()), Vec::new())
        .expect("second replacement");
    assert!(
        !RoutePlan::from_contribution(3, &after_second)
            .expect("plan after final withdrawal")
            .destinations
            .contains_key(&shared),
        "only the final contributor withdraws the destination"
    );
    chain.close();
}

struct ControlledRouter {
    name: String,
    contribution: Arc<Mutex<RouteContribution>>,
}

impl ControlledRouter {
    fn new(name: impl Into<String>, contribution: RouteContribution) -> Self {
        Self {
            name: name.into(),
            contribution: Arc::new(Mutex::new(contribution)),
        }
    }

    fn replace(&self, contribution: RouteContribution) {
        *self.contribution.lock().expect("contribution lock") = contribution;
    }
}

impl Router for ControlledRouter {
    fn name(&self) -> &str {
        &self.name
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
        Ok(self.contribution.lock().expect("contribution lock").clone())
    }

    fn open(
        &self,
        _: RouteRequest,
        _: Arc<RoutePlan>,
        inputs: Vec<QuerySnapshot>,
    ) -> Result<Box<dyn RouterSession>, RouterError> {
        assert!(inputs.is_empty());
        Ok(Box::new(ControlledSession {
            contribution: Arc::clone(&self.contribution),
        }))
    }
}

struct ControlledSession {
    contribution: Arc<Mutex<RouteContribution>>,
}

impl RouterSession for ControlledSession {
    fn current(&self) -> RouteContribution {
        self.contribution.lock().expect("contribution lock").clone()
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

fn session(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay")
}

fn complaining(session: RelayUrl) -> RouteContribution {
    RouteContribution {
        shortfalls: (0..200)
            .map(|index| format!("relay list {index} undecodable"))
            .collect(),
        ..covering(session)
    }
}

fn covering(session: RelayUrl) -> RouteContribution {
    RouteContribution {
        destinations: vec![RouteDestination::new(
            session.clone(),
            BTreeSet::from([RouteTarget::WholeRequest]),
            "test",
        )],
        coverage: BTreeMap::from([(
            RouteTarget::WholeRequest,
            CoverageState::Covered(BTreeSet::from([session])),
        )]),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}
