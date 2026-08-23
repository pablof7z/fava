//! Expressing the answer as a diff against what is already running.
//!
//! `retain` is every running subscription that still has an owner — including
//! ones whose owner set grew or shrank, because neither changes a byte on the
//! wire. `close` is only ever "the last owner is gone". `open` is only ever
//! coverage that was missing.

use std::collections::{BTreeMap, BTreeSet};

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, EoseCompleteness, InstalledSubscriptions, PlanRevision,
    PlannedSubscription, RelayReadConstraints, SubscriptionAttribution, SubscriptionPlan,
    SubscriptionShortfall, WithdrawalReason, WithdrawnSubscription,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// Assemble the answer for one relay session.
pub(crate) fn assemble(
    relay: &RelaySessionKey,
    revision: PlanRevision,
    opened: Vec<(SubscriptionId, AttributedSubscription)>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    owners: &BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
    shortfalls: Vec<SubscriptionShortfall>,
) -> SubscriptionPlan {
    let mut attribution = Vec::with_capacity(opened.len() + owners.len());
    let mut open = Vec::with_capacity(opened.len());
    for (id, attributed) in opened {
        open.push(PlannedSubscription {
            id: id.clone(),
            filters: attributed.filters.clone(),
            serves: attributed.serves.clone(),
        });
        attribution.push((id, attributed));
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
    open.sort_by(|left, right| left.id.cmp(&right.id));
    retain.sort();

    SubscriptionPlan {
        relay: relay.clone(),
        revision,
        open,
        retain,
        close: withdrawals(installed, owners, &shortfalls),
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
    shortfalls: &[SubscriptionShortfall],
) -> Vec<WithdrawnSubscription> {
    let lost: BTreeSet<DemandId> = shortfalls.iter().map(|entry| entry.demand).collect();
    let mut close = Vec::new();
    for id in installed.ids() {
        if owners.contains_key(id) {
            continue;
        }
        let Some(entry) = installed.get(id) else {
            continue;
        };
        let reason = if entry.serves.iter().any(|demand| lost.contains(demand)) {
            WithdrawalReason::ConstraintChanged
        } else {
            WithdrawalReason::DemandWithdrawn {
                released: entry.serves.clone(),
            }
        };
        close.push(WithdrawnSubscription {
            id: id.clone(),
            reason,
        });
    }
    close
}
