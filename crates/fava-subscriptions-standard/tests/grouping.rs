//! Standard planner grouping, retention, withdrawal, and declared-limit evidence.

mod support;

use std::collections::BTreeSet;

use fava_subscriptions::{
    DeclaredLimit, InstalledSubscriptions, PlanRevision, RelayReadConstraints, ShortfallReason,
    SubscriptionPlanError, SubscriptionPlanner, WithdrawalReason,
};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_subscriptions_testkit::{PlannerScenario, apply_plan, assert_conformant};
use nostr::filter::{Filter, SingleLetterTag};
use nostr::key::Keys;
use support::{
    bounded_demand, declared, declaring_subscriptions, demand, demand_id, observation, relay,
};

fn planner() -> StandardSubscriptionPlanner {
    StandardSubscriptionPlanner::new()
}

/// RELAY-003 acceptance: 300 compatible tag-value queries share one wire
/// request while each logical query keeps its own identity and evidence.
#[test]
fn three_hundred_compatible_tag_queries_share_one_wire_request() {
    let key = SingleLetterTag::from_char('t').expect("tag key");
    let asked: Vec<_> = (1..=300)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("topic-{index}")),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("three hundred tag queries", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1, "compatible tag demand shares one REQ");
    assert_eq!(plan.attribution.len(), 1);
    let id = plan.open[0].id.clone();
    assert_eq!(
        plan.attribution.serves(&id).len(),
        300,
        "one EOSE settles every logical query it served"
    );
    for index in 1..=300 {
        assert!(plan.attribution.serves(&id).contains(&demand_id(index)));
    }
    assert!(plan.shortfalls.is_empty());
}

