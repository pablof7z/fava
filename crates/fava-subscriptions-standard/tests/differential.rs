//! Grouped versus ungrouped: the differential the audit says does not exist.
//!
//! `StandardSubscriptionPlanner` and the no-grouping policy are given the same
//! demand under the same declared constraints. Grouping is only allowed to
//! change how many wire subscriptions are opened. It may not change which
//! events each logical demand receives, which demand an EOSE settles, whether
//! one observation can see another's results, or what cancellation withdraws.

mod support;

use std::collections::BTreeSet;

use fava_subscriptions::{PlanRevision, RelayDemand, SubscriptionPlanner};
use fava_subscriptions_no_grouping::planner as no_grouping;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_subscriptions_testkit::{
    PlannerScenario, assert_planners_agree, assert_withdrawal_agrees,
};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind, Tag};
use nostr::filter::{Filter, SingleLetterTag};
use nostr::key::Keys;
use support::{bounded_demand, declared, demand, demand_id, relay};

fn grouping() -> StandardSubscriptionPlanner {
    StandardSubscriptionPlanner::new()
}

fn tag_key() -> SingleLetterTag {
    SingleLetterTag::from_char('t').expect("tag key")
}

/// A corpus wide enough that a union filter admits events some members of the
/// union never asked for — which is exactly where grouping could leak.
fn corpus(authors: &[Keys], topics: &[&str]) -> Vec<Event> {
    let mut events = Vec::new();
    for author in authors {
        for topic in topics {
            for kind in [1_u16, 7] {
                events.push(
                    EventBuilder::new(Kind::from_u16(kind), format!("about {topic}"))
                        .tag(Tag::hashtag(*topic))
                        .finalize(author)
                        .expect("event signs"),
                );
            }
        }
    }
    events
}

/// Author-axis grouping must not change which events belong to which demand.
#[test]
fn grouped_author_demand_delivers_exactly_what_ungrouped_delivers() {
    let authors: Vec<Keys> = (0..4).map(|_| Keys::generate()).collect();
    let asked: Vec<RelayDemand> = authors
        .iter()
        .enumerate()
        .map(|(index, keys)| {
            demand(
                u64::try_from(index).expect("index fits") + 1,
                Filter::new()
                    .author(keys.public_key())
                    .kind(Kind::from_u16(1)),
            )
        })
        .collect();
    let events = corpus(&authors, &["alpha", "beta"]);
    let scenario = PlannerScenario::fresh("author axis", relay(), asked);

    let report = assert_planners_agree(&grouping(), &no_grouping(), &scenario, &events);

    assert_eq!(report.grouped_wire, 1, "grouping earned its name");
    assert_eq!(report.ungrouped_wire, 4);
    assert!(report.shortfall.is_empty());
}

/// Tag-axis grouping at the RELAY-003 acceptance scale.
#[test]
fn three_hundred_grouped_tag_queries_deliver_exactly_what_ungrouped_delivers() {
    let key = tag_key();
    let asked: Vec<RelayDemand> = (1..=300)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("topic-{index}")),
            )
        })
        .collect();
    let author = Keys::generate();
    let topics: Vec<String> = (1..=300).map(|index| format!("topic-{index}")).collect();
    let sampled: Vec<&str> = topics
        .iter()
        .step_by(37)
        .map(String::as_str)
        .chain(["unasked-topic"])
        .collect();
    let events = corpus(std::slice::from_ref(&author), &sampled);
    let scenario = PlannerScenario::fresh("three hundred tag queries", relay(), asked);

    let report = assert_planners_agree(&grouping(), &no_grouping(), &scenario, &events);

    assert_eq!(report.grouped_wire, 1);
    assert_eq!(report.ungrouped_wire, 300);
}

/// Two observations sharing one wire subscription must not see each other's
/// results: refiltering keeps them isolated under both planners.
#[test]
fn grouping_keeps_two_observations_isolated_from_each_other() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let asked = vec![
        demand(1, Filter::new().author(alice.public_key())),
        demand(2, Filter::new().author(bob.public_key())),
    ];
    let events = corpus(&[alice, bob], &["shared"]);
    let scenario = PlannerScenario::fresh("two observations", relay(), asked);

    let report = assert_planners_agree(&grouping(), &no_grouping(), &scenario, &events);

    assert_eq!(report.grouped_wire, 1, "one wire request serves both");
    assert_ne!(demand_id(1), demand_id(2));
}

