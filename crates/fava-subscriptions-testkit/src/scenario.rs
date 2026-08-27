//! One planning situation, and what applying its answer leaves installed.

use std::collections::BTreeSet;

use fava_relay::RelaySessionKey;
use fava_subscriptions::{
    DemandId, InstalledSubscription, InstalledSubscriptions, PlanRevision, PlanRevisions,
    RelayDemand, RelayReadConstraints, SubscriptionPlan, SubscriptionPlanner, validate_plan,
};

fn fresh_revision() -> PlanRevision {
    PlanRevisions::new()
        .expect("test revision authority")
        .allocate()
        .expect("test revision")
}

/// One complete planning situation: the relay, the demand, what the relay
/// declared, and what is already live on the session.
#[derive(Clone, Debug)]
pub struct PlannerScenario {
    /// Human-readable name used in assertion messages.
    pub name: &'static str,
    /// Relay session the plan is scoped to.
    pub relay: RelaySessionKey,
    /// Complete current logical demand for that session.
    pub demand: Vec<RelayDemand>,
    /// Limits the relay declared, or their honest absence.
    pub constraints: RelayReadConstraints,
    /// Wire subscriptions already live on the session.
    pub installed: InstalledSubscriptions,
    /// Revision the caller is planning.
    pub revision: PlanRevision,
}

impl PlannerScenario {
    /// A scenario against a fresh session with nothing declared and nothing
    /// installed.
    #[must_use]
    pub fn fresh(name: &'static str, relay: RelaySessionKey, demand: Vec<RelayDemand>) -> Self {
        Self {
            name,
            relay,
            demand,
            constraints: RelayReadConstraints::unknown(),
            installed: InstalledSubscriptions::empty(),
            revision: fresh_revision(),
        }
    }