/// The whole reason aggregate demand must reach the planner: two observations
/// of compatible author demand become one wire subscription.
#[test]
fn compatible_author_demand_from_two_observations_shares_one_subscription() {
    let authors = [Keys::generate().public_key(), Keys::generate().public_key()];
    let asked: Vec<_> = authors
        .iter()
        .enumerate()
        .map(|(index, author)| {
            demand(
                u64::try_from(index).expect("index fits") + 1,
                Filter::new().author(*author),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("two observations, one axis", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    assert_eq!(plan.open[0].serves.len(), 2);
    assert_eq!(
        plan.open[0].filters[0]
            .authors
            .as_ref()
            .expect("merged authors")
            .len(),
        2
    );
}

/// §8 falsifier: replanning leaves an unchanged wire subscription untouched.
#[test]
fn replanning_retains_unchanged_wire_subscriptions() {
    let key = SingleLetterTag::from_char('t').expect("tag key");
    let stable = demand(1, Filter::new().custom_tag(key, "stable"));
    let first = PlannerScenario::fresh("first revision", relay(), vec![stable.clone()]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );
    assert_eq!(installed.len(), 1);

    let arriving = demand(2, Filter::new().search("something else entirely"));
    let second = first
        .clone()
        .demanding(vec![stable, arriving])
        .continuing(installed, PlanRevision::new(2));

    let plan = assert_conformant(&planner(), &second);

    assert_eq!(plan.open.len(), 1, "only the new demand opens a REQ");
    assert_eq!(plan.retain.len(), 1, "the unchanged REQ is left alone");
    assert!(plan.close.is_empty());
    assert_eq!(plan.attribution.len(), 2);
}

/// A second observation joining an already-installed grouped subscription
/// changes attribution without touching the wire.
#[test]
fn joining_demand_reuses_the_installed_subscription_without_a_frame() {
    let filter = Filter::new().search("identical");
    let first = PlannerScenario::fresh("first holder", relay(), vec![demand(1, filter.clone())]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );

    let second = first
        .clone()
        .demanding(vec![demand(1, filter.clone()), demand(2, filter)])
        .continuing(installed, PlanRevision::new(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.is_noop(), "a second holder emits no frame");
    assert_eq!(plan.retain.len(), 1);
    let id = plan.retain[0].clone();
    assert_eq!(plan.attribution.serves(&id).len(), 2);
}

/// §8 falsifier: an empty demand set against a live session closes everything.
#[test]
fn withdrawal_only_plan_is_conformant() {
    let asked = demand(1, Filter::new().search("leaving"));
    let first = PlannerScenario::fresh("holder present", relay(), vec![asked]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );

    let second = first
        .clone()
        .demanding(Vec::new())
        .continuing(installed, PlanRevision::new(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.open.is_empty());
    assert!(plan.retain.is_empty());
    assert_eq!(plan.close.len(), 1);
    assert_eq!(
        plan.close[0].reason,
        WithdrawalReason::DemandWithdrawn {
            released: [demand_id(1)].into_iter().collect()
        }
    );
}

/// Refcounted withdrawal: one of two holders leaving keeps the wire open.
#[test]
fn one_of_two_holders_leaving_keeps_the_subscription_open() {
    let filter = Filter::new().search("shared");
    let both = vec![demand(1, filter.clone()), demand(2, filter.clone())];
    let first = PlannerScenario::fresh("two holders", relay(), both);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );

    let second = first
        .clone()
        .demanding(vec![demand(2, filter)])
        .continuing(installed, PlanRevision::new(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.close.is_empty(), "the surviving holder keeps it open");
    assert_eq!(plan.retain.len(), 1);
    let id = plan.retain[0].clone();
    assert_eq!(
        plan.attribution.serves(&id),
        &[demand_id(2)].into_iter().collect::<BTreeSet<_>>()
    );
}

/// §8 falsifier: a declared ceiling produces typed shortfall inside `Ok`, and
/// the demand that lost is named.
#[test]
fn partial_plan_reports_shortfall_and_still_installs() {
    let asked: Vec<_> = (1..=3)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let scenario = PlannerScenario::fresh("ceiling of two", relay(), asked)
        .declaring(declaring_subscriptions(2));

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 2, "two of three still install");
    assert_eq!(plan.shortfalls.len(), 1);
    assert_eq!(
        plan.shortfalls[0].reason,
        ShortfallReason::SubscriptionsExhausted {
            required: 3,
            maximum: 2
        }
    );
    let carried: BTreeSet<_> = plan
        .attribution
        .ids()
        .flat_map(|id| plan.attribution.serves(id).iter().copied())
        .collect();
    assert!(!carried.contains(&plan.shortfalls[0].demand));
}

/// A shortfall is attributable to the exact demand that lost, and stays that
/// way across replans with the same inputs.
#[test]
fn shortfall_names_the_same_demand_on_every_replan() {
    let asked: Vec<_> = (1..=4)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let scenario = PlannerScenario::fresh("stable loser", relay(), asked)
        .declaring(declaring_subscriptions(2));

    let first = assert_conformant(&planner(), &scenario);
    let second = assert_conformant(&planner(), &scenario);

    assert_eq!(first.shortfalls, second.shortfalls);
    assert_eq!(first.shortfalls.len(), 2);
}

/// A ceiling reached later does not churn the subscriptions already live.
#[test]
fn a_declared_ceiling_keeps_installed_subscriptions_first() {
    let early: Vec<_> = (1..=2)
        .map(|index| demand(index, Filter::new().search(format!("early-{index}"))))
        .collect();
    let first = PlannerScenario::fresh("before the ceiling", relay(), early.clone())
        .declaring(declaring_subscriptions(2));
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );
    let live: BTreeSet<_> = installed.ids().cloned().collect();

    let mut later = early;
    later.push(demand(3, Filter::new().search("late")));
    let second = first
        .clone()
        .demanding(later)
        .continuing(installed, PlanRevision::new(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.open.is_empty());
    assert!(plan.close.is_empty());
    assert_eq!(plan.retain.iter().cloned().collect::<BTreeSet<_>>(), live);
    assert_eq!(plan.shortfalls.len(), 1);
    assert_eq!(plan.shortfalls[0].demand, demand_id(3));
}

/// RELAY-003: differing whole-query bounds are a difference that changes
/// meaning, so they are never merged.
#[test]
fn demand_with_differing_bounds_is_never_merged() {
    let author = Keys::generate().public_key();
    let other = Keys::generate().public_key();
    let asked = vec![
        bounded_demand(1, Filter::new().author(author), 10),
        bounded_demand(2, Filter::new().author(other), 20),
    ];
    let scenario = PlannerScenario::fresh("incompatible bounds", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 2);
}

/// GOALS:1049: a relay-declared default limit makes a union return fewer events
/// per member than each member would have received alone, so only exact
/// duplicates may still be deduplicated.
#[test]
fn a_declared_default_filter_limit_forbids_axis_merging() {
    let author = Keys::generate().public_key();
    let other = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().author(author)),
        demand(2, Filter::new().author(other)),
    ];
    let constraints = RelayReadConstraints {
        default_filter_limit: declared(100),
        ..RelayReadConstraints::unknown()
    };
    let scenario =
        PlannerScenario::fresh("declared default limit", relay(), asked).declaring(constraints);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 2, "axis merging is unsafe under a default");
}

/// The same demand under unknown constraints still merges: a declared default
/// is the only thing that forbids it, and absence is never invented.
#[test]
fn the_same_demand_merges_when_no_default_limit_is_declared() {
    let author = Keys::generate().public_key();
    let other = Keys::generate().public_key();
    let asked = vec![
        demand(1, Filter::new().author(author)),
        demand(2, Filter::new().author(other)),
    ];
    let scenario = PlannerScenario::fresh("nothing declared", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
}

/// RELAY-004: a declared filter limit below the demand's own is typed shortfall
/// attributed to that demand, not a clamp.
#[test]
fn demand_exceeding_a_declared_filter_limit_is_typed_shortfall() {
    let asked = vec![
        demand(1, Filter::new().search("small").limit(10)),
        demand(2, Filter::new().search("large").limit(5_000)),
    ];
    let constraints = RelayReadConstraints {
        max_filter_limit: declared(100),
        ..RelayReadConstraints::unknown()
    };
    let scenario =
        PlannerScenario::fresh("declared filter limit", relay(), asked).declaring(constraints);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    assert_eq!(
        plan.shortfalls,
        vec![fava_subscriptions::SubscriptionShortfall {
            demand: demand_id(2),
            reason: ShortfallReason::FilterLimitExceeded {
                required: 5_000,
                maximum: 100,
            },
        }]
    );
}

/// RELAY-004: a declared message bound splits a merged REQ into exact subsets
/// rather than truncating it.
#[test]
fn a_declared_message_bound_splits_rather_than_truncates() {
    let key = SingleLetterTag::from_char('t').expect("tag key");
    let asked: Vec<_> = (1..=8)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("value-{index:0>32}")),
            )
        })
        .collect();
    let constraints = RelayReadConstraints {
        max_message_bytes: declared(220),
        ..RelayReadConstraints::unknown()
    };
    let scenario = PlannerScenario::fresh("declared message bound", relay(), asked.clone())
        .declaring(constraints);

    let plan = assert_conformant(&planner(), &scenario);

    assert!(plan.open.len() > 1, "the merged REQ is split");
    assert!(plan.shortfalls.is_empty(), "splitting carries every demand");
    let served: BTreeSet<_> = plan
        .attribution
        .ids()
        .flat_map(|id| plan.attribution.serves(id).iter().copied())
        .collect();
    assert_eq!(served.len(), asked.len());
}

