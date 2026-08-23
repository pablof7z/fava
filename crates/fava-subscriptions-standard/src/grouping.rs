//! Merging compatible unsent demand into the fewest wire filters that preserve
//! meaning.
//!
//! RELAY-003: a planner MAY merge filters that differ in one safely unionable
//! dimension and MUST NOT merge across differences that would change meaning.
//!
//! Everything this module sees is demand that has **not** reached the wire. A
//! subscription the relay is already serving never enters here — merging it
//! would rewrite it, and a rewritten REQ re-serves a window the relay already
//! finished.
//!
//! A candidate carries both its union filter and the exact demand it was merged
//! from. Splitting a candidate therefore re-groups its members rather than
//! truncating a filter, and attribution cannot drift from the filter it
//! describes.

use std::collections::{BTreeMap, BTreeSet};

use fava_subscriptions::{QueryBounds, RelayDemand, RelayReadConstraints};
use nostr::filter::{Filter, SingleLetterTag};

/// Partition unsent demand into the candidate wire subscriptions it can share.
///
/// Three steps:
///
/// 1. canonicalise, then bucket byte-identical filters together regardless of
///    whole-query bounds — two demands asking the relay for exactly the same
///    bytes are one request;
/// 2. merge to a **fixed point** — a merge can unlock a pairing neither operand
///    qualified for, so one greedy pass is not enough once more than one axis
///    exists;
/// 3. fold byte-identical survivors — a merge can recreate a filter the pool
///    already holds.
///
/// Merging is refused across differing whole-query bounds, and refused entirely
/// beyond exact deduplication when the relay declares a default filter limit: a
/// relay-applied default makes a union return fewer events per member than each
/// member would have received alone (GOALS:1049).
pub(crate) fn group(
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
) -> Vec<(Filter, Vec<RelayDemand>)> {
    let mut candidates = bucket_by_filter(&canonical_order(demand));
    if constraints.default_filter_limit.get().is_none() {
        merge_to_fixed_point(&mut candidates);
    }
    fold_identical_unions(candidates)
}

/// Order demand so the plan is a function of the demand set, not the sequence.
///
/// A first-fit pass over an arbitrary slice order can produce a different
/// grouping — and therefore different wire identity — for demand that has not
/// changed. The planner is a published contract with a conformance kit for
/// competing providers and must not carry an unstated ordering precondition
/// (CR-3).
fn canonical_order(demand: &[RelayDemand]) -> Vec<RelayDemand> {
    let mut ordered: Vec<(String, RelayDemand)> = demand
        .iter()
        .map(|item| (canonical_encoding(&item.filter), item.clone()))
        .collect();
    ordered.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.id().cmp(&right.1.id()))
    });
    ordered.into_iter().map(|(_, item)| item).collect()
}

/// Canonical text of one filter, used only for ordering and folding.
fn canonical_encoding(filter: &Filter) -> String {
    serde_json::to_string(filter).unwrap_or_else(|_| format!("{filter:?}"))
}

/// The whole-query bounds every member shares, if they share one.
///
/// A candidate whose members disagree about bounds may still exist — two
/// demands can ask for byte-identical filters under different bounds — but it
/// may never be *widened*, because the wider filter would answer one of them
/// differently from what it asked for.
fn uniform_bounds(members: &[RelayDemand]) -> Option<QueryBounds> {
    let anchor = members.first()?.bounds;
    members
        .iter()
        .all(|item| item.bounds == anchor)
        .then_some(anchor)
}

/// Bucket demand carrying byte-identical filters into one candidate.
///
/// Two byte-identical filters cannot be told apart by any identity scheme. Left
/// separate they become two permanently-live duplicate REQs: the relay
/// double-delivers every matching event, a subscription slot is burned, and
/// completion evidence splits across two identities so neither is credited.
fn bucket_by_filter(demand: &[RelayDemand]) -> Vec<(Filter, Vec<RelayDemand>)> {
    let mut order: Vec<String> = Vec::new();
    let mut buckets: BTreeMap<String, (Filter, Vec<RelayDemand>)> = BTreeMap::new();
    for item in demand {
        let key = canonical_encoding(&item.filter);
        if let Some((_, members)) = buckets.get_mut(&key) {
            members.push(item.clone());
        } else {
            order.push(key.clone());
            buckets.insert(key, (item.filter.clone(), vec![item.clone()]));
        }
    }
    order
        .into_iter()
        .filter_map(|key| buckets.remove(&key))
        .collect()
}

