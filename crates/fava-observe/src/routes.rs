//! Route-plan binding for one observation and its later route revisions.

use std::collections::BTreeMap;
use std::sync::Arc;

use fava_query::{
    ObservationId, Query, QueryAcquisition, QueryBranchId, RelayWithdrawal, RouteOrigin,
};
use fava_relay::RelaySessionKey;
use fava_routing::{RoutePlan, RouteRequest, Router, RouterSession};
use fava_runtime::{CancellationToken, Runtime, TaskHandle, TaskName};
use fava_subscriptions::{RelayDemand, demand_for_query};

use crate::error::ObserveError;
use crate::registry::Registry;

/// The route plan bound to one observation, plus any open router session.
pub(crate) struct RouteBinding {
    pub(crate) plan: RoutePlan,
    pub(crate) session: Option<Box<dyn RouterSession>>,
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
                origin: Origin::Explicit,
            })
        }
        QueryAcquisition::Automatic => {
            let session = fava_routing::open(routers, &request)
                .map_err(|error| ObserveError::Relay(error.to_string()))?;
            match RoutePlan::from_contribution(1, &session.current()) {
                Ok(plan) => Ok(RouteBinding {
                    plan,
                    session: Some(session),
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

/// Everything one observation's route-following task needs.
pub(crate) struct Following {
    /// Observation whose demand the route revisions replace.
    pub(crate) id: ObservationId,
    /// Registry the retained demand lives in.
    pub(crate) registry: Arc<Registry>,
    /// Query whose filter every destination carries.
    pub(crate) query: Query,
    /// Branch the demand belongs to.
    pub(crate) branch: QueryBranchId,
    /// Cancellation installed with the observation.
    pub(crate) cancel: CancellationToken,
    /// Revision the initial contribution produced.
    pub(crate) revision: u64,
}

/// Follow later route revisions and retain the demand each one implies.
pub(crate) fn follow(
    runtime: &Runtime,
    mut session: Box<dyn RouterSession>,
    following: Following,
) -> Option<TaskHandle<Option<()>>> {
    let Following {
        id,
        registry,
        query,
        branch,
        cancel,
        mut revision,
    } = following;
    runtime
        .spawn_cancellable(TaskName("observe.routes"), cancel, async move {
            loop {
                let Ok(contribution) = session.next_change().await else {
                    break;
                };
                revision = revision.saturating_add(1);
                let Ok(plan) = RoutePlan::from_contribution(revision, &contribution) else {
                    break;
                };
                registry.assign(
                    id,
                    branch,
                    demand_for(id, branch, &query, &plan, Origin::Automatic),
                    Some(plan.revision),
                    RelayWithdrawal::RouteWithdrawn,
                );
            }
            session.close();
        })
        .ok()
}
