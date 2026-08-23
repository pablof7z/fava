//! One exact Nostr subscription per logical relay demand.
//!
//! This policy never merges. It exists so that grouping can be falsified
//! differentially: the same demand planned here and by the standard policy must
//! ask the relay for the same events, attribute them to the same logical
//! demand, and withdraw the same demand on cancellation.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    AttributedSubscription, DemandId, InstalledSubscriptions, PlanRevision, PlannedSubscription,
    RelayDemand, RelayReadConstraints, ShortfallReason, SubscriptionAttribution, SubscriptionPlan,
    SubscriptionPlanError, SubscriptionPlanner, SubscriptionShortfall, WithdrawalReason,
    WithdrawnSubscription,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// One wire subscription per logical demand, diffed against what is installed.
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
        let mut desired: BTreeMap<SubscriptionId, (Filter, DemandId)> = BTreeMap::new();
        for item in demand {
            let id = identity(item, constraints);
            if let Some(reason) = refusal(item, constraints, &id) {
                shortfalls.push(SubscriptionShortfall {
                    demand: item.id(),
                    reason,
                });
                continue;
            }
            if desired.contains_key(&id) {
                shortfalls.push(SubscriptionShortfall {
                    demand: item.id(),
                    reason: ShortfallReason::SubscriptionIdTooLong {
                        maximum: constraints
                            .max_subscription_id_chars
                            .get()
                            .map_or(0, NonZeroUsize::get),
                    },
                });
                continue;
            }
            desired.insert(id, (item.filter.clone(), item.id()));
        }
        enforce_count(&mut desired, constraints, installed, &mut shortfalls);

        Ok(assemble(relay, revision, desired, installed, shortfalls))
    }
}

/// Construct the policy that preserves one wire subscription per demand.
#[must_use]
pub const fn planner() -> impl SubscriptionPlanner {
    OnePerDemand
}

/// Wire identity for one demand: its own logical identity, made wire-safe.
fn identity(demand: &RelayDemand, constraints: &RelayReadConstraints) -> SubscriptionId {
    let full = format!("fava-{}-{}", demand.owner.get(), demand.branch.0);
    match constraints.max_subscription_id_chars.get() {
        Some(maximum) if full.chars().count() > maximum.get() => {
            SubscriptionId::new(full.chars().take(maximum.get()).collect::<String>())
        }
        _ => SubscriptionId::new(full),
    }
}

/// Why this demand cannot be expressed exactly under the declared constraints.
fn refusal(
    demand: &RelayDemand,
    constraints: &RelayReadConstraints,
    id: &SubscriptionId,
) -> Option<ShortfallReason> {
    if let (Some(required), Some(maximum)) =
        (demand.filter.limit, constraints.max_filter_limit.get())
        && required > maximum.get()
    {
        return Some(ShortfallReason::FilterLimitExceeded {
            required,
            maximum: maximum.get(),
        });
    }
    if let Some(maximum) = constraints.max_message_bytes.get() {
        let bytes = encoded_bytes(id, &demand.filter);
        if bytes > maximum.get() {
            return Some(ShortfallReason::MessageTooLarge {
                bytes,
                maximum: maximum.get(),
            });
        }
    }
    None
}

/// Exact encoded byte length of the REQ this demand produces.
fn encoded_bytes(id: &SubscriptionId, filter: &Filter) -> usize {
    fava_wire::encode_client(&fava_wire::ClientMessage::req(id.clone(), filter.clone()))
        .map_or(usize::MAX, |frame| frame.len())
}

/// Drop what does not fit a *declared* subscription count, keeping installed
/// subscriptions first so a ceiling does not churn live demand.
fn enforce_count(
    desired: &mut BTreeMap<SubscriptionId, (Filter, DemandId)>,
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    shortfalls: &mut Vec<SubscriptionShortfall>,
) {
    let Some(maximum) = constraints.max_subscriptions.get().map(NonZeroUsize::get) else {
        return;
    };
    let required = desired.len();
    if required <= maximum {
        return;
    }
    let mut ranked: Vec<SubscriptionId> = desired.keys().cloned().collect();
    ranked.sort_by(|left, right| {
        installed
            .get(left)
            .is_none()
            .cmp(&installed.get(right).is_none())
            .then_with(|| left.cmp(right))
    });
    for id in ranked.split_off(maximum) {
        if let Some((_, demand)) = desired.remove(&id) {
            shortfalls.push(SubscriptionShortfall {
                demand,
                reason: ShortfallReason::SubscriptionsExhausted { required, maximum },
            });
        }
    }
}

/// Express the desired set as a diff against what is installed.
fn assemble(
    relay: &RelaySessionKey,
    revision: PlanRevision,
    desired: BTreeMap<SubscriptionId, (Filter, DemandId)>,
    installed: &InstalledSubscriptions,
    shortfalls: Vec<SubscriptionShortfall>,
) -> SubscriptionPlan {
    let mut open = Vec::new();
    let mut retain = Vec::new();
    let mut attribution = Vec::new();
    let mut served_now: BTreeMap<DemandId, SubscriptionId> = BTreeMap::new();
    for (id, (filter, demand)) in desired {
        let serves: BTreeSet<DemandId> = [demand].into_iter().collect();
        let filters = vec![filter];
        served_now.insert(demand, id.clone());
        attribution.push((
            id.clone(),
            AttributedSubscription {
                filters: filters.clone(),
                serves: serves.clone(),
            },
        ));
        if installed
            .get(&id)
            .is_some_and(|entry| entry.filters == filters)
        {
            retain.push(id);
        } else {
            open.push(PlannedSubscription {
                id,
                filters,
                serves,
            });
        }
    }

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
