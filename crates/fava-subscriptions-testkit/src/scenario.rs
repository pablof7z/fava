//! One planning situation, and what applying its answer leaves installed.

use std::collections::BTreeSet;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    InstalledSubscription, InstalledSubscriptions, PlanRevision, RelayDemand, RelayReadConstraints,
    SubscriptionPlan, SubscriptionPlanner, validate_plan,
};

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
            revision: PlanRevision(1),
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
/// # Panics
///
/// Panics when the planner refuses input it should have understood, or when the
/// plan violates any rule in [`validate_plan`].
#[must_use]
pub fn assert_conformant(
    planner: &dyn SubscriptionPlanner,
    scenario: &PlannerScenario,
) -> SubscriptionPlan {
    let plan = planner
        .plan(
            &scenario.relay,
            &scenario.demand,
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
        &scenario.demand,
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
