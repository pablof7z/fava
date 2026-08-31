//! Executable conformance rules that define semantic equivalence for any
//! planner, standard or competing.

use std::collections::BTreeSet;

use fava_relay::RelaySessionKey;
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
    check_running_subscriptions(demand, installed, plan)?;
    check_distinct_filters(installed, plan)?;
    check_declared_limits(constraints, plan)
}

/// CR-1: a running subscription is never withdrawn while it is still wanted.
///
/// Grouping compiles unsent demand. A demand joining or leaving the relay must
/// never rewrite a subscription the relay is already serving: the relay would
/// re-serve the entire stored window for demand that was already settled, and
/// the waste is quadratic in the number of times demand grows. A subscription
/// closes when its last logical owner is gone and at no other time.
fn check_running_subscriptions(
    demand: &[RelayDemand],
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let requested: BTreeSet<DemandId> = demand.iter().map(RelayDemand::id).collect();
    for withdrawn in &plan.close {
        let Some(entry) = installed.get(withdrawn) else {
            continue;
        };
        if let Some(retained) = entry
            .serves
            .iter()
            .find(|served| requested.contains(served))
        {
            return Err(PlanConformanceError::RunningSubscriptionWithdrawn {
                id: withdrawn.clone(),
                still_wanted: *retained,
            });
        }
    }
    Ok(())
}

