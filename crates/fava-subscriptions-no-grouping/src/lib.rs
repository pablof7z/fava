//! One exact Nostr subscription per logical relay demand.
//!
//! This policy never merges. It exists so that grouping can be falsified
//! differentially: the same demand planned here and by the standard policy must
//! ask the relay for the same events, attribute them to the same logical
//! demand, and withdraw the same demand on cancellation.
//!
//! Not merging does not exempt it from the rules that govern a running
//! subscription. It attaches demand to a subscription already serving it rather
//! than opening a second one, never rewrites or reopens a running subscription,
//! closes only at the last owner, and mints fresh identity for everything it
//! opens.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, EoseCompleteness, InstalledSubscriptions, PlanRevision,
    PlannedSubscription, RelayDemand, RelayReadConstraints, ShortfallReason,
    SubscriptionAttribution, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
    SubscriptionShortfall, WithdrawalReason, WithdrawnSubscription,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// One wire subscription per logical demand, diffed against what is running.
struct OnePerDemand;

impl SubscriptionPlanner for OnePerDemand {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
        installed: &InstalledSubscriptions,
        revision: PlanRevision,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        let mut seen = BTreeSet::new();
        for item in demand {
            if !seen.insert(item.id()) {
                return Err(SubscriptionPlanError::DuplicateDemand(item.id()));
            }
        }

        let mut shortfalls = Vec::new();
        let admissible = admit_filter_limits(demand, constraints, &mut shortfalls);
        let mut attached: BTreeMap<SubscriptionId, BTreeSet<DemandId>> = BTreeMap::new();
        let mut pending: Vec<RelayDemand> = Vec::new();
        for item in &admissible {
            match running_host(item, installed) {
                Some(id) => {
                    attached.entry(id).or_default().insert(item.id());
                }
                None => pending.push(item.clone()),
            }
        }
        pending.sort_by_key(RelayDemand::id);

        let owners = surviving_owners(&admissible, installed, &attached);
        let carried = admit_pending(
            pending,
            constraints,
            installed,
            owners.len(),
            revision,
            &mut shortfalls,
        );

        Ok(assemble(
            relay,
            revision,
            carried,
            constraints,
            installed,
            &owners,
            shortfalls,
        ))
    }
}

/// Construct the policy that preserves one wire subscription per demand.
#[must_use]
pub const fn planner() -> impl SubscriptionPlanner {
    OnePerDemand
}

/// Wire identity for one newly-opened subscription.
///
/// Minted from the owner's monotonic revision and this subscription's ordinal,
/// never from the filter: a derived identity comes back when the same filter is
/// re-demanded, and a late EOSE for the closed request would settle the new one
/// (GOALS:426, QUERY-010). Nothing the relay advertises feeds it either, so a
/// NIP-11 refetch cannot move an established id.
fn mint(revision: PlanRevision, ordinal: usize) -> SubscriptionId {
    SubscriptionId::new(format!("fava-{}-{ordinal}", revision.get()))
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

/// The running subscription that already serves this demand, if any.
///
/// This policy attaches only to a **byte-identical** running filter. It never
/// merges, so it never widens; the containment attach the grouping policy
/// performs would make the two disagree about how many requests exist, which is
/// exactly the difference the differential is measuring.
fn running_host(
    demand: &RelayDemand,
    installed: &InstalledSubscriptions,
) -> Option<SubscriptionId> {
    let exact = std::slice::from_ref(&demand.filter);
    for id in installed.ids() {
        let Some(entry) = installed.get(id) else {
            continue;
        };
        if entry.serves.contains(&demand.id()) || entry.filters == exact {
            return Some(id.clone());
        }
    }
    None
}

/// Which running subscriptions still have at least one logical owner.
fn surviving_owners(
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

/// Give every pending demand a request, within the residual budget.
fn admit_pending(
    pending: Vec<RelayDemand>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    running: usize,
    revision: PlanRevision,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) -> Vec<PlannedSubscription> {
    let residual = constraints
        .max_subscriptions
        .get()
        .map(NonZeroUsize::get)
        .map(|maximum| (maximum.saturating_sub(running), maximum));
    let declared_id = constraints
        .max_subscription_id_chars
        .get()
        .map(NonZeroUsize::get);
    let declared_bytes = constraints.max_message_bytes.get().map(NonZeroUsize::get);

    let required = running + pending.len();
    let mut carried: Vec<PlannedSubscription> = Vec::new();
    let mut running_filters: Vec<Vec<Filter>> = installed
        .ids()
        .filter_map(|id| installed.get(id).map(|entry| entry.filters.clone()))
        .collect();
    for item in pending {
        if let Some((residual, maximum)) = residual
            && carried.len() >= residual
        {
            shortfalls.push(SubscriptionShortfall {
                demand: item.id(),
                reason: ShortfallReason::SubscriptionsExhausted { required, maximum },
            });
            continue;
        }
        let filters = vec![item.filter.clone()];
        // Two byte-identical requests on one session are strictly worse than
        // one: the relay double-delivers and completion evidence splits.
        if let Some(existing) = carried
            .iter_mut()
            .find(|planned| planned.filters == filters)
        {
            existing.serves.insert(item.id());
            continue;
        }
        if running_filters.contains(&filters) {
            continue;
        }
        let id = mint(revision, carried.len());
        if declared_id.is_some_and(|maximum| id.as_str().chars().count() > maximum) {
            shortfalls.push(SubscriptionShortfall {
                demand: item.id(),
                reason: ShortfallReason::SubscriptionIdTooLong {
                    maximum: declared_id.unwrap_or(0),
                },
            });
            continue;
        }
        let bytes = encoded_bytes(&id, &item.filter);
        if let Some(maximum) = declared_bytes
            && bytes > maximum
        {
            shortfalls.push(SubscriptionShortfall {
                demand: item.id(),
                reason: ShortfallReason::MessageTooLarge { bytes, maximum },
            });
            continue;
        }
        running_filters.push(filters.clone());
        carried.push(PlannedSubscription {
            id,
            filters,
            serves: [item.id()].into_iter().collect(),
        });
    }
    carried
}

/// Exact encoded byte length of the REQ this demand produces.
fn encoded_bytes(id: &SubscriptionId, filter: &Filter) -> usize {
    fava_wire::encode_client(&fava_wire::ClientMessage::req(id.clone(), filter.clone()))
        .map_or(usize::MAX, |frame| frame.len())
}

/// Express the answer as a diff against what is running.
fn assemble(
    relay: &RelaySessionKey,
    revision: PlanRevision,
    open: Vec<PlannedSubscription>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    owners: &BTreeMap<SubscriptionId, BTreeSet<DemandId>>,
    shortfalls: Vec<SubscriptionShortfall>,
) -> SubscriptionPlan {
    let mut attribution = Vec::with_capacity(open.len() + owners.len());
    for planned in &open {
        attribution.push((
            planned.id.clone(),
            AttributedSubscription {
                filters: planned.filters.clone(),
                serves: planned.serves.clone(),
                completeness: completeness(&planned.filters, constraints),
            },
        ));
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
    retain.sort();

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

/// What an EOSE on this subscription would actually prove.
fn completeness(filters: &[Filter], constraints: &RelayReadConstraints) -> EoseCompleteness {
    if filters.iter().any(|filter| filter.limit.is_some()) {
        return EoseCompleteness::LimitedRequest;
    }
    if constraints.default_filter_limit.get().is_some() {
        return EoseCompleteness::RelayDefaultLimit;
    }
    EoseCompleteness::Proven
}
