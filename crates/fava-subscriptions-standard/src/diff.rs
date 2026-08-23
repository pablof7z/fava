//! Turning the desired candidate set into a diff against what is installed.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, InstalledSubscriptions, PlanRevision, PlannedSubscription,
    RelayReadConstraints, ShortfallReason, SubscriptionAttribution, SubscriptionPlan,
    SubscriptionShortfall, WithdrawalReason, WithdrawnSubscription,
};
use fava_wire::SubscriptionId;

use crate::wire;

/// Assemble the desired candidates into a plan expressed against `installed`.
pub(crate) fn assemble(
    relay: &RelaySessionKey,
    revision: PlanRevision,
    candidates: Vec<PlannedSubscription>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    mut shortfalls: Vec<SubscriptionShortfall>,
) -> SubscriptionPlan {
    let resolved = resolve_identity(candidates, constraints, installed, &mut shortfalls);

    let mut open = Vec::new();
    let mut retain = Vec::new();
    let mut attribution = Vec::new();
    let mut served_now: BTreeMap<DemandId, SubscriptionId> = BTreeMap::new();
    for candidate in resolved {
        for demand in &candidate.serves {
            served_now.insert(*demand, candidate.id.clone());
        }
        attribution.push((
            candidate.id.clone(),
            AttributedSubscription {
                filters: candidate.filters.clone(),
                serves: candidate.serves.clone(),
            },
        ));
        let unchanged = installed
            .get(&candidate.id)
            .is_some_and(|entry| entry.filters == candidate.filters);
        if unchanged {
            retain.push(candidate.id);
        } else {
            open.push(PlannedSubscription {
                id: candidate.id,
                filters: candidate.filters,
                serves: candidate.serves,
            });
        }
    }
    open.sort_by(|left, right| left.id.cmp(&right.id));
    retain.sort();

    let close = withdrawals(installed, &served_now, &shortfalls, &retain);
    SubscriptionPlan {
        relay: relay.clone(),
        revision,
        open,
        retain,
        close,
        attribution: SubscriptionAttribution::from_entries(attribution),
        shortfalls,
    }
}

/// Give every carried candidate a wire id free of collision.
///
/// A candidate whose derived id already names an installed subscription with
/// identical filters keeps it — that is what makes retention possible. Anything
/// else is stepped to the next free id, and a declared id space too small to
/// hold one more subscription becomes typed shortfall rather than a collision.
fn resolve_identity(
    candidates: Vec<PlannedSubscription>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Vec<PlannedSubscription> {
    let installed_ids: BTreeSet<SubscriptionId> = installed.ids().cloned().collect();
    let mut taken: BTreeSet<SubscriptionId> = BTreeSet::new();
    let mut resolved = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let reusable = !taken.contains(&candidate.id)
            && installed
                .get(&candidate.id)
                .is_some_and(|entry| entry.filters == candidate.filters);
        let id = if reusable {
            Some(candidate.id.clone())
        } else {
            let mut blocked = taken.clone();
            blocked.extend(installed_ids.iter().cloned());
            wire::allocate(&candidate.filters, constraints, &blocked)
        };
        if let Some(id) = id {
            taken.insert(id.clone());
            resolved.push(PlannedSubscription { id, ..candidate });
            continue;
        }
        let maximum = constraints
            .max_subscription_id_chars
            .get()
            .map_or(0, NonZeroUsize::get);
        for demand in candidate.serves {
            shortfalls.push(SubscriptionShortfall {
                demand,
                reason: ShortfallReason::SubscriptionIdTooLong { maximum },
            });
        }
    }
    resolved
}

/// Every installed subscription the plan no longer wants, with its reason.
fn withdrawals(
    installed: &InstalledSubscriptions,
    served_now: &BTreeMap<DemandId, SubscriptionId>,
    shortfalls: &[SubscriptionShortfall],
    retain: &[SubscriptionId],
) -> Vec<WithdrawnSubscription> {
    let retained: BTreeSet<&SubscriptionId> = retain.iter().collect();
    let lost: BTreeSet<DemandId> = shortfalls.iter().map(|entry| entry.demand).collect();
    let mut close = Vec::new();
    for id in installed.ids() {
        if retained.contains(id) {
            continue;
        }
        let Some(entry) = installed.get(id) else {
            continue;
        };
        let reason = entry
            .serves
            .iter()
            .find_map(|demand| served_now.get(demand))
            .map_or_else(
                || {
                    if entry.serves.iter().any(|demand| lost.contains(demand)) {
                        WithdrawalReason::ConstraintChanged
                    } else {
                        WithdrawalReason::DemandWithdrawn {
                            released: entry.serves.clone(),
                        }
                    }
                },
                |into| WithdrawalReason::Regrouped { into: into.clone() },
            );
        close.push(WithdrawnSubscription {
            id: id.clone(),
            reason,
        });
    }
    close
}
