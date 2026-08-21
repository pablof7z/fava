//! Standard exact grouping of compatible logical relay demand.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    RelayDemand, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
};
use fava_wire::{ClientMessage, SubscriptionId, encode_client};
use nostr::filter::Filter;

/// Exact subscription planner that groups compatible author and tag filters.
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
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        if demand.is_empty() {
            return Err(SubscriptionPlanError::EmptyDemand);
        }
        refuse_duplicate_ids(demand)?;
        let mut groups: Vec<Group> = Vec::new();
        for item in demand {
            if let Some((index, merged)) =
                groups.iter_mut().enumerate().find_map(|(index, group)| {
                    merge_candidate(&group.filter, &item.filter).map(|merged| (index, merged))
                })
            {
                let group = &mut groups[index];
                group.filter = merged;
                group.logical.push(item.subscription_id.clone());
            } else {
                groups.push(Group {
                    wire_id: item.subscription_id.clone(),
                    filter: item.filter.clone(),
                    logical: vec![item.subscription_id.clone()],
                });
            }
        }
        if groups.len() > self.max_subscriptions.get() {
            return Err(SubscriptionPlanError::TooManySubscriptions {
                required: groups.len(),
                maximum: self.max_subscriptions.get(),
            });
        }

        let mut messages = Vec::with_capacity(groups.len());
        let mut attribution = BTreeMap::new();
        let mut logical = BTreeMap::new();
        for group in groups {
            let message = ClientMessage::req(group.wire_id.clone(), group.filter.clone());
            let bytes = encode_client(&message)
                .map_err(|error| SubscriptionPlanError::Encoding(error.to_string()))?
                .len();
            if bytes > self.max_frame_bytes.get() {
                return Err(SubscriptionPlanError::FrameTooLarge {
                    bytes,
                    maximum: self.max_frame_bytes.get(),
                });
            }
            attribution.insert(group.wire_id.clone(), group.filter);
            logical.insert(group.wire_id, group.logical);
            messages.push(message);
        }
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

fn merge_candidate(left: &Filter, right: &Filter) -> Option<Filter> {
    if left == right {
        return Some(left.clone());
    }
    if left.limit.is_some() || right.limit.is_some() {
        return None;
    }

    merge_author_axis(left, right).or_else(|| merge_tag_axis(left, right))
}

fn merge_author_axis(left: &Filter, right: &Filter) -> Option<Filter> {
    let mut left_base = left.clone();
    let mut right_base = right.clone();
    let left_authors = left_base.authors.take()?;
    let right_authors = right_base.authors.take()?;
    if left_authors.is_empty() || right_authors.is_empty() || left_base != right_base {
        return None;
    }

    let mut merged = left.clone();
    merged
        .authors
        .get_or_insert_with(BTreeSet::new)
        .extend(right_authors);
    Some(merged)
}

fn merge_tag_axis(left: &Filter, right: &Filter) -> Option<Filter> {
    let mut left_base = left.clone();
    let mut right_base = right.clone();
    left_base.generic_tags.clear();
    right_base.generic_tags.clear();
    if left_base != right_base || left.generic_tags.len() != right.generic_tags.len() {
        return None;
    }

    let mut differing_key = None;
    for (key, left_values) in &left.generic_tags {
        let right_values = right.generic_tags.get(key)?;
        if left_values == right_values {
            continue;
        }
        if left_values.is_empty() || right_values.is_empty() || differing_key.is_some() {
            return None;
        }
        differing_key = Some(*key);
    }

    let key = differing_key?;
    let mut merged = left.clone();
    merged
        .generic_tags
        .get_mut(&key)?
        .extend(right.generic_tags.get(&key)?.iter().cloned());
    Some(merged)
}
