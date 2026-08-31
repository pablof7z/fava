//! Evidence for the merge predicate: which axes union, which refuse, and that
//! merging reaches a fixed point.

mod support;

use std::collections::BTreeSet;

use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_subscriptions_testkit::{PlannerScenario, assert_conformant};
use nostr::event::{EventId, Kind};
use nostr::filter::{Filter, SingleLetterTag};
use nostr::key::Keys;
use support::{demand, relay};

fn planner() -> StandardSubscriptionPlanner {
    StandardSubscriptionPlanner::new()
}

fn tag(key: char) -> SingleLetterTag {
    SingleLetterTag::from_char(key).expect("ASCII letter tag key")
}

fn event(byte: u8) -> EventId {
    EventId::from_slice(&[byte; 32]).expect("32-byte event id")
}

/// C7: three profile kinds for one author is one request, not three. Against a
/// real-world ceiling near twenty concurrent subscriptions this is the common
/// case, not an edge one.
#[test]
fn three_kind_queries_for_one_author_share_one_wire_request() {
    let alice = Keys::generate().public_key();
    let asked: Vec<_> = [0_u16, 3, 10_002]
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            demand(
                u64::try_from(index).expect("index fits") + 1,
                Filter::new().author(alice).kind(Kind::from_u16(kind)),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("three kinds, one author", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    let kinds = plan.open[0].filters[0]
        .kinds
        .as_ref()
        .expect("merged kinds");
    assert_eq!(
        kinds,
        &[0_u16, 3, 10_002]
            .into_iter()
            .map(Kind::from_u16)
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(plan.open[0].serves.len(), 3);
}

/// C11: fetching N events by id is one request, not N.
#[test]
fn separate_event_id_fetches_share_one_wire_request() {
    let asked: Vec<_> = (1..=5_u8)
        .map(|byte| demand(u64::from(byte), Filter::new().id(event(byte))))
        .collect();
    let scenario = PlannerScenario::fresh("five id fetches", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    assert_eq!(plan.open[0].filters[0].ids.as_ref().expect("ids").len(), 5);
}

/// The narrowing guard, on the axis C7 adds. `None` and `Some(empty)` are both
/// unconstrained to `match_event`, so folding either into a constrained sibling
/// produces a filter matching strictly fewer events than one of its own inputs.
#[test]
fn an_unconstrained_kinds_operand_is_never_folded_into_a_constrained_one() {
    let alice = Keys::generate().public_key();
    let constrained = demand(1, Filter::new().author(alice).kind(Kind::from_u16(1)));
    let absent = demand(2, Filter::new().author(alice));
    let empty = demand(
        3,
        Filter {
            authors: Some([alice].into_iter().collect()),
            kinds: Some(BTreeSet::new()),
            ..Filter::new()
        },
    );
    let scenario = PlannerScenario::fresh(
        "unconstrained kinds",
        relay(),
        vec![constrained, absent, empty],
    );

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(
        plan.open.len(),
        3,
        "an unconstrained operand is never unioned into a constrained one"
    );
}

/// The same guard on the authors and ids axes. The two pairs carry distinct
/// `search` terms so the only merge either pair could make is the unconstrained
/// one under test.
#[test]
fn an_unconstrained_author_or_id_operand_is_never_folded_in() {
    let alice = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().search("authors").author(alice)),
        demand(2, Filter::new().search("authors")),
        demand(3, Filter::new().search("ids").id(event(1))),
        demand(4, Filter::new().search("ids")),
    ];
    let scenario = PlannerScenario::fresh("unconstrained operands", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 4);
}

/// C8: a merge can unlock a pairing neither operand qualified for, so one
/// greedy pass is not enough. Without the fixed point this returns two
/// requests and the third candidate never joins.
#[test]
fn a_merge_that_unlocks_a_third_group_reaches_the_fixed_point() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().kind(Kind::from_u16(1)).author(alice)),
        demand(2, Filter::new().kind(Kind::from_u16(1)).author(bob)),
        demand(
            3,
            Filter::new().kind(Kind::from_u16(2)).authors([alice, bob]),
        ),
    ];
    let scenario = PlannerScenario::fresh("cross-axis unlock", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1, "the fixed point collapses all three");
    let merged = &plan.open[0].filters[0];
    assert_eq!(merged.kinds.as_ref().expect("kinds").len(), 2);
    assert_eq!(merged.authors.as_ref().expect("authors").len(), 2);
    assert_eq!(plan.open[0].serves.len(), 3);
}

