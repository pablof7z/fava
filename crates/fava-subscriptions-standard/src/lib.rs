//! Standard exact grouping of compatible logical relay demand.
//!
//! This planner is told the complete current demand for one relay session and
//! the set currently installed on it, and answers with a diff: what to open,
//! what to retain untouched, and what to close. It invents no relay limit —
//! every bound it honors is one the relay declared through
//! [`RelayReadConstraints`], and demand it cannot carry becomes a typed
//! [`SubscriptionShortfall`] inside a plan that still installs the rest.

mod diff;
mod grouping;
mod wire;

use std::collections::{BTreeSet, VecDeque};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    InstalledSubscriptions, PlanRevision, PlannedSubscription, RelayDemand, RelayReadConstraints,
    ShortfallReason, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
    SubscriptionShortfall,
};

/// Exact subscription planner that groups compatible author and tag filters.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandardSubscriptionPlanner;

impl StandardSubscriptionPlanner {
    /// The standard grouping policy.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl SubscriptionPlanner for StandardSubscriptionPlanner {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
        installed: &InstalledSubscriptions,
        revision: PlanRevision,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        refuse_duplicate_demand(demand)?;
        let mut shortfalls = Vec::new();
        let admissible = admit_filter_limits(demand, constraints, &mut shortfalls);
        let grouped = grouping::group(&admissible, constraints);
        let candidates = fit_message_bound(grouped, constraints, &mut shortfalls)?;
        let carried = fit_subscription_count(candidates, constraints, installed, &mut shortfalls);
        Ok(diff::assemble(
            relay,
            revision,
            carried,
            constraints,
            installed,
            shortfalls,
        ))
    }
}

/// Two demands in one call may not carry the same logical identity.
fn refuse_duplicate_demand(demand: &[RelayDemand]) -> Result<(), SubscriptionPlanError> {
    let mut seen = BTreeSet::new();
    for item in demand {
        if !seen.insert(item.id()) {
            return Err(SubscriptionPlanError::DuplicateDemand(item.id()));
        }
    }
    Ok(())
}

/// Set aside demand whose own `limit` exceeds a *declared* filter limit.
fn admit_filter_limits(
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Vec<RelayDemand> {
    let Some(maximum) = constraints.max_filter_limit.get() else {
        return demand.to_vec();
    };
    let mut admissible = Vec::with_capacity(demand.len());
    for item in demand {
        match item.filter.limit {
            Some(required) if required > maximum.get() => shortfalls.push(SubscriptionShortfall {
                demand: item.id(),
                reason: ShortfallReason::FilterLimitExceeded {
                    required,
                    maximum: maximum.get(),
                },
            }),
            _ => admissible.push(item.clone()),
        }
    }
    admissible
}

/// Split every candidate until its exact REQ fits a *declared* message bound.
///
/// Splitting undoes a merge rather than truncating it: the members are halved
/// and each half is regrouped, so every surviving REQ is still an exact
/// encoding of the demand attributed to it.
fn fit_message_bound(
    grouped: Vec<Vec<RelayDemand>>,
    constraints: &RelayReadConstraints,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Result<Vec<PlannedSubscription>, SubscriptionPlanError> {
    let mut queue: VecDeque<Vec<RelayDemand>> = grouped.into();
    let mut carried = Vec::new();
    while let Some(members) = queue.pop_front() {
        let Some(filter) = grouping::merged_filter(&members, constraints) else {
            continue;
        };
        let filters = vec![filter];
        let Some(id) = wire::identity(&filters, constraints, 0) else {
            record_id_shortfall(&members, constraints, shortfalls);
            continue;
        };
        let bytes = wire::encoded_bytes(&id, &filters)?;
        let declared = constraints.max_message_bytes.get().map(NonZeroUsize::get);
        if declared.is_none_or(|maximum| bytes <= maximum) {
            carried.push(PlannedSubscription {
                id,
                filters,
                serves: members.iter().map(RelayDemand::id).collect(),
            });
            continue;
        }
        let maximum = declared.unwrap_or(bytes);
        if members.len() == 1 {
            shortfalls.push(SubscriptionShortfall {
                demand: members[0].id(),
                reason: ShortfallReason::MessageTooLarge { bytes, maximum },
            });
            continue;
        }
        let midpoint = members.len().div_ceil(2);
        let (left, right) = members.split_at(midpoint);
        queue.extend(grouping::group(left, constraints));
        queue.extend(grouping::group(right, constraints));
    }
    Ok(carried)
}

/// Drop the candidates that do not fit a *declared* subscription count.
///
/// Already-installed candidates are kept first so a declared ceiling does not
/// churn live subscriptions; ties break on wire id, so the demand that loses is
/// the same on every replan with the same inputs.
fn fit_subscription_count(
    mut candidates: Vec<PlannedSubscription>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Vec<PlannedSubscription> {
    let Some(maximum) = constraints.max_subscriptions.get().map(NonZeroUsize::get) else {
        return candidates;
    };
    let required = candidates.len();
    if required <= maximum {
        return candidates;
    }
    candidates.sort_by(|left, right| {
        installed
            .get(&left.id)
            .is_none()
            .cmp(&installed.get(&right.id).is_none())
            .then_with(|| left.id.cmp(&right.id))
    });
    for dropped in candidates.split_off(maximum) {
        for demand in dropped.serves {
            shortfalls.push(SubscriptionShortfall {
                demand,
                reason: ShortfallReason::SubscriptionsExhausted { required, maximum },
            });
        }
    }
    candidates
}

/// Report every member of a group whose wire identity cannot be expressed.
fn record_id_shortfall(
    members: &[RelayDemand],
    constraints: &RelayReadConstraints,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) {
    let maximum = constraints
        .max_subscription_id_chars
        .get()
        .map_or(0, NonZeroUsize::get);
    for member in members {
        shortfalls.push(SubscriptionShortfall {
            demand: member.id(),
            reason: ShortfallReason::SubscriptionIdTooLong { maximum },
        });
    }
}