/// Fold candidates whose union filters are byte-identical into one.
///
/// A merge can recreate a filter the pool already holds, so the fold runs again
/// over the survivors rather than only over the inputs.
fn fold_identical_unions(
    candidates: Vec<(Filter, Vec<RelayDemand>)>,
) -> Vec<(Filter, Vec<RelayDemand>)> {
    let mut folded: Vec<(Filter, Vec<RelayDemand>)> = Vec::with_capacity(candidates.len());
    for (filter, members) in candidates {
        if let Some((_, existing)) = folded.iter_mut().find(|(seen, _)| *seen == filter) {
            existing.extend(members);
        } else {
            folded.push((filter, members));
        }
    }
    folded
}

/// Merge candidates pairwise until no pair is mergeable.
///
/// One pass is not enough: merging `{authors:[A]}` with `{authors:[B]}` yields
/// `{authors:[A,B]}`, which a third candidate `{kinds:[2], authors:[A,B]}` is
/// one component from — while it was two components from either original. A
/// single greedy pass silently leaves that collapse on the table.
///
/// The union filter travels with its candidate instead of being recomputed on
/// every comparison, so reaching the fixed point stays quadratic in candidates
/// rather than cubic in demand.
fn merge_to_fixed_point(candidates: &mut Vec<(Filter, Vec<RelayDemand>)>) {
    loop {
        let Some((left, right, merged)) = first_mergeable_pair(candidates) else {
            return;
        };
        let (_, absorbed) = candidates.remove(right);
        candidates[left].0 = merged;
        candidates[left].1.extend(absorbed);
    }
}

/// The first pair of candidates a merge would join, in canonical order.
fn first_mergeable_pair(
    candidates: &[(Filter, Vec<RelayDemand>)],
) -> Option<(usize, usize, Filter)> {
    for left in 0..candidates.len() {
        // A candidate whose own members disagree about whole-query bounds can
        // never be widened, and two candidates bounded differently are answered
        // differently even by one identical filter.
        let Some(bounds) = uniform_bounds(&candidates[left].1) else {
            continue;
        };
        for right in (left + 1)..candidates.len() {
            if uniform_bounds(&candidates[right].1) != Some(bounds) {
                continue;
            }
            if let Some(merged) =
                merge_on_sole_differing_axis(&candidates[left].0, &candidates[right].0)
            {
                return Some((left, right, merged));
            }
        }
    }
    None
}

/// Union two filters that disagree on exactly one value-set axis.
///
/// One function over every axis rather than one function per axis: the axes are
/// copies of a single idea, and separate copies drift.
fn merge_on_sole_differing_axis(left: &Filter, right: &Filter) -> Option<Filter> {
    // Checked before any component comparison, because two *equal* limits
    // produce no differing component and would otherwise sail through. A
    // relay-side `limit` caps a result count, not a predicate: two `limit:200`
    // requests for disjoint author sets promise 400 rows between them, while
    // one merged `limit:200` request still promises 200. Requiring equal rather
    // than absent limits looks like a guard but does not save the widening
    // property.
    if left.limit.is_some() || right.limit.is_some() {
        return None;
    }
    if !remainder_matches(left, right) {
        return None;
    }
    // A tag name present on one side and absent on the other is a disagreement
    // in that name's component, and one this rule refuses: unioning an absent
    // name in produces a filter with no constraint on it at all.
    if !left.generic_tags.keys().eq(right.generic_tags.keys()) {
        return None;
    }

    // Tags are one component **per name**, never one "tags" axis. Tag names are
    // conjunctive across names, so counting them as a single component would
    // let `{#e:X}` and `{#p:Y}` union into a filter demanding both together — a
    // filter matching *neither* operand. That is a narrowing wearing a union's
    // clothes, and it is the single most dangerous mistake available here.
    let differing_tags: Vec<SingleLetterTag> = left
        .generic_tags
        .iter()
        .filter(|(name, values)| right.generic_tags.get(name) != Some(*values))
        .map(|(name, _)| *name)
        .collect();
    let differing = usize::from(left.ids != right.ids)
        + usize::from(left.authors != right.authors)
        + usize::from(left.kinds != right.kinds)
        + differing_tags.len();
    if differing != 1 {
        return None;
    }

    if left.ids != right.ids {
        return union_ids(left, right);
    }
    if left.authors != right.authors {
        return union_authors(left, right);
    }
    if left.kinds != right.kinds {
        return union_kinds(left, right);
    }
    union_tag(left, right, *differing_tags.first()?)
}

