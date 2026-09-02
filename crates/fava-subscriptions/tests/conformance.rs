//! Executable evidence for the planner conformance rules C1..C11.
//!
//! These are the rules `crates/fava/src/relay.rs` used to enforce privately as
//! nine unspecified string assumptions. Each falsifier below either proves a
//! plan the old code wrongly rejected is now accepted, or proves a rule the old
//! code never had is now enforced.

mod support;

use std::collections::{BTreeSet, VecDeque};
use std::num::NonZeroUsize;

use fava_subscriptions::{
    AttributedSubscription, DeclaredLimit, EoseCompleteness, InstalledSubscription,
    InstalledSubscriptions, PlanConformanceError, PlannedSubscription, RelayReadConstraints,
    ShortfallReason, SubscriptionAttribution, SubscriptionPlan, SubscriptionShortfall,
    validate_plan,
};
use nostr::event::Kind;
use nostr::filter::Filter;
use nostr::types::RelayUrl;
use support::{demand, demand_id, observation, opening, relay, revision, wire};

fn unknown() -> RelayReadConstraints {
    RelayReadConstraints::unknown()
}

fn nothing() -> InstalledSubscriptions {
    InstalledSubscriptions::empty()
}

/// Old rule 7 forbade a REQ carrying more than one filter. NIP-01 permits many,
/// and no authority requires one, so a multi-filter REQ is conformant.
#[test]
fn a_multi_filter_req_planner_is_accepted() {
    let first = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let second = demand(2, Filter::new().kind(Kind::from_u16(7)));
    let id = wire("grouped");
    let plan = opening(
        &id,
        vec![first.filter.clone(), second.filter.clone()],
        [demand_id(1), demand_id(2)].into_iter().collect(),
    );

    validate_plan(&relay(), &[first, second], &unknown(), &nothing(), &plan)
        .expect("a two-filter REQ serving two demands is conformant");
    assert_eq!(plan.open[0].filters.len(), 2);
}

/// Old rules 2 and 3 required a non-empty attribution and a non-empty message
/// list, which made planner-driven withdrawal structurally impossible.
#[test]
fn withdrawal_only_plan_is_conformant() {
    let id = wire("leaving");
    let installed = InstalledSubscriptions::from_entries([(
        id.clone(),
        InstalledSubscription {
            filters: vec![Filter::new().kind(Kind::from_u16(1))],
            serves: [demand_id(1)].into_iter().collect(),
        },
    )]);
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: Vec::new(),
        retain: Vec::new(),
        close: vec![id],
        attribution: SubscriptionAttribution::default(),
        shortfalls: Vec::new(),
    };

    validate_plan(&relay(), &[], &unknown(), &installed, &plan)
        .expect("closing everything for empty demand is the correct plan");
    assert!(!plan.is_noop());
    assert_eq!(plan.installed_count(), 0);
}

/// A plan that carries some demand and reports the rest is `Ok`, not `Err`.
#[test]
fn partial_plan_reports_shortfall_and_still_installs() {
    let carried = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let lost = demand(2, Filter::new().kind(Kind::from_u16(7)));
    let id = wire("carried");
    let mut plan = opening(
        &id,
        vec![carried.filter.clone()],
        [demand_id(1)].into_iter().collect(),
    );
    plan.shortfalls.push(SubscriptionShortfall {
        demand: demand_id(2),
        reason: ShortfallReason::SubscriptionsExhausted {
            required: 2,
            maximum: 1,
        },
    });

    validate_plan(&relay(), &[carried, lost], &unknown(), &nothing(), &plan)
        .expect("a plan may install some demand and report the rest");
    assert_eq!(plan.open.len(), 1);
    assert_eq!(plan.shortfalls.len(), 1);
}

/// C1: routing already chose the relay; a plan for another one is refused.
#[test]
fn a_plan_scoped_to_another_relay_is_refused() {
    let asked = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let id = wire("elsewhere");
    let mut plan = opening(
        &id,
        vec![asked.filter.clone()],
        [demand_id(1)].into_iter().collect(),
    );
    plan.relay = RelayUrl::parse("wss://other.example").expect("relay URL");

    assert_eq!(
        validate_plan(&relay(), &[asked], &unknown(), &nothing(), &plan),
        Err(PlanConformanceError::WrongRelay)
    );
}

