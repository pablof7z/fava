//! Grouped versus ungrouped: the differential that proves grouping did not
//! change meaning.

use std::collections::{BTreeMap, BTreeSet};

use fava_subscriptions::{
    DemandId, PlanRevision, RelayDemand, SubscriptionPlan, SubscriptionPlanner,
};
use fava_wire::SubscriptionId;
use nostr::event::Event;
use nostr::filter::{Filter, MatchEventOptions};

use crate::scenario::{PlannerScenario, assert_conformant};

/// What one differential run observed, for a caller that wants to assert more.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DifferentialReport {
    /// Wire subscriptions the grouping planner opened.
    pub grouped_wire: usize,
    /// Wire subscriptions the ungrouped planner opened.
    pub ungrouped_wire: usize,
    /// Demand each planner could not carry.
    pub shortfall: BTreeSet<DemandId>,
}

/// Prove two planners give the same demand the same meaning.
///
/// Both plans are checked against [`fava_subscriptions::validate_plan`], then
/// compared on the four things grouping could silently break:
///
/// 1. **query meaning** — the events each logical demand receives, after the
///    local refiltering a grouped wire subscription requires, are exactly the
///    events its own filter selects;
/// 2. **access isolation** — no demand receives an event its own filter rejects,
///    so one observation never sees another's results;
/// 3. **evidence** — an EOSE on any wire subscription settles exactly the same
///    logical demand under both planners.
///
/// Cancellation is the fourth, and lives in [`assert_withdrawal_agrees`]
/// because it needs a second demand set.
///
/// # Panics
///
/// Panics on the first divergence, naming the scenario and the demand.
pub fn assert_planners_agree(
    grouping: &dyn SubscriptionPlanner,
    ungrouped: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
    corpus: &[Event],
) -> DifferentialReport {
    let grouped_plan = assert_conformant(grouping, scenario);
    let ungrouped_plan = assert_conformant(ungrouped, scenario);
    let wanted = wanted_filters(&scenario.demand);

    let grouped_short = shortfall_of(&grouped_plan);
    let ungrouped_short = shortfall_of(&ungrouped_plan);
    assert_eq!(
        grouped_short, ungrouped_short,
        "{}: planners disagree about which demand could not be carried",
        scenario.name
    );

    for event in corpus {
        let grouped = delivered_to(&grouped_plan, &wanted, event);
        let ungrouped = delivered_to(&ungrouped_plan, &wanted, event);
        assert_eq!(
            grouped, ungrouped,
            "{}: grouped and ungrouped delivery diverge for event {}",
            scenario.name, event.id
        );
        let expected: BTreeSet<DemandId> = wanted
            .iter()
            .filter(|(demand, filter)| {
                !grouped_short.contains(*demand)
                    && filter.match_event(event, MatchEventOptions::new())
            })
            .map(|(demand, _)| *demand)
            .collect();
        assert_eq!(
            grouped, expected,
            "{}: grouping changed which demand event {} belongs to",
            scenario.name, event.id
        );
    }

    for id in grouped_plan.attribution.ids() {
        let settled = settled_by_eose_on(&grouped_plan, id);
        for demand in settled {
            assert!(
                wanted.contains_key(&demand),
                "{}: a grouped EOSE would settle demand that was never asked for",
                scenario.name
            );
        }
    }
    assert_eq!(
        every_settled(&grouped_plan),
        every_settled(&ungrouped_plan),
        "{}: grouped and ungrouped EOSE settle different logical demand",
        scenario.name
    );

    DifferentialReport {
        grouped_wire: grouped_plan.attribution.len(),
        ungrouped_wire: ungrouped_plan.attribution.len(),
        shortfall: grouped_short,
    }
}

/// Prove cancellation means the same thing to both planners.
///
/// The scenario is planned, executed, and replanned against `surviving`. Under
/// both planners: every surviving demand must still be served or reported as
/// shortfall, no withdrawn demand may still be served, and every wire
/// subscription the plan closes must be one that no surviving demand needs.
///
/// # Panics
///
/// Panics on the first divergence, naming the scenario.
pub fn assert_withdrawal_agrees(
    grouping: &dyn SubscriptionPlanner,
    ungrouped: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
    surviving: &[RelayDemand],
) {
    let withdrawn: BTreeSet<DemandId> = scenario
        .demand
        .iter()
        .map(RelayDemand::id)
        .filter(|id| !surviving.iter().any(|item| item.id() == *id))
        .collect();
    let alive: BTreeSet<DemandId> = surviving.iter().map(RelayDemand::id).collect();

    for planner in [grouping, ungrouped] {
        let first = assert_conformant(planner, scenario);
        let installed = crate::scenario::apply_plan(&scenario.installed, &first);
        let next = scenario
            .clone()
            .demanding(surviving.to_vec())
            .continuing(installed.clone(), PlanRevision(scenario.revision.0 + 1));
        let replan = assert_conformant(planner, &next);

        let served = every_settled(&replan);
        for demand in &withdrawn {
            assert!(
                !served.contains(demand),
                "{}: withdrawn demand is still served after replan",
                scenario.name
            );
        }
        let short = shortfall_of(&replan);
        for demand in &alive {
            assert!(
                served.contains(demand) || short.contains(demand),
                "{}: a surviving demand is neither served nor reported as shortfall",
                scenario.name
            );
        }
        for closed in &replan.close {
            let still_needed = installed
                .get(&closed.id)
                .is_some_and(|entry| entry.serves.iter().any(|demand| alive.contains(demand)))
                && replan.retain.contains(&closed.id);
            assert!(
                !still_needed,
                "{}: replan closes a subscription it also retains",
                scenario.name
            );
        }
    }
}

/// Logical demand one event reaches under this plan, after local refiltering.
///
/// A grouped wire subscription is a union: the relay may return an event that
/// only some of its members asked for. Refiltering against each member's own
/// filter is what keeps the union honest, and is exactly what this models.
#[must_use]
pub fn delivered_to(
    plan: &SubscriptionPlan,
    wanted: &BTreeMap<DemandId, Filter>,
    event: &Event,
) -> BTreeSet<DemandId> {
    let mut reached = BTreeSet::new();
    for id in plan.attribution.ids() {
        let Some(entry) = plan.attribution.get(id) else {
            continue;
        };
        if !entry
            .filters
            .iter()
            .any(|filter| filter.match_event(event, MatchEventOptions::new()))
        {
            continue;
        }
        for demand in &entry.serves {
            if wanted
                .get(demand)
                .is_some_and(|filter| filter.match_event(event, MatchEventOptions::new()))
            {
                reached.insert(*demand);
            }
        }
    }
    reached
}

/// Logical demand an EOSE on one wire subscription settles.
#[must_use]
pub fn settled_by_eose_on(plan: &SubscriptionPlan, id: &SubscriptionId) -> BTreeSet<DemandId> {
    plan.attribution.serves(id).clone()
}

/// Every logical demand some EOSE in this plan would settle.
fn every_settled(plan: &SubscriptionPlan) -> BTreeSet<DemandId> {
    plan.attribution
        .ids()
        .flat_map(|id| plan.attribution.serves(id).iter().copied())
        .collect()
}

/// Demand the plan reports it did not carry.
fn shortfall_of(plan: &SubscriptionPlan) -> BTreeSet<DemandId> {
    plan.shortfalls.iter().map(|entry| entry.demand).collect()
}

/// Each demand's own filter, which is the authority on what belongs to it.
fn wanted_filters(demand: &[RelayDemand]) -> BTreeMap<DemandId, Filter> {
    demand
        .iter()
        .map(|item| (item.id(), item.filter.clone()))
        .collect()
}