/// Demand the planners must refuse to merge still agrees.
#[test]
fn unmergeable_demand_agrees_between_planners() {
    let author = Keys::generate();
    let other = Keys::generate();
    let asked = vec![
        bounded_demand(1, Filter::new().author(author.public_key()), 10),
        bounded_demand(2, Filter::new().author(other.public_key()), 20),
    ];
    let events = corpus(&[author, other], &["gamma"]);
    let scenario = PlannerScenario::fresh("incompatible bounds", relay(), asked);

    let report = assert_planners_agree(&grouping(), &no_grouping(), &scenario, &events);

    assert_eq!(report.grouped_wire, 2);
    assert_eq!(report.ungrouped_wire, 2);
}

/// Cancellation withdraws the same logical demand under both planners.
#[test]
fn cancellation_withdraws_the_same_demand_under_both_planners() {
    let authors: Vec<Keys> = (0..4).map(|_| Keys::generate()).collect();
    let asked: Vec<RelayDemand> = authors
        .iter()
        .enumerate()
        .map(|(index, keys)| {
            demand(
                u64::try_from(index).expect("index fits") + 1,
                Filter::new().author(keys.public_key()),
            )
        })
        .collect();
    let surviving: Vec<RelayDemand> = asked.iter().take(2).cloned().collect();
    let scenario = PlannerScenario::fresh("half cancel", relay(), asked);

    assert_withdrawal_agrees(&grouping(), &no_grouping(), &scenario, &surviving);
}

/// Cancelling everything withdraws everything under both planners.
#[test]
fn cancelling_everything_withdraws_everything_under_both_planners() {
    let key = tag_key();
    let asked: Vec<RelayDemand> = (1..=5)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("topic-{index}")),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("full cancel", relay(), asked);

    assert_withdrawal_agrees(&grouping(), &no_grouping(), &scenario, &[]);
}

/// A declared ceiling makes both planners lose demand, and they must lose the
/// same amount of it — shortfall is a shared fact, not a policy detail.
#[test]
fn a_declared_ceiling_is_reported_by_both_planners() {
    let asked: Vec<RelayDemand> = (1..=4)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let constraints = fava_subscriptions::RelayReadConstraints {
        max_subscriptions: declared(2),
        ..fava_subscriptions::RelayReadConstraints::unknown()
    };
    let scenario = PlannerScenario::fresh("shared ceiling", relay(), asked).declaring(constraints);

    let grouped = fava_subscriptions_testkit::assert_conformant(&grouping(), &scenario);
    let ungrouped = fava_subscriptions_testkit::assert_conformant(&no_grouping(), &scenario);

    let grouped_lost: BTreeSet<_> = grouped.shortfalls.iter().map(|e| e.demand).collect();
    let ungrouped_lost: BTreeSet<_> = ungrouped.shortfalls.iter().map(|e| e.demand).collect();
    assert_eq!(grouped_lost.len(), 2);
    assert_eq!(ungrouped_lost.len(), 2);
    assert_eq!(grouped.revision, PlanRevision(1));
    assert_eq!(ungrouped.revision, PlanRevision(1));
    for plan in [&grouped, &ungrouped] {
        assert_eq!(plan.installed_after().count(), 2);
    }
}

/// A planner that never groups is still a conformant planner: this is the
/// replaceability proof for the contract.
#[test]
fn the_no_grouping_policy_passes_the_same_conformance_kit() {
    let key = tag_key();
    let asked: Vec<RelayDemand> = (1..=20)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("topic-{index}")),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("no grouping conformance", relay(), asked);

    let plan = fava_subscriptions_testkit::assert_conformant(&no_grouping(), &scenario);

    assert_eq!(plan.open.len(), 20);
    assert!(plan.shortfalls.is_empty());
    let _ = no_grouping()
        .plan(
            &scenario.relay,
            &scenario.demand,
            &scenario.constraints,
            &scenario.installed,
            scenario.revision,
        )
        .expect("plans");
}