/// Cross-product refusal: two filters differing on two axes cannot merge. The
/// union would match combinations neither operand asked for.
#[test]
fn two_differing_axes_are_never_merged() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().kind(Kind::from_u16(0)).author(alice)),
        demand(2, Filter::new().kind(Kind::from_u16(1)).author(bob)),
    ];
    let scenario = PlannerScenario::fresh("cross product", relay(), asked);

    assert_eq!(assert_conformant(&planner(), &scenario).open.len(), 2);
}

/// Tags are one component per name and conjunctive across names. Treating them
/// as one axis would union `{#e:X}` and `{#p:Y}` into a filter demanding both
/// together — matching neither operand.
#[test]
fn two_different_tag_names_are_never_merged() {
    let asked = vec![
        demand(1, Filter::new().custom_tag(tag('e'), "x")),
        demand(2, Filter::new().custom_tag(tag('p'), "y")),
        demand(
            3,
            Filter::new()
                .custom_tag(tag('e'), "x")
                .custom_tag(tag('p'), "y"),
        ),
    ];
    let scenario = PlannerScenario::fresh("conjunctive tag names", relay(), asked);

    assert_eq!(assert_conformant(&planner(), &scenario).open.len(), 3);
}

/// C13: on tags the polarity inverts. A present name with an empty value set
/// matches nothing, so unioning it into a sibling that shares the name is a
/// widening and is allowed — unlike the absent name, which is unconstrained.
#[test]
fn an_empty_tag_value_set_unions_into_a_sibling_that_shares_the_name() {
    let key = tag('t');
    let asked = vec![
        demand(
            1,
            Filter::new().custom_tags(key, std::iter::empty::<String>()),
        ),
        demand(2, Filter::new().custom_tag(key, "topic")),
    ];
    let scenario = PlannerScenario::fresh("empty tag values", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    assert_eq!(
        plan.open[0].filters[0]
            .generic_tags
            .get(&key)
            .expect("tag values")
            .len(),
        1
    );
}

/// A `limit` on either side refuses the merge, even when the two limits are
/// equal: a merged `limit:200` request still promises 200 rows where the two
/// originals promised 400 between them.
#[test]
fn equal_limits_still_refuse_a_merge_but_identical_filters_still_share() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().author(alice).limit(200)),
        demand(2, Filter::new().author(bob).limit(200)),
    ];
    let scenario = PlannerScenario::fresh("equal limits", relay(), asked);
    assert_eq!(assert_conformant(&planner(), &scenario).open.len(), 2);

    let shared = Filter::new().author(alice).limit(200);
    let identical = PlannerScenario::fresh(
        "identical limited filters",
        relay(),
        vec![demand(1, shared.clone()), demand(2, shared)],
    );
    let plan = assert_conformant(&planner(), &identical);
    assert_eq!(plan.open.len(), 1, "exact duplicates still share a request");
    assert_eq!(plan.open[0].serves.len(), 2);
}

/// A window is a bound and a `search` term has no union, so neither is ever
/// merged across.
#[test]
fn windows_and_search_terms_are_never_merged_across() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().author(alice).since(1_000.into())),
        demand(2, Filter::new().author(bob).since(2_000.into())),
        demand(3, Filter::new().author(alice).search("one")),
        demand(4, Filter::new().author(bob).search("two")),
    ];
    let scenario = PlannerScenario::fresh("bounds and search", relay(), asked);

    assert_eq!(assert_conformant(&planner(), &scenario).open.len(), 4);
}

/// CR-3 in its own right: the answer is a function of the demand set. The
/// conformance kit already checks a reversal on every plan; this pins the
/// property against several orderings of a set that actually merges.
#[test]
fn grouping_is_invariant_under_demand_permutation() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().kind(Kind::from_u16(1)).author(alice)),
        demand(2, Filter::new().kind(Kind::from_u16(1)).author(bob)),
        demand(
            3,
            Filter::new().kind(Kind::from_u16(2)).authors([alice, bob]),
        ),
        demand(4, Filter::new().search("unmergeable")),
    ];
    let scenario = PlannerScenario::fresh("canonical", relay(), asked.clone());
    let baseline = assert_conformant(&planner(), &scenario);

    for rotation in 1..asked.len() {
        let mut permuted = asked.clone();
        permuted.rotate_left(rotation);
        let plan = assert_conformant(&planner(), &scenario.clone().demanding(permuted));
        assert_eq!(plan, baseline, "rotation {rotation} changed the plan");
    }
}

