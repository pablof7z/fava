//! Standard exact grouping of compatible logical relay demand.
//!
//! The planner is told the complete current demand for one relay session and
//! what is currently running on it, and answers with a diff. The two inputs
//! play entirely different roles, and keeping them apart is the whole design:
//!
//! * **demand** that no running subscription already carries is *unsent*. Only
//!   unsent demand is grouped, and only unsent demand is given wire identity.
//! * **what is running** is consulted to attach demand whose traffic is already
//!   arriving, to compute the residual subscription budget, and to find the
//!   subscriptions that have lost their last owner. It never reaches grouping
//!   and never reaches identity.
//!
//! A subscription the relay is already serving is therefore immutable: demand
//! joining never widens it and demand leaving never narrows it. The alternative
//! — recomputing a desired wire set and diffing it — tears down and re-runs a
//! completed subscription every time demand grows, and the relay traffic that
//! wastes is quadratic in the number of growth steps.
//!
//! It invents no relay limit. Every bound it honors is one the relay declared
//! through [`RelayReadConstraints`], and demand it cannot carry becomes a typed
//! [`SubscriptionShortfall`] inside a plan that still installs the rest.

mod attach;
mod diff;
mod grouping;
mod wire;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::num::NonZeroUsize;

use fava_relay::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, EoseCompleteness, InstalledSubscriptions, PlanRevision,
    RelayDemand, RelayReadConstraints, ShortfallReason, SubscriptionPlan, SubscriptionPlanError,
    SubscriptionPlanner, SubscriptionShortfall,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// Exact subscription planner that groups compatible unsent demand.
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

        let (mut attached, pending) = attach::admit(&admissible, installed);
        let grouped = grouping::group(&pending, constraints);
        let candidates = fit_message_bound(grouped, constraints, &mut shortfalls)?;
        let candidates = fold_identical_filters(candidates);
        let candidates = fold_into_running(candidates, installed, &mut attached);

        let owners = attach::surviving_owners(&admissible, installed, &attached);
        let candidates = fit_residual_count(candidates, constraints, owners.len(), &mut shortfalls);

        Ok(diff::assemble(
            relay,
            revision,
            candidates,
            constraints,
            installed,
            &owners,
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
/// Splitting undoes a merge rather than truncating one: the members are halved
/// and each half is regrouped, so every surviving REQ is still an exact
/// encoding of the demand attributed to it.
///
/// Sizing runs against a probe id of the widest identity this plan could mint,
/// so the estimate never understates the frame.
fn fit_message_bound(
    grouped: Vec<(Filter, Vec<RelayDemand>)>,
    constraints: &RelayReadConstraints,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Result<Vec<AttributedSubscription>, SubscriptionPlanError> {
    let mut queue: VecDeque<(Filter, Vec<RelayDemand>)> = grouped.into();
    let mut carried = Vec::new();
    while let Some((filter, members)) = queue.pop_front() {
        if members.is_empty() {
            continue;
        }
        let filters = vec![filter];
        let bytes = wire::encoded_bytes(&filters)?;
        let declared = constraints.max_message_bytes.get().map(NonZeroUsize::get);
        if declared.is_none_or(|maximum| bytes <= maximum) {
            carried.push(AttributedSubscription {
                completeness: completeness(&filters, constraints),
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

/// Fold candidates carrying byte-identical filters into one subscription.
///
/// Grouping already folds identical demand, but splitting for a declared
/// message bound can recreate a filter the pool already holds. Two
/// byte-identical REQs on one session are strictly worse than one: the relay
/// double-delivers, a slot is burned, and completion evidence splits.
fn fold_identical_filters(candidates: Vec<AttributedSubscription>) -> Vec<AttributedSubscription> {
    let mut folded: Vec<AttributedSubscription> = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if let Some(existing) = folded
            .iter_mut()
            .find(|seen| seen.filters == candidate.filters)
        {
            existing.serves.extend(candidate.serves);
        } else {
            folded.push(candidate);
        }
    }
    folded
}

/// Attach a candidate whose filters a running subscription already carries.
///
/// Nothing should reach here — a demand covered by a running filter attaches
/// before grouping — but a merged candidate could in principle recreate a
/// running filter, and opening it would put two byte-identical REQs on one
/// session.
fn fold_into_running(
    candidates: Vec<AttributedSubscription>,
    installed: &InstalledSubscriptions,
    attached: &mut BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
) -> Vec<AttributedSubscription> {
    let mut kept = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let running = installed
            .ids()
            .find(|id| {
                installed
                    .get(id)
                    .is_some_and(|entry| entry.filters == candidate.filters)
            })
            .cloned();
        match running {
            Some(id) => attached.entry(id).or_default().extend(candidate.serves),
            None => kept.push(candidate),
        }
    }
    kept
}

/// Drop the candidates that do not fit the *residual* subscription budget.
///
/// The budget a plan may spend is the declared maximum less what is already
/// running and still wanted. A running subscription is never closed to make
/// room for a new one, so a relay that lowers its advertisement below the count
/// already live simply leaves no residual.
fn fit_residual_count(
    mut candidates: Vec<AttributedSubscription>,
    constraints: &RelayReadConstraints,
    running: usize,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Vec<AttributedSubscription> {
    let Some(maximum) = constraints.max_subscriptions.get().map(NonZeroUsize::get) else {
        return candidates;
    };
    let residual = maximum.saturating_sub(running);
    if candidates.len() <= residual {
        return candidates;
    }
    let required = running + candidates.len();
    for dropped in candidates.split_off(residual) {
        for demand in dropped.serves {
            shortfalls.push(SubscriptionShortfall {
                demand,
                reason: ShortfallReason::SubscriptionsExhausted { required, maximum },
            });
        }
    }
    candidates
}


/// What an EOSE on one candidate would actually prove.
///
/// The planner is the only component that sees both the filter it is sending
/// and what the relay declared, so it records the fact here instead of leaving
/// the evidence layer to re-derive it from a filter it never saw.
fn completeness(filters: &[Filter], constraints: &RelayReadConstraints) -> EoseCompleteness {
    if filters.iter().any(|filter| filter.limit.is_some()) {
        return EoseCompleteness::LimitedRequest;
    }
    if constraints.default_filter_limit.get().is_some() {
        return EoseCompleteness::RelayDefaultLimit;
    }
    EoseCompleteness::Proven
}