/// RELAY-004: identifiers are never silently collided under a declared
/// id-length bound.
#[test]
fn a_declared_id_length_is_honored_or_reported() {
    let asked: Vec<_> = (1..=4)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let constraints = RelayReadConstraints {
        max_subscription_id_chars: declared(1),
        ..RelayReadConstraints::unknown()
    };
    let scenario =
        PlannerScenario::fresh("declared id length", relay(), asked).declaring(constraints);

    let plan = assert_conformant(&planner(), &scenario);

    for id in plan.installed_after() {
        assert_eq!(id.as_str().chars().count(), 1);
    }
    let ids: BTreeSet<_> = plan.installed_after().cloned().collect();
    assert_eq!(ids.len(), plan.installed_after().count(), "no collision");
}

/// `Err` is reserved for input the planner cannot process at all.
#[test]
fn two_demands_with_one_identity_are_refused() {
    let filter = Filter::new().search("duplicate");
    let asked = vec![demand(1, filter.clone()), demand(1, filter)];

    let error = planner()
        .plan(
            &relay(),
            &asked,
            &RelayReadConstraints::unknown(),
            &InstalledSubscriptions::empty(),
            PlanRevision::new(1),
        )
        .expect_err("one logical identity cannot appear twice");

    assert_eq!(error, SubscriptionPlanError::DuplicateDemand(demand_id(1)));
    assert_ne!(observation(1), observation(2));
    assert_eq!(DeclaredLimit::Unknown.get(), None);
}
