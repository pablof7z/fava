//! Route-plan binding for one observation and its later route revisions.

use std::collections::BTreeMap;
use std::sync::Arc;

use fava_query::{
    ObservationId, Query, QueryAcquisition, QueryBranchId, QuerySnapshot, RouteOrigin,
};
use fava_relay::RelaySessionKey;
use fava_routing::{RoutePlan, RouteRequest, Router, RouterSession};
use fava_subscriptions::{RelayDemand, demand_for_query};

use crate::error::ObserveError;

/// The route plan bound to one observation, plus any open router session.
pub(crate) struct RouteBinding {
    pub(crate) plan: RoutePlan,
    pub(crate) session: Option<Box<dyn RouterSession>>,
    pub(crate) inputs: Vec<Query>,
    pub(crate) origin: Origin,
}

/// Whether this observation's relays came from an explicit set or a router.
#[derive(Clone, Copy)]
pub(crate) enum Origin {
    Explicit,
    Automatic,
}

impl Origin {
    fn at(self, revision: u64) -> RouteOrigin {
        match self {
            Self::Explicit => RouteOrigin::Explicit,
            Self::Automatic => RouteOrigin::Automatic { revision },
        }
    }

    pub(crate) const fn revision_of(self, plan: &RoutePlan) -> Option<u64> {
        match self {
            Self::Explicit => None,
            Self::Automatic => Some(plan.revision),
        }
    }
}

/// Bind one query to its immediate route plan without awaiting acquisition.
#[allow(
    clippy::result_large_err,
    reason = "ObserveError names the exact source role that refused; a live-relay role carries its session identity"
)]
pub(crate) fn bind(
    query: &Query,
    routers: &[Arc<dyn Router>],
    mut snapshot: impl FnMut(&Query) -> Result<QuerySnapshot, ObserveError>,
) -> Result<RouteBinding, ObserveError> {
    let request = RouteRequest::Read(query.clone());
    match query.source().acquisition() {
        QueryAcquisition::Explicit(relays) => {
            let plan =
                RoutePlan::explicit(relays.iter().cloned(), query.access(), &request.targets())
                    .map_err(|error| ObserveError::Relay(error.to_string()))?;
            Ok(RouteBinding {
                plan,
                session: None,
                inputs: Vec::new(),
                origin: Origin::Explicit,
            })
        }
        QueryAcquisition::Automatic => {
            let declared = fava_routing::queries(routers, &request)
                .map_err(|error| ObserveError::Relay(error.to_string()))?;
            let inputs = declared
                .iter()
                .map(|queries| queries.iter().map(&mut snapshot).collect())
                .collect::<Result<Vec<Vec<_>>, _>>()?;
            let input_queries = declared.into_iter().flatten().collect();
            let session = fava_routing::open(routers, &request, &inputs)
                .map_err(|error| ObserveError::Relay(error.to_string()))?;
            match RoutePlan::from_contribution(1, &session.current()) {
                Ok(plan) => Ok(RouteBinding {
                    plan,
                    session: Some(session),
                    inputs: input_queries,
                    origin: Origin::Automatic,
                }),
                Err(error) => {
                    let mut session = session;
                    session.close();
                    Err(ObserveError::Relay(error.to_string()))
                }
            }
        }
    }
}

/// The demand one route plan implies for one observation branch.
pub(crate) fn demand_for(
    id: ObservationId,
    branch: QueryBranchId,
    query: &Query,
    plan: &RoutePlan,
    origin: Origin,
) -> BTreeMap<RelaySessionKey, (RelayDemand, RouteOrigin)> {
    let demand = demand_for_query(id, branch, query);
    plan.destinations
        .keys()
        .map(|relay| (relay.clone(), (demand.clone(), origin.at(plan.revision))))
        .collect()
}
