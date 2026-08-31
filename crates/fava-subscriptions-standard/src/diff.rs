//! Expressing the answer as a diff against what is already running.
//!
//! `retain` is every running subscription that still has an owner — including
//! ones whose owner set grew or shrank, because neither changes a byte on the
//! wire. `close` is only ever "the last owner is gone". `open` is only ever
//! coverage that was missing.

use std::collections::{BTreeMap, BTreeSet};

use fava_relay::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, EoseCompleteness, InstalledSubscriptions, PlanRevision,
    PlannedSubscription, RelayReadConstraints, SubscriptionAttribution, SubscriptionPlan,
    SubscriptionShortfall,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// Assemble the answer for one relay session.
pub(crate) fn assemble(
    relay: &RelaySessionKey,
    revision: PlanRevision,
    opened: Vec<AttributedSubscription>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    owners: &BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
    shortfalls: Vec<SubscriptionShortfall>,
) -> SubscriptionPlan {
    // Attribution covers only what already carries a wire id: the retained
    // set. A planned subscription is its own attribution.
    let mut attribution = Vec::with_capacity(owners.len());
    let mut open = Vec::with_capacity(opened.len());
    for attributed in opened {
        open.push(PlannedSubscription {
            filters: attributed.filters,
            serves: attributed.serves,
            completeness: attributed.completeness,
        });
    }

    let mut retain = Vec::with_capacity(owners.len());
    for (id, serves) in owners {
        let Some(entry) = installed.get(id) else {
            continue;
        };
        attribution.push((
            id.clone(),
            AttributedSubscription {
                filters: entry.filters.clone(),
                serves: serves.clone(),
                completeness: completeness(&entry.filters, constraints),
            },
        ));
        retain.push(id.clone());
    }
    open.sort_by(|left, right| left.filters.cmp(&right.filters));
    retain.sort();

    SubscriptionPlan {
        relay: relay.clone(),
        revision,
        open,
        retain,
        close: withdrawals(installed, owners),
        attribution: SubscriptionAttribution::from_entries(attribution),
        shortfalls,
    }
}

/// What an EOSE on this subscription would actually prove.
///
/// The planner is the only component that sees both the filter it is sending
/// and what the relay declared, so it records the fact instead of leaving the
/// evidence layer to re-derive it from a filter it never saw.
fn completeness(filters: &[Filter], constraints: &RelayReadConstraints) -> EoseCompleteness {
    if filters.iter().any(|filter| filter.limit.is_some()) {
        return EoseCompleteness::LimitedRequest;
    }
    if constraints.default_filter_limit.get().is_some() {
        return EoseCompleteness::RelayDefaultLimit;
    }
    EoseCompleteness::Proven
}

/// Every running subscription whose last logical owner is gone.
///
/// There is no other reason to close one. A subscription that keeps an owner is
/// retained unchanged, even when it is now broader than what its remaining
/// owners asked for: the surplus is discarded by the local per-demand re-match,
/// and narrowing it would cost a full re-serve of the stored window.
fn withdrawals(
    installed: &InstalledSubscriptions,
    owners: &BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
) -> Vec<SubscriptionId> {
    installed
        .ids()
        .filter(|id| !owners.contains_key(*id))
        .cloned()
        .collect()
}
