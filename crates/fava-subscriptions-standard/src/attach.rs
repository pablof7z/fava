//! Deciding which demand is already being served, and which is still unsent.
//!
//! This is the only place `installed` is allowed to influence the shape of the
//! answer, and it influences it by *removing* work, never by rewriting a
//! running subscription.

use std::collections::{BTreeMap, BTreeSet};

use fava_subscriptions::{DemandId, InstalledSubscriptions, RelayDemand, filter_covers};
use fava_wire::SubscriptionId;

/// Decide, for each demand, whether it needs a subscription of its own.
///
/// Three outcomes in order, matching what a running relay session can actually
/// offer:
///
/// 1. the demand is already an owner of a running subscription — a pure
///    refcount, nothing to do;
/// 2. a running subscription's filter already asks the relay for everything
///    this demand wants — attach, and the local per-demand re-match keeps the
///    surplus out of its results;
/// 3. neither — the demand is pending and will get its own request.
///
/// Returns the extra demand each running subscription now also serves, paired
/// with the demand no running subscription carries.
///
/// A pending demand ships its **whole** filter. The incumbent's coverage is
/// never subtracted from it: over-fetch is absorbed locally, while a residual
/// that turns out not to be equivalent is silent data loss.
#[allow(
    clippy::type_complexity,
    reason = "naming this pair would add a public type for a shape used in one place"
)]
pub(crate) fn admit(
    demand: &[RelayDemand],
    installed: &InstalledSubscriptions,
) -> (
    BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
    Vec<RelayDemand>,
) {
    let mut attached: BTreeMap<SubscriptionId, BTreeSet<DemandId>> = BTreeMap::new();
    let mut pending = Vec::new();
    for item in demand {
        match host_for(item, installed) {
            Some(id) => {
                attached.entry(id).or_default().insert(item.id());
            }
            None => pending.push(item.clone()),
        }
    }
    (attached, pending)
}

/// The running subscription that already serves this demand, if any.
///
/// Ties break on ascending wire id so the answer does not depend on map
/// iteration luck.
fn host_for(demand: &RelayDemand, installed: &InstalledSubscriptions) -> Option<SubscriptionId> {
    let mut covering = None;
    for id in installed.ids() {
        let Some(entry) = installed.get(id) else {
            continue;
        };
        if entry.serves.contains(&demand.id()) {
            return Some(id.clone());
        }
        if covering.is_none()
            && entry
                .filters
                .iter()
                .any(|filter| filter_covers(filter, &demand.filter))
        {
            covering = Some(id.clone());
        }
    }
    covering
}

/// Which running subscriptions still have at least one logical owner.
///
/// A subscription's owners are the demands it was installed to serve that are
/// still in the current demand set, plus anything newly attached to it. When
/// that count reaches zero — and only then — the subscription may close.
pub(crate) fn surviving_owners(
    demand: &[RelayDemand],
    installed: &InstalledSubscriptions,
    attached: &BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
) -> BTreeMap<SubscriptionId, BTreeSet<DemandId>> {
    let requested: BTreeSet<DemandId> = demand.iter().map(RelayDemand::id).collect();
    let mut owners: BTreeMap<SubscriptionId, BTreeSet<DemandId>> = BTreeMap::new();
    for id in installed.ids() {
        let Some(entry) = installed.get(id) else {
            continue;
        };
        let mut still: BTreeSet<DemandId> = entry
            .serves
            .iter()
            .filter(|served| requested.contains(served))
            .copied()
            .collect();
        if let Some(joined) = attached.get(id) {
            still.extend(joined.iter().copied());
        }
        if !still.is_empty() {
            owners.insert(id.clone(), still);
        }
    }
    owners
}