/// C15, answered rather than left open: access-context partitioning already
/// sits above the merge predicate, structurally.
///
/// `RelaySessionKey` is `(RelayUrl, RelayAccess)` and `plan` is scoped to one
/// of them, so authenticated and unauthenticated demand for the same relay are
/// two different sessions and two different plans. The merge predicate never
/// learns that access exists, and cannot: there is no path by which demand
/// under one access reaches a plan for another. No partition needs adding above
/// `group()`, and this test exists so the question is not re-litigated.
#[test]
fn access_context_partitions_above_the_merge_predicate() {
    use fava_relay::{RelayAccess, RelaySessionKey};
    use nostr::types::RelayUrl;

    let url = RelayUrl::parse("wss://relay.example").expect("relay URL");
    let public = RelaySessionKey {
        relay: url.clone(),
        access: RelayAccess::Public,
    };
    let authenticated = RelaySessionKey {
        relay: url,
        access: RelayAccess::Authenticated(nostr::key::Keys::generate().public_key()),
    };
    assert_ne!(
        public, authenticated,
        "access is part of the session identity a plan is scoped to"
    );

    // The same filter under two accesses is planned twice, never merged once.
    let filter = Filter::new().kind(Kind::from_u16(1));
    let public_plan = assert_conformant(
        &planner(),
        &PlannerScenario::fresh("public", public.clone(), vec![demand(1, filter.clone())]),
    );
    let authenticated_plan = assert_conformant(
        &planner(),
        &PlannerScenario::fresh(
            "authenticated",
            authenticated.clone(),
            vec![demand(2, filter)],
        ),
    );

    assert_eq!(public_plan.relay, public);
    assert_eq!(authenticated_plan.relay, authenticated);
    // A planned subscription is its own attribution; nothing is retained here.
    assert_eq!(public_plan.open[0].serves.len(), 1);
    assert_eq!(authenticated_plan.open[0].serves.len(), 1);
}

#[test]
fn query_access_survives_real_demand_compilation_and_grouping() {
    use std::num::NonZeroU64;

    use fava_query::{ObservationId, Query, QueryBranchId};
    use fava_relay::{RelayAccess, RelaySessionKey};
    use fava_subscriptions::demand_for_query;
    use nostr::types::RelayUrl;

    let url = RelayUrl::parse("wss://same.example").expect("relay URL");
    let authenticated = RelayAccess::Authenticated(Keys::generate().public_key());
    let public_query = Query::events()
        .kinds([Kind::from_u16(1)])
        .expect("one kind is bounded")
        .with_relay_access(RelayAccess::Public);
    let private_query = public_query
        .clone()
        .with_relay_access(authenticated.clone());
    let public_key = RelaySessionKey {
        relay: url.clone(),
        access: public_query.access().clone(),
    };
    let private_key = RelaySessionKey {
        relay: url,
        access: private_query.access().clone(),
    };
    let observation =
        |value| ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity"));
    let public_demands = [
        demand_for_query(observation(1), QueryBranchId::ROOT, &public_query),
        demand_for_query(observation(2), QueryBranchId::ROOT, &public_query),
    ];
    let private_demands = [
        demand_for_query(observation(3), QueryBranchId::ROOT, &private_query),
        demand_for_query(observation(4), QueryBranchId::ROOT, &private_query),
    ];

    let public_plan = assert_conformant(
        &planner(),
        &PlannerScenario::fresh(
            "public grouped",
            public_key.clone(),
            public_demands.to_vec(),
        ),
    );
    let private_plan = assert_conformant(
        &planner(),
        &PlannerScenario::fresh(
            "authenticated grouped",
            private_key.clone(),
            private_demands.to_vec(),
        ),
    );

    assert_eq!(public_plan.relay, public_key);
    assert_eq!(private_plan.relay, private_key);
    assert_ne!(public_plan.relay, private_plan.relay);
    assert_eq!(public_plan.open.len(), 1);
    assert_eq!(private_plan.open.len(), 1);
    assert_eq!(public_plan.open[0].serves.len(), 2);
    assert_eq!(private_plan.open[0].serves.len(), 2);
    assert!(
        public_plan.open[0]
            .serves
            .is_disjoint(&private_plan.open[0].serves),
        "grouping may share only within one exact relay-access plan"
    );
}