    /// The same scenario against different declared limits.
    #[must_use]
    pub fn declaring(mut self, constraints: RelayReadConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// The same scenario continued against a live session.
    #[must_use]
    pub fn continuing(mut self, installed: InstalledSubscriptions, revision: PlanRevision) -> Self {
        self.installed = installed;
        self.revision = revision;
        self
    }

    /// The same scenario with a different demand set — a cancellation, a new
    /// observation, or both.
    #[must_use]
    pub fn demanding(mut self, demand: Vec<RelayDemand>) -> Self {
        self.demand = demand;
        self
    }
}

/// Plan the scenario and prove the answer conforms.
///
/// Every rule in [`validate_plan`] is checked, and so is CR-3, which
/// `validate_plan` structurally cannot see: order invariance is a property of
/// *two* plans, so it is proved here by replanning a reversed demand slice and
/// requiring the identical answer. A planner whose grouping first-fits in slice
/// order churns the wire for demand that has not changed.
///
/// # Panics
///
/// Panics when the planner refuses input it should have understood, when the
/// plan violates any rule in [`validate_plan`], or when the answer depends on
/// the order of the demand slice.
#[must_use]
pub fn assert_conformant(
    planner: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
) -> SubscriptionPlan {
    let plan = plan_once(planner, scenario, &scenario.demand);
    let mut reversed = scenario.demand.clone();
    reversed.reverse();
    let permuted = plan_once(planner, scenario, &reversed);
    assert_eq!(
        plan, permuted,
        "{}: the plan depends on the order of the demand slice, not the demand set",
        scenario.name
    );
    plan
}

/// Plan one demand ordering and check every rule `validate_plan` owns.
fn plan_once(
    planner: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
    demand: &[RelayDemand],
) -> SubscriptionPlan {
    let plan = planner
        .plan(
            &scenario.relay,
            demand,
            &scenario.constraints,
            &scenario.installed,
            scenario.revision,
        )
        .unwrap_or_else(|error| {
            panic!(
                "{}: planner refused conformant input: {error}",
                scenario.name
            )
        });
    if let Err(violation) = validate_plan(
        &scenario.relay,
        demand,
        &scenario.constraints,
        &scenario.installed,
        &plan,
    ) {
        panic!("{}: plan is not conformant: {violation}", scenario.name);
    }
    plan
}

/// What the session holds once this plan has been executed exactly.
///
/// This is how a caller turns one plan into the baseline for the next replan,
/// which is what makes retention and withdrawal observable across revisions.
#[must_use]
pub fn apply_plan(
    baseline: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> InstalledSubscriptions {
    let closed: BTreeSet<_> = plan.close.iter().map(|entry| &entry.id).collect();
    let mut entries = Vec::new();
    for id in baseline.ids() {
        if closed.contains(id) {
            continue;
        }
        let Some(attributed) = plan.attribution.get(id) else {
            continue;
        };
        entries.push((
            id.clone(),
            InstalledSubscription {
                filters: attributed.filters.clone(),
                serves: attributed.serves.clone(),
            },
        ));
    }
    for planned in &plan.open {
        entries.push((
            planned.id.clone(),
            InstalledSubscription {
                filters: planned.filters.clone(),
                serves: planned.serves.clone(),
            },
        ));
    }
    InstalledSubscriptions::from_entries(entries)
}

/// Prove that demand joining a live session never disturbs what is running.
///
/// The scenario is planned and executed; `arriving` is then added to the demand
/// set and the session is replanned. Whatever the planner does with the
/// newcomer it may not touch the incumbents: every running subscription keeps
/// its exact wire id and its exact filters, and none of them is closed.
///
/// A planner that recomputes a desired wire set and diffs it against what is
/// installed fails here, because merging the newcomer changes the incumbent's
/// filter bytes — and the relay then re-serves a stored window it had already
/// finished.
///
/// # Panics
///
/// Panics on the first incumbent the replan disturbs.
pub fn assert_running_subscriptions_are_immutable(
    planner: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
    arriving: &[RelayDemand],
) -> SubscriptionPlan {
    let first = assert_conformant(planner, scenario);
    let installed = apply_plan(&scenario.installed, &first);
    assert!(
        !installed.is_empty(),
        "{}: nothing was installed, so immutability is not being tested",
        scenario.name
    );

    let mut joined = scenario.demand.clone();
    joined.extend(arriving.iter().cloned());
    let next = scenario
        .clone()
        .demanding(joined)
        .continuing(installed.clone(), fresh_revision());
    let replan = assert_conformant(planner, &next);

    for id in installed.ids() {
        assert!(
            !replan.close.iter().any(|withdrawn| &withdrawn.id == id),
            "{}: arriving demand closed running subscription {id}",
            scenario.name
        );
        assert!(
            replan.retain.contains(id),
            "{}: arriving demand stopped retaining running subscription {id}",
            scenario.name
        );
        let before = installed.get(id).expect("installed entry");
        let after = replan
            .attribution
            .get(id)
            .unwrap_or_else(|| panic!("{}: retained {id} lost its attribution", scenario.name));
        assert_eq!(
            before.filters, after.filters,
            "{}: arriving demand rewrote the filters of running subscription {id}",
            scenario.name
        );
    }
    replan
}

/// Prove that demand leaving never narrows a subscription others still hold.
///
/// A grouped subscription that loses one of two owners keeps running unchanged
/// and over-broad; the surplus is discarded by the local per-demand re-match.
/// Narrowing it would cost a full re-serve of the stored window and buy nothing.
///
/// # Panics
///
/// Panics if the replan closes or rewrites a subscription that still has an
/// owner, or if a retained subscription still serves demand that left.
pub fn assert_partial_withdrawal_leaves_the_wire_alone(
    planner: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
    surviving: &[RelayDemand],
) -> SubscriptionPlan {
    let first = assert_conformant(planner, scenario);
    let installed = apply_plan(&scenario.installed, &first);
    let alive: BTreeSet<DemandId> = surviving.iter().map(RelayDemand::id).collect();

    let next = scenario
        .clone()
        .demanding(surviving.to_vec())
        .continuing(installed.clone(), fresh_revision());
    let replan = assert_conformant(planner, &next);

    for id in installed.ids() {
        let before = installed.get(id).expect("installed entry");
        if !before.serves.iter().any(|demand| alive.contains(demand)) {
            continue;
        }
        assert!(
            !replan.close.iter().any(|withdrawn| &withdrawn.id == id),
            "{}: withdrawal closed {id}, which another owner still holds",
            scenario.name
        );
        let after = replan
            .attribution
            .get(id)
            .unwrap_or_else(|| panic!("{}: retained {id} lost its attribution", scenario.name));
        assert_eq!(
            before.filters, after.filters,
            "{}: withdrawal narrowed running subscription {id}",
            scenario.name
        );
        assert!(
            after.serves.iter().all(|demand| alive.contains(demand)),
            "{}: {id} still serves demand that was withdrawn",
            scenario.name
        );
    }
    replan
}
