//! The pending-admission cohort and the coverage test that lets demand attach.
//!
//! A wire subscription that has reached the socket is immutable. Demand that
//! has *not* reached the wire batches in one fixed, first-arrival-anchored,
//! non-sliding window and is compiled as a single cohort against an empty
//! incumbent namespace, so the merge step structurally cannot widen a live
//! request. Demand arriving after the freeze either attaches to an incumbent
//! that already physically covers it, or opens its own request alongside it.
//!
//! Rewriting a running subscription costs the relay a full re-serve of the
//! window it already served, and the cost is quadratic in the number of growth
//! steps. It is never taken.

use std::collections::BTreeSet;
use std::time::Duration;

use fava_subscriptions::{DemandId, RelayDemand};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;

/// Fixed wire-admission window, anchored at the first uncovered demand.
///
/// Repeated arming while a window is pending never extends it: a sliding
/// deadline starves under a steady arrival stream.
pub(crate) const ADMISSION_WINDOW: Duration = Duration::from_millis(10);

/// One wire subscription currently live on a relay session generation.
///
/// The filter is immutable for the life of the subscription. `serves` is the
/// refcount: the subscription is withdrawn when, and only when, it empties.
#[derive(Clone, Debug)]
pub(crate) struct LiveSubscription {
    /// Filters the REQ carries. Never rewritten.
    pub(crate) filters: Vec<Filter>,
    /// Logical demand this subscription serves.
    pub(crate) serves: BTreeSet<DemandId>,
    /// Whether the relay has sent EOSE for this exact request.
    pub(crate) stored_events_complete: bool,
}

/// Whether one incumbent request can serve a later demand without any wire work.
///
/// Exact filter equality always attaches. Beyond that this is a real
/// containment test: the incumbent must be unconstrained wherever the candidate
/// is constrained, and a superset wherever both are. A limited request is
/// exact-only, because a result-count boundary is not a set axis and cannot be
/// reconstructed for a later owner.
pub(crate) fn covers(incumbent: &Filter, candidate: &Filter) -> bool {
    if incumbent == candidate {
        return true;
    }
    if incumbent.limit.is_some() || candidate.limit.is_some() {
        return false;
    }
    if incumbent.search != candidate.search {
        return false;
    }
    covers_set(incumbent.ids.as_ref(), candidate.ids.as_ref())
        && covers_set(incumbent.authors.as_ref(), candidate.authors.as_ref())
        && covers_set(incumbent.kinds.as_ref(), candidate.kinds.as_ref())
        && covers_tags(incumbent, candidate)
        && covers_since(incumbent, candidate)
        && covers_until(incumbent, candidate)
}

/// Whether any filter of one incumbent request covers the candidate filter.
pub(crate) fn attaches(incumbent: &LiveSubscription, candidate: &Filter) -> bool {
    incumbent
        .filters
        .iter()
        .any(|filter| covers(filter, candidate))
}

/// Whether every filter of one planned request is already covered.
pub(crate) fn attaches_all(incumbent: &LiveSubscription, candidates: &[Filter]) -> bool {
    !candidates.is_empty()
        && candidates
            .iter()
            .all(|candidate| attaches(incumbent, candidate))
}

/// The demand identities one cohort carries.
pub(crate) fn identities(cohort: &[RelayDemand]) -> BTreeSet<DemandId> {
    cohort.iter().map(RelayDemand::id).collect()
}

/// A wire id this slot has already retired.
///
/// Reusing one would let a straggler EOSE for the closed request settle the
/// fresh one, which `GOALS:426` (QUERY-010) forbids by name.
pub(crate) fn is_retired(retired: &BTreeSet<SubscriptionId>, id: &SubscriptionId) -> bool {
    retired.contains(id)
}

fn covers_set<T: Ord>(incumbent: Option<&BTreeSet<T>>, candidate: Option<&BTreeSet<T>>) -> bool {
    match (incumbent, candidate) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => narrow.is_subset(wide),
    }
}

fn covers_tags(incumbent: &Filter, candidate: &Filter) -> bool {
    incumbent.generic_tags.iter().all(|(key, wide)| {
        candidate
            .generic_tags
            .get(key)
            .is_some_and(|narrow| narrow.is_subset(wide))
    })
}

fn covers_since(incumbent: &Filter, candidate: &Filter) -> bool {
    match (incumbent.since, candidate.since) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => wide <= narrow,
    }
}

fn covers_until(incumbent: &Filter, candidate: &Filter) -> bool {
    match (incumbent.until, candidate.until) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(wide), Some(narrow)) => wide >= narrow,
    }
}

#[cfg(test)]
mod tests {
    use nostr::event::Kind;
    use nostr::key::Keys;

    use super::*;

    #[test]
    fn an_identical_filter_always_attaches() {
        let filter = Filter::new().kind(Kind::TextNote);

        assert!(covers(&filter.clone(), &filter));
    }

    #[test]
    fn an_unconstrained_axis_covers_a_constrained_one() {
        let wide = Filter::new().kind(Kind::TextNote);
        let narrow = Filter::new()
            .kind(Kind::TextNote)
            .author(Keys::generate().public_key());

        assert!(covers(&wide, &narrow));
        assert!(!covers(&narrow, &wide));
    }

    #[test]
    fn a_superset_author_list_covers_a_subset() {
        let alice = Keys::generate().public_key();
        let bob = Keys::generate().public_key();
        let wide = Filter::new().authors([alice, bob]);
        let narrow = Filter::new().authors([alice]);

        assert!(covers(&wide, &narrow));
        assert!(!covers(&narrow, &wide));
    }

    #[test]
    fn a_limited_request_attaches_only_on_exact_equality() {
        let limited = Filter::new().kind(Kind::TextNote).limit(10);
        let narrow = Filter::new()
            .kind(Kind::TextNote)
            .author(Keys::generate().public_key());

        assert!(!covers(&limited, &narrow));
        assert!(covers(&limited.clone(), &limited));
    }

    #[test]
    fn a_wider_time_window_covers_a_narrower_one() {
        let wide = Filter::new().since(fava_state::Timestamp::from(10));
        let narrow = Filter::new().since(fava_state::Timestamp::from(20));

        assert!(covers(&wide, &narrow));
        assert!(!covers(&narrow, &wide));
    }

    #[test]
    fn an_unconstrained_candidate_is_never_covered_by_a_constrained_incumbent() {
        let wide = Filter::new().kind(Kind::TextNote);

        assert!(!covers(&wide, &Filter::new()));
    }
}
