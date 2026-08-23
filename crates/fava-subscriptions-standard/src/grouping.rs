//! Merging compatible demand into the fewest wire filters that preserve meaning.
//!
//! RELAY-003: a planner MAY merge filters that differ in one safely unionable
//! dimension and MUST NOT merge across differences that would change meaning.

use std::collections::BTreeSet;

use fava_subscriptions::{DemandId, RelayDemand, RelayReadConstraints};
use nostr::filter::Filter;

/// One candidate wire subscription: the merged filter and the exact logical
/// demand it stands for.
#[derive(Clone, Debug)]
pub(crate) struct Group {
    /// Merged filter carried by this candidate REQ.
    pub(crate) filter: Filter,
    /// Demands merged into it, in input order.
    pub(crate) members: Vec<RelayDemand>,
}

impl Group {
    /// Logical demand this candidate serves.
    pub(crate) fn serves(&self) -> BTreeSet<DemandId> {
        self.members.iter().map(RelayDemand::id).collect()
    }
}

/// Merge demand into candidate wire subscriptions.
///
/// Merging is refused across differing whole-query bounds, and refused entirely
/// beyond exact deduplication when the relay declares a default filter limit:
/// a relay-applied default limit makes a union return fewer events per member
/// than each member would have received alone (GOALS:1049).
pub(crate) fn group(demand: &[RelayDemand], constraints: &RelayReadConstraints) -> Vec<Group> {
    let dedup_only = constraints.default_filter_limit.get().is_some();
    let mut groups: Vec<Group> = Vec::new();
    for item in demand {
        let merged = groups.iter().enumerate().find_map(|(index, group)| {
            merge_candidate(group, item, dedup_only).map(|filter| (index, filter))
        });
        match merged {
            Some((index, filter)) => {
                groups[index].filter = filter;
                groups[index].members.push(item.clone());
            }
            None => groups.push(Group {
                filter: item.filter.clone(),
                members: vec![item.clone()],
            }),
        }
    }
    groups
}

/// The filter that would carry `item` inside `group` without changing meaning.
fn merge_candidate(group: &Group, item: &RelayDemand, dedup_only: bool) -> Option<Filter> {
    let anchor = group.members.first()?;
    if anchor.bounds != item.bounds {
        return None;
    }
    if group.filter == item.filter {
        return Some(group.filter.clone());
    }
    if dedup_only || group.filter.limit.is_some() || item.filter.limit.is_some() {
        return None;
    }
    merge_author_axis(&group.filter, &item.filter)
        .or_else(|| merge_tag_axis(&group.filter, &item.filter))
}

/// Union two filters that differ only in their author set.
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

/// Union two filters that differ only in the values of one tag axis.
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
