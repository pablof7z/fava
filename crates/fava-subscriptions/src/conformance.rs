//! Executable conformance rules that define semantic equivalence for any
//! planner, standard or competing.

use std::collections::{BTreeMap, BTreeSet};

use fava_state::RelaySessionKey;
use fava_wire::SubscriptionId;
use nostr::filter::Filter;
use thiserror::Error;

use crate::constraints::RelayReadConstraints;
use crate::demand::{DemandId, RelayDemand};
use crate::installed::InstalledSubscriptions;
use crate::plan::SubscriptionPlan;

/// Conformance rules that define semantic equivalence for any planner.
///
/// This function is the whole of the contract's conformance obligation. It
/// replaces the private `validate_plan` that lived in `crates/fava/src/relay.rs`,
/// which is deleted, not adapted.
///
/// Authority: ARCH:1514 "the conformance rules that define semantic
/// equivalence"; ARCH:3148 external providers "pass the same conformance kit as
/// the standard provider".
///
/// # Errors
///
/// [`PlanConformanceError`] for the first violated rule, in declaration order.
pub fn validate_plan(
    relay: &RelaySessionKey,
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    check_relay(relay, plan)?;
    let resulting = check_buckets(installed, plan)?;
    check_attribution_keys(&resulting, plan)?;
    check_filters(installed, plan)?;
    check_demand(demand, plan)?;
    check_declared_limits(constraints, plan)
}

/// C1: the plan is scoped to the relay session it was asked about.
fn check_relay(
    relay: &RelaySessionKey,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    if &plan.relay == relay {
        Ok(())
    } else {
        Err(PlanConformanceError::WrongRelay)
    }
}