/// C2: one wire id cannot be opened and closed by the same plan.
#[test]
fn one_wire_id_cannot_appear_in_two_diff_buckets() {
    let asked = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let id = wire("both");
    // The installed subscription serves nothing, so CR-1 has nothing to protect
    // and the bucket rule is what refuses the plan.
    let installed = InstalledSubscriptions::from_entries([(
        id.clone(),
        InstalledSubscription {
            filters: vec![asked.filter.clone()],
            serves: BTreeSet::new(),
        },
    )]);
    // `open` carries no identity to collide with, so the only way one wire id
    // can appear twice is to be both retained and closed.
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(1),
        open: Vec::new(),
        retain: vec![id.clone()],
        close: vec![id.clone()],
        attribution: SubscriptionAttribution::from_entries([(
            id.clone(),
            AttributedSubscription {
                filters: vec![asked.filter.clone()],
                serves: BTreeSet::new(),
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    };

    assert_eq!(
        validate_plan(&relay(), &[], &unknown(), &installed, &plan),
        Err(PlanConformanceError::OverlappingBuckets(id))
    );
}

/// C4: a plan cannot retain or close something the session never installed.
#[test]
fn retaining_a_subscription_that_is_not_installed_is_refused() {
    let asked = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let id = wire("imagined");
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(1),
        open: Vec::new(),
        retain: vec![id.clone()],
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([(
            id.clone(),
            AttributedSubscription {
                filters: vec![asked.filter.clone()],
                serves: [demand_id(1)].into_iter().collect(),
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    };

    assert_eq!(
        validate_plan(&relay(), &[asked], &unknown(), &nothing(), &plan),
        Err(PlanConformanceError::UnknownInstalled(id))
    );
}

/// C6: a REQ with no filter asks the relay for nothing under a live id.
#[test]
fn a_planned_subscription_without_a_filter_is_refused() {
    let asked = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let id = wire("empty");
    let mut plan = opening(
        &id,
        vec![asked.filter.clone()],
        [demand_id(1)].into_iter().collect(),
    );
    plan.open[0].filters.clear();

    assert_eq!(
        validate_plan(&relay(), &[asked], &unknown(), &nothing(), &plan),
        Err(PlanConformanceError::EmptyFilters(0))
    );
}

/// C8: silently dropping demand is exactly the defect shortfall exists to stop.
#[test]
fn demand_that_is_neither_served_nor_reported_is_refused() {
    let carried = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let dropped = demand(2, Filter::new().kind(Kind::from_u16(7)));
    let id = wire("partial");
    let plan = opening(
        &id,
        vec![carried.filter.clone()],
        [demand_id(1)].into_iter().collect(),
    );

    assert_eq!(
        validate_plan(&relay(), &[carried, dropped], &unknown(), &nothing(), &plan),
        Err(PlanConformanceError::DemandUnaccounted(demand_id(2)))
    );
}

/// C9: attribution that invents demand would settle an observation that never
/// asked this relay for anything.
#[test]
fn attribution_that_invents_demand_is_refused() {
    let asked = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let id = wire("inventive");
    let plan = opening(
        &id,
        vec![asked.filter.clone()],
        [demand_id(1), demand_id(9)].into_iter().collect(),
    );

    assert_eq!(
        validate_plan(&relay(), &[asked], &unknown(), &nothing(), &plan),
        Err(PlanConformanceError::DemandInvented(demand_id(9)))
    );
}

/// C10: a declared subscription ceiling is honored, not clamped silently.
#[test]
fn exceeding_a_declared_subscription_count_is_refused() {
    let first = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let second = demand(2, Filter::new().kind(Kind::from_u16(7)));
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(1),
        open: vec![
            PlannedSubscription {
                filters: vec![first.filter.clone()],
                serves: [demand_id(1)].into_iter().collect(),
                completeness: EoseCompleteness::Proven,
            },
            PlannedSubscription {
                filters: vec![second.filter.clone()],
                serves: [demand_id(2)].into_iter().collect(),
                completeness: EoseCompleteness::Proven,
            },
        ],
        retain: Vec::new(),
        close: Vec::new(),
        // Nothing is retained, so nothing carries a wire id to attribute.
        attribution: SubscriptionAttribution::default(),
        shortfalls: Vec::new(),
    };
    let constraints = RelayReadConstraints {
        max_subscriptions: DeclaredLimit::Declared(NonZeroUsize::new(1).expect("non-zero")),
        ..RelayReadConstraints::unknown()
    };

    assert_eq!(
        validate_plan(&relay(), &[first, second], &constraints, &nothing(), &plan),
        Err(PlanConformanceError::DeclaredSubscriptionsExceeded {
            installed: 2,
            maximum: 1,
        })
    );
}

/// An unknown limit constrains nothing: absence is never an invented default.
#[test]
fn unknown_declared_limits_constrain_nothing() {
    let asked: Vec<_> = (1..=200_u16)
        .map(|index| demand(u64::from(index), Filter::new().kind(Kind::from_u16(index))))
        .collect();
    let mut open = Vec::new();
    for item in &asked {
        open.push(PlannedSubscription {
            filters: vec![item.filter.clone()],
            serves: [item.id()].into_iter().collect(),
            completeness: EoseCompleteness::Proven,
        });
    }
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(1),
        open,
        retain: Vec::new(),
        close: Vec::new(),
        // Nothing is retained, so nothing carries a wire id to attribute.
        attribution: SubscriptionAttribution::default(),
        shortfalls: Vec::new(),
    };

    assert_eq!(DeclaredLimit::default().get(), None);
    validate_plan(&relay(), &asked, &unknown(), &nothing(), &plan)
        .expect("200 subscriptions are conformant when the relay declared nothing");
}

/// Every conformance rule reaches a distinct, non-empty typed message.
#[test]
fn every_conformance_refusal_is_typed_not_a_string() {
    let mut refusals: VecDeque<PlanConformanceError> = VecDeque::new();
    refusals.push_back(PlanConformanceError::WrongRelay);
    refusals.push_back(PlanConformanceError::OverlappingBuckets(wire("a")));
    refusals.push_back(PlanConformanceError::UnknownInstalled(wire("a")));
    refusals.push_back(PlanConformanceError::AttributionMismatch);
    refusals.push_back(PlanConformanceError::EmptyFilters(0));
    refusals.push_back(PlanConformanceError::DemandUnaccounted(demand_id(1)));
    refusals.push_back(PlanConformanceError::DemandInvented(demand_id(1)));
    refusals.push_back(PlanConformanceError::DeclaredSubscriptionsExceeded {
        installed: 2,
        maximum: 1,
    });

    let rendered: BTreeSet<String> = refusals.iter().map(ToString::to_string).collect();
    assert_eq!(rendered.len(), refusals.len());
    assert!(rendered.iter().all(|text| !text.is_empty()));
    assert_ne!(observation(1), observation(2));
}
