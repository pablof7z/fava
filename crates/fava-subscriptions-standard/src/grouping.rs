//! Merging compatible demand into the fewest wire filters that preserve meaning.
//!
//! RELAY-003: a planner MAY merge filters that differ in one safely unionable
//! dimension and MUST NOT merge across differences that would change meaning.
//!
//! A merged candidate is represented as the exact demand it was merged from,
//! never as a filter that has forgotten its members. [`merged_filter`]
//! recomputes the union from those members, so splitting a candidate is
//! undoing a merge rather than truncating a filter.

use std::collections::BTreeSet;

use fava_subscriptions::{RelayDemand, RelayReadConstraints};
use nostr::filter::Filter;

/// Partition demand into the candidate wire subscriptions it can share.
///
/// Merging is refused across differing whole-query bounds, and refused entirely
/// beyond exact deduplication when the relay declares a default filter limit:
/// a relay-applied default limit makes a union return fewer events per member
/// than each member would have received alone (GOALS:1049).
pub(crate) fn group(
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
) -> Vec<Vec<RelayDemand>> {
    let dedup_only = constraints.default_filter_limit.get().is_some();
    let mut groups: Vec<(Filter, Vec<RelayDemand>)> = Vec::new();
    for item in demand {
        let merged = groups
            .iter()
            .enumerate()
            .find_map(|(index, (filter, members))| {
                merge_candidate(filter, members.first(), item, dedup_only)
                    .map(|merged| (index, merged))
            });
        match merged {
            Some((index, filter)) => {
                groups[index].0 = filter;
                groups[index].1.push(item.clone());
            }
            None => groups.push((item.filter.clone(), vec![item.clone()])),
        }
    }
    groups.into_iter().map(|(_, members)| members).collect()
}

/// The exact union filter one candidate's members share.
///
/// Recomputing from the members keeps the filter and the attribution derived
/// from one source, so they cannot drift apart.
pub(crate) fn merged_filter(
    members: &[RelayDemand],
    constraints: &RelayReadConstraints,
) -> Option<Filter> {
    let dedup_only = constraints.default_filter_limit.get().is_some();
    let mut merged = members.first()?.filter.clone();
    for item in members.iter().skip(1) {
        merged = merge_candidate(&merged, members.first(), item, dedup_only)?;
    }
    Some(merged)
}

/// The filter that would carry `item` alongside `anchor`'s group without
/// changing meaning.
fn merge_candidate(
    filter: &Filter,
    anchor: Option<&RelayDemand>,
    item: &RelayDemand,
    dedup_only: bool,
) -> Option<Filter> {
    if anchor?.bounds != item.bounds {
        return None;
    }
    if filter == &item.filter {
        return Some(filter.clone());
    }
    if dedup_only || filter.limit.is_some() || item.filter.limit.is_some() {
        return None;
    }
    merge_author_axis(filter, &item.filter).or_else(|| merge_tag_axis(filter, &item.filter))
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
