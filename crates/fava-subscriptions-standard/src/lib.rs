//! Standard exact grouping of compatible logical relay demand.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    RelayDemand, RelayLimits, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
    enforce_limits,
};
use fava_wire::{ClientMessage, SubscriptionId};
use nostr::filter::Filter;

/// Exact subscription planner that groups compatible author filters.
pub struct StandardSubscriptionPlanner {
    max_subscriptions: NonZeroUsize,
    max_frame_bytes: NonZeroUsize,
}

impl Default for StandardSubscriptionPlanner {
    fn default() -> Self {
        Self::bounded(
            NonZeroUsize::new(64).expect("constant is non-zero"),
            NonZeroUsize::new(1_048_576).expect("constant is non-zero"),
        )
    }
}

impl StandardSubscriptionPlanner {
    /// Configure exact relay subscription-count and frame-size limits.
    #[must_use]
    pub const fn bounded(max_subscriptions: NonZeroUsize, max_frame_bytes: NonZeroUsize) -> Self {
        Self {
            max_subscriptions,
            max_frame_bytes,
        }
    }
}

impl SubscriptionPlanner for StandardSubscriptionPlanner {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        limits: &RelayLimits,
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        if demand.is_empty() {
            return Err(SubscriptionPlanError::EmptyDemand);
        }
        refuse_duplicate_ids(demand)?;
        let mut groups: Vec<Group> = Vec::new();
        for item in demand {
            if let Some(group) = groups
                .iter_mut()
                .find(|group| compatible(&group.filter, &item.filter))
            {
                merge_filter(&mut group.filter, &item.filter);
                group.logical.push(item.subscription_id.clone());
            } else {
                groups.push(Group {
                    wire_id: item.subscription_id.clone(),
                    filter: item.filter.clone(),
                    logical: vec![item.subscription_id.clone()],
                });
            }
        }
        let mut messages = Vec::with_capacity(groups.len());
        let mut attribution = BTreeMap::new();
        let mut logical = BTreeMap::new();
        for group in groups {
            let message = ClientMessage::req(group.wire_id.clone(), group.filter.clone());
            attribution.insert(group.wire_id.clone(), group.filter);
            logical.insert(group.wire_id, group.logical);
            messages.push(message);
        }
        enforce_limits(
            limits,
            self.max_subscriptions.get(),
            self.max_frame_bytes.get(),
            &messages,
        )?;
        Ok(SubscriptionPlan {
            relay: relay.clone(),
            messages,
            attribution,
            demand: logical,
        })
    }
}

struct Group {
    wire_id: SubscriptionId,
    filter: Filter,
    logical: Vec<SubscriptionId>,
}

fn refuse_duplicate_ids(demand: &[RelayDemand]) -> Result<(), SubscriptionPlanError> {
    let mut ids = BTreeSet::new();
    for item in demand {
        if !ids.insert(item.subscription_id.clone()) {
            return Err(SubscriptionPlanError::DuplicateSubscription(
                item.subscription_id.clone(),
            ));
        }
    }
    Ok(())
}

fn compatible(left: &Filter, right: &Filter) -> bool {
    if left == right {
        return true;
    }
    if left.limit.is_some() || right.limit.is_some() {
        return false;
    }
    let mut left_base = left.clone();
    let mut right_base = right.clone();
    let left_authors = left_base.authors.take();
    let right_authors = right_base.authors.take();
    left_authors.is_some()
        && right_authors.is_some()
        && left_authors
            .as_ref()
            .is_some_and(|authors| !authors.is_empty())
        && right_authors
            .as_ref()
            .is_some_and(|authors| !authors.is_empty())
        && left_base == right_base
}

fn merge_filter(current: &mut Filter, incoming: &Filter) {
    if current == incoming {
        return;
    }
    let incoming = incoming
        .authors
        .as_ref()
        .expect("compatible filters have authors");
    current
        .authors
        .get_or_insert_with(BTreeSet::new)
        .extend(incoming.iter().copied());
}