/// C2, C3, C4: the diff is internally consistent against the baseline.
///
/// Returns the wire ids the plan expects to be installed after execution.
fn check_buckets(
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<BTreeSet<SubscriptionId>, PlanConformanceError> {
    let mut seen: BTreeSet<SubscriptionId> = BTreeSet::new();
    for id in plan
        .open
        .iter()
        .map(|planned| &planned.id)
        .chain(plan.retain.iter())
        .chain(plan.close.iter().map(|withdrawn| &withdrawn.id))
    {
        if !seen.insert(id.clone()) {
            return Err(PlanConformanceError::OverlappingBuckets(id.clone()));
        }
    }
    for planned in &plan.open {
        if installed.get(&planned.id).is_some() {
            return Err(PlanConformanceError::ReopenedInstalled(planned.id.clone()));
        }
    }
    for id in plan
        .retain
        .iter()
        .chain(plan.close.iter().map(|withdrawn| &withdrawn.id))
    {
        if installed.get(id).is_none() {
            return Err(PlanConformanceError::UnknownInstalled(id.clone()));
        }
    }
    Ok(plan.installed_after().cloned().collect())
}

/// C5: attribution describes exactly the resulting installed set, including
/// which logical demand each wire subscription serves.
fn check_attribution_keys(
    resulting: &BTreeSet<SubscriptionId>,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let attributed: BTreeSet<SubscriptionId> = plan.attribution.ids().cloned().collect();
    if &attributed != resulting {
        return Err(PlanConformanceError::AttributionMismatch);
    }
    for planned in &plan.open {
        let Some(entry) = plan.attribution.get(&planned.id) else {
            return Err(PlanConformanceError::AttributionMismatch);
        };
        if entry.serves != planned.serves {
            return Err(PlanConformanceError::AttributionMismatch);
        }
    }
    Ok(())
}

/// C6, C7: every planned REQ carries a filter and attribution repeats it exactly.
fn check_filters(
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    for planned in &plan.open {
        if planned.filters.is_empty() {
            return Err(PlanConformanceError::EmptyFilters(planned.id.clone()));
        }
    }
    let mut expected: BTreeMap<&SubscriptionId, &[Filter]> = BTreeMap::new();
    for planned in &plan.open {
        expected.insert(&planned.id, &planned.filters);
    }
    for id in &plan.retain {
        let Some(entry) = installed.get(id) else {
            return Err(PlanConformanceError::UnknownInstalled(id.clone()));
        };
        expected.insert(id, &entry.filters);
    }
    for (id, filters) in expected {
        let Some(entry) = plan.attribution.get(id) else {
            return Err(PlanConformanceError::AttributionMismatch);
        };
        if entry.filters.as_slice() != filters {
            return Err(PlanConformanceError::FilterAttributionMismatch(id.clone()));
        }
    }
    Ok(())
}

/// C8, C9: every input demand is accounted for and no attribution is invented.
fn check_demand(
    demand: &[RelayDemand],
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let requested: BTreeSet<DemandId> = demand.iter().map(RelayDemand::id).collect();
    let mut accounted: BTreeSet<DemandId> =
        plan.shortfalls.iter().map(|entry| entry.demand).collect();
    for id in plan.attribution.ids() {
        for served in plan.attribution.serves(id) {
            if !requested.contains(served) {
                return Err(PlanConformanceError::DemandInvented(*served));
            }
            accounted.insert(*served);
        }
    }
    for id in &requested {
        if !accounted.contains(id) {
            return Err(PlanConformanceError::DemandUnaccounted(*id));
        }
    }
    for entry in &plan.shortfalls {
        if !requested.contains(&entry.demand) {
            return Err(PlanConformanceError::DemandInvented(entry.demand));
        }
    }
    Ok(())
}

/// C10, C11: the resulting installed set honors every *declared* limit.
fn check_declared_limits(
    constraints: &RelayReadConstraints,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let resulting = plan.installed_after().count();
    if let Some(maximum) = constraints.max_subscriptions.get()
        && resulting > maximum.get()
    {
        return Err(PlanConformanceError::DeclaredSubscriptionsExceeded {
            installed: resulting,
            maximum: maximum.get(),
        });
    }
    if let Some(maximum) = constraints.max_subscription_id_chars.get() {
        for id in plan.installed_after() {
            if id.as_str().chars().count() > maximum.get() {
                return Err(PlanConformanceError::DeclaredIdLengthExceeded {
                    id: id.clone(),
                    maximum: maximum.get(),
                });
            }
        }
    }
    Ok(())
}

/// A violated planner conformance rule.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PlanConformanceError {
    /// C1: the plan names a different relay than the one it was asked about.
    #[error("plan is scoped to the wrong relay session")]
    WrongRelay,
    /// C2: `open`, `retain`, and `close` are not pairwise disjoint.
    #[error("wire subscription {0} appears in more than one diff bucket")]
    OverlappingBuckets(SubscriptionId),
    /// C3: `open` names an id that is already installed.
    #[error("plan opens already-installed subscription {0}")]
    ReopenedInstalled(SubscriptionId),
    /// C4: `retain` or `close` names an id that is not installed.
    #[error("plan references subscription {0} that is not installed")]
    UnknownInstalled(SubscriptionId),
    /// C5: attribution keys are not exactly `open` ∪ `retain`, or an opened
    /// subscription's own `serves` disagrees with its attribution.
    #[error("attribution does not describe the resulting installed set")]
    AttributionMismatch,
    /// C6: a `PlannedSubscription` carries no filters.
    #[error("planned subscription {0} carries no filter")]
    EmptyFilters(SubscriptionId),
    /// C7: attribution filters for a wire id differ from its planned filters.
    #[error("attribution filters for {0} do not match the planned REQ")]
    FilterAttributionMismatch(SubscriptionId),
    /// C8: some `DemandId` in the input is neither attributed nor a shortfall.
    #[error("demand {:?}/{:?} is neither served nor reported as shortfall", .0.owner, .0.branch)]
    DemandUnaccounted(DemandId),
    /// C9: attribution names a `DemandId` that was not in the input demand.
    #[error("attribution invents demand {:?}/{:?}", .0.owner, .0.branch)]
    DemandInvented(DemandId),
    /// C10: the resulting installed count exceeds a *declared* maximum.
    #[error("plan installs {installed} subscriptions but the relay declared {maximum}")]
    DeclaredSubscriptionsExceeded {
        /// Count after execution.
        installed: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// C11: a wire id is longer than a *declared* id-length maximum.
    #[error("subscription id {id} exceeds the declared length {maximum}")]
    DeclaredIdLengthExceeded {
        /// Offending id.
        id: SubscriptionId,
        /// Declared maximum.
        maximum: usize,
    },
}