/// CR-2: the plan never opens a second subscription for filters already on the
/// wire.
///
/// Two byte-identical REQs cannot be told apart by any identity scheme. The
/// relay double-delivers every matching event forever, a subscription slot is
/// burned, and completion evidence splits across two identities so neither is
/// credited. Byte-identical candidates belong in one subscription.
fn check_distinct_filters(
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let mut seen: Vec<(&SubscriptionId, &[Filter])> = Vec::new();
    for id in &plan.retain {
        if let Some(entry) = installed.get(id) {
            seen.push((id, &entry.filters));
        }
    }
    let mut planned_filters: Vec<&[Filter]> = Vec::new();
    for (position, planned) in plan.open.iter().enumerate() {
        if let Some((first, _)) = seen
            .iter()
            .find(|(_, filters)| *filters == planned.filters.as_slice())
        {
            return Err(PlanConformanceError::DuplicateFilters {
                first: (*first).clone(),
                second: position,
            });
        }
        if planned_filters.contains(&planned.filters.as_slice()) {
            return Err(PlanConformanceError::DuplicatePlannedFilters(position));
        }
        planned_filters.push(&planned.filters);
    }
    Ok(())
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

/// C4: `retain` and `close` name installed subscriptions, and name them once.
///
/// `open` carries no identity to collide with: the session mints each wire id
/// when it sends the REQ, so a plan cannot reopen an installed subscription or
/// name one twice.
///
/// Returns the wire ids the plan expects to still be installed after execution.
fn check_buckets(
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<BTreeSet<SubscriptionId>, PlanConformanceError> {
    let mut seen: BTreeSet<SubscriptionId> = BTreeSet::new();
    for id in plan.retain.iter().chain(plan.close.iter()) {
        if !seen.insert(id.clone()) {
            return Err(PlanConformanceError::OverlappingBuckets(id.clone()));
        }
        if installed.get(id).is_none() {
            return Err(PlanConformanceError::UnknownInstalled(id.clone()));
        }
    }
    Ok(plan.retain.iter().cloned().collect())
}

/// C5: attribution describes exactly the subscriptions that carry a wire id.
///
/// That is the retained set. Each entry of `open` is its own attribution and
/// has no id until the session mints one, so there is no second record of what
/// it serves and nothing for the two to disagree about.
fn check_attribution_keys(
    resulting: &BTreeSet<SubscriptionId>,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let attributed: BTreeSet<SubscriptionId> = plan.attribution.ids().cloned().collect();
    if &attributed != resulting {
        return Err(PlanConformanceError::AttributionMismatch);
    }
    Ok(())
}

/// C6: every planned REQ carries a filter, and retained attribution repeats
/// the filters actually installed.
fn check_filters(
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    for (position, planned) in plan.open.iter().enumerate() {
        if planned.filters.is_empty() {
            return Err(PlanConformanceError::EmptyFilters(position));
        }
    }
    for id in &plan.retain {
        let Some(entry) = installed.get(id) else {
            return Err(PlanConformanceError::UnknownInstalled(id.clone()));
        };
        let Some(attributed) = plan.attribution.get(id) else {
            return Err(PlanConformanceError::AttributionMismatch);
        };
        if attributed.filters != entry.filters {
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
    // Demand is served either by a subscription that already carries a wire id
    // — the retained set, which attribution covers — or by one this plan opens,
    // which is its own attribution until the session names it.
    for served in plan
        .attribution
        .ids()
        .flat_map(|id| plan.attribution.serves(id).iter())
        .chain(plan.open.iter().flat_map(|planned| planned.serves.iter()))
    {
        if !requested.contains(served) {
            return Err(PlanConformanceError::DemandInvented(*served));
        }
        accounted.insert(*served);
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

/// C10: the resulting installed set honors the declared subscription limit.
///
/// There is no declared-identifier-length rule: the session mints fixed-width
/// identifiers inside the 64 characters NIP-01 obliges every relay to accept,
/// so a Fava identifier is never too long for a conforming relay.
fn check_declared_limits(
    constraints: &RelayReadConstraints,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let resulting = plan.installed_count();
    // The budget a plan may spend is the *residual*: the declared maximum less
    // what is already running and still wanted. A relay that lowers its
    // advertisement below the count already live does not authorize closing a
    // running subscription (CR-1), so the plan is answerable only for what it
    // opened.
    if let Some(maximum) = constraints.max_subscriptions.get() {
        let residual = maximum.get().saturating_sub(plan.retain.len());
        if plan.open.len() > residual {
            return Err(PlanConformanceError::DeclaredSubscriptionsExceeded {
                installed: resulting,
                maximum: maximum.get(),
            });
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
    /// C4: `retain` or `close` names an id that is not installed.
    #[error("plan references subscription {0} that is not installed")]
    UnknownInstalled(SubscriptionId),
    /// C5: attribution keys are not exactly `open` ∪ `retain`, or an opened
    /// subscription's own `serves` disagrees with its attribution.
    #[error("attribution does not describe the resulting installed set")]
    AttributionMismatch,
    /// C6: a `PlannedSubscription` carries no filters.
    #[error("planned subscription at position {0} carries no filter")]
    EmptyFilters(usize),
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
    /// C5b: a planned subscription and its attribution disagree about which
    /// logical demand it serves.
    #[error("planned subscription {0} and its attribution disagree about what it serves")]
    ServedDemandDisagrees(SubscriptionId),
    /// C5c: a withdrawal names a successor the plan does not produce.
    #[error("withdrawal of {id} names successor {successor}, which the plan does not install")]
    UnknownSuccessor {
        /// Wire id being withdrawn.
        id: SubscriptionId,
        /// Successor it named.
        successor: SubscriptionId,
    },
    /// CR-1: the plan withdraws a running subscription that still has an owner.
    #[error("plan closes running subscription {id}, still wanted by {:?}/{:?}", .still_wanted.owner, .still_wanted.branch)]
    RunningSubscriptionWithdrawn {
        /// Wire id the plan wants closed.
        id: SubscriptionId,
        /// A demand in the input set that this subscription still serves.
        still_wanted: DemandId,
    },
    /// CR-2: two subscriptions in the resulting installed set carry the same
    /// filters.
    #[error("subscriptions {first} and {second} carry byte-identical filters")]
    DuplicateFilters {
        /// Installed or retained wire id that already carries them.
        first: SubscriptionId,
        /// Position in `open` that duplicates them.
        second: usize,
    },
    /// CR-2: two entries of `open` carry byte-identical filters.
    #[error("two planned subscriptions carry identical filters; second at position {0}")]
    DuplicatePlannedFilters(usize),
}