/// Whether every field outside the union axes is identical.
///
/// Written as a destructure so a new `nostr::Filter` field fails the build
/// rather than being silently unioned across.
fn remainder_matches(left: &Filter, right: &Filter) -> bool {
    let Filter {
        ids: _,
        authors: _,
        kinds: _,
        search: left_search,
        since: left_since,
        until: left_until,
        limit: _,
        generic_tags: _,
    } = left;
    let Filter {
        ids: _,
        authors: _,
        kinds: _,
        search: right_search,
        since: right_since,
        until: right_until,
        limit: _,
        generic_tags: _,
    } = right;
    // `since`/`until` are bounds, not value sets: there is no union of two
    // windows that is not either a narrowing or a widening far past both
    // operands. A NIP-50 `search` term has no union at all.
    left_search == right_search && left_since == right_since && left_until == right_until
}

/// Union the event-id axis.
fn union_ids(left: &Filter, right: &Filter) -> Option<Filter> {
    let values = union_constrained(left.ids.as_ref(), right.ids.as_ref())?;
    let mut merged = left.clone();
    merged.ids = Some(values);
    Some(merged)
}

/// Union the author axis.
fn union_authors(left: &Filter, right: &Filter) -> Option<Filter> {
    let values = union_constrained(left.authors.as_ref(), right.authors.as_ref())?;
    let mut merged = left.clone();
    merged.authors = Some(values);
    Some(merged)
}

/// Union the kind axis.
fn union_kinds(left: &Filter, right: &Filter) -> Option<Filter> {
    let values = union_constrained(left.kinds.as_ref(), right.kinds.as_ref())?;
    let mut merged = left.clone();
    merged.kinds = Some(values);
    Some(merged)
}

/// Union one `ids`/`authors`/`kinds` axis, refusing an unconstrained operand.
///
/// `None` on these axes is not "the empty set", it is *no constraint on this
/// axis* — and `nostr`'s `match_event` treats `Some(empty)` the same way.
/// Folding either into a constrained sibling produces a filter matching
/// strictly fewer events than one of its own inputs. That is a narrowing, and
/// it is the failure this guard exists for.
fn union_constrained<T: Clone + Ord>(
    left: Option<&BTreeSet<T>>,
    right: Option<&BTreeSet<T>>,
) -> Option<BTreeSet<T>> {
    let left = left.filter(|values| !values.is_empty())?;
    let right = right.filter(|values| !values.is_empty())?;
    Some(left.union(right).cloned().collect())
}

/// Union one tag name's value set.
///
/// The polarity is the inverse of the axes above, and getting it backwards
/// reintroduces the narrowing bug on a new axis. On tags the unconstrained
/// shape is an **absent name** — a filter that never mentions `#t` matches
/// every event, tagged or not — while a **present name with an empty value
/// set** matches nothing, because `match_event` evaluates `any()` over an empty
/// set. Unioning an empty value set in is therefore a widening and is allowed;
/// it is the absent name, refused above, that would narrow.
fn union_tag(left: &Filter, right: &Filter, name: SingleLetterTag) -> Option<Filter> {
    let left_values = left.generic_tags.get(&name)?;
    let right_values = right.generic_tags.get(&name)?;
    let values: BTreeSet<String> = left_values.union(right_values).cloned().collect();
    let mut merged = left.clone();
    merged.generic_tags.insert(name, values);
    Some(merged)
}
