//! Turning one executed plan into the baseline for the next one.
//!
//! `installed` must be exactly what the transport accepted on the current
//! session generation, because the planner reads it to decide attach, residual
//! budget, refcount, and which wire ids are taken. Recording anything the
//! transport refused would make every one of those answers wrong.

use std::collections::BTreeSet;

use fava_subscriptions::{InstalledSubscription, InstalledSubscriptions, SubscriptionPlan};
use fava_wire::SubscriptionId;

/// What the session holds once this plan has been executed.
///
/// On a plan the transport accepted in full this is exactly
/// `fava_subscriptions_testkit::apply_plan`, which this crate's evidence
/// asserts against. It differs only when a REQ was refused: that successor
/// never opened, so it is not installed and its predecessor was not closed.
pub(crate) fn accepted(
    baseline: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
    opened: &BTreeSet<SubscriptionId>,
    closed: &BTreeSet<SubscriptionId>,
) -> InstalledSubscriptions {
    let mut entries = Vec::new();
    for id in baseline.ids() {
        if closed.contains(id) {
            continue;
        }
        let Some(current) = baseline.get(id) else {
            continue;
        };
        let Some(attributed) = plan.attribution.get(id) else {
            continue;
        };
        // A retained subscription keeps its exact filters; only the demand it
        // serves may change, and that is a refcount, not wire work.
        entries.push((
            id.clone(),
            InstalledSubscription {
                filters: current.filters.clone(),
                serves: attributed.serves.clone(),
            },
        ));
    }
    for candidate in &plan.open {
        if !opened.contains(&candidate.id) {
            continue;
        }
        entries.push((
            candidate.id.clone(),
            InstalledSubscription {
                filters: candidate.filters.clone(),
                serves: candidate.serves.clone(),
            },
        ));
    }
    InstalledSubscriptions::from_entries(entries)
}

#[cfg(test)]
mod tests {
    use fava_query::{ObservationId, QueryBounds, QueryBranchId};
    use fava_relay::{RelayAccess, RelaySessionKey};
    use fava_subscriptions::{
        PlanRevision, PlanRevisions, RelayDemand, RelayReadConstraints, SubscriptionPlanner,
    };
    use fava_subscriptions_standard::StandardSubscriptionPlanner;
    use fava_subscriptions_testkit::apply_plan;
    use nostr::event::Kind;
    use nostr::filter::Filter;
    use nostr::key::Keys;
    use nostr::types::RelayUrl;

    use super::*;

    fn relay() -> RelaySessionKey {
        RelaySessionKey {
            relay: RelayUrl::parse("wss://relay.example").expect("relay URL"),
            access: RelayAccess::Public,
        }
    }

    fn revision(sequence: u64) -> PlanRevision {
        let mut revisions = PlanRevisions::new().expect("revision authority");
        let mut current = revisions.allocate().expect("first revision");
        for _ in 1..sequence {
            current = revisions.allocate().expect("requested revision");
        }
        current
    }

    fn demand(owner: u64, filter: Filter) -> RelayDemand {
        RelayDemand::new(
            ObservationId::new(std::num::NonZeroU64::new(owner).expect("non-zero")),
            QueryBranchId::ROOT,
            filter,
            QueryBounds::default(),
        )
    }

    fn ids(installed: &InstalledSubscriptions) -> Vec<SubscriptionId> {
        installed.ids().cloned().collect()
    }

    #[test]
    fn a_fully_accepted_plan_installs_exactly_what_the_reference_reducer_says() {
        let planner = StandardSubscriptionPlanner::new();
        let constraints = RelayReadConstraints::unknown();
        let first = vec![demand(
            1,
            Filter::new().author(Keys::generate().public_key()),
        )];
        let opening = planner
            .plan(
                &relay(),
                &first,
                &constraints,
                &InstalledSubscriptions::empty(),
                revision(1),
            )
            .expect("the planner accepts the cohort");
        let accepted_ids: BTreeSet<SubscriptionId> =
            opening.open.iter().map(|entry| entry.id.clone()).collect();
        let installed = accepted(
            &InstalledSubscriptions::empty(),
            &opening,
            &accepted_ids,
            &BTreeSet::new(),
        );

        assert_eq!(
            ids(&installed),
            ids(&apply_plan(&InstalledSubscriptions::empty(), &opening))
        );

        let second = vec![
            first[0].clone(),
            demand(2, Filter::new().kind(Kind::Metadata)),
        ];
        let growing = planner
            .plan(&relay(), &second, &constraints, &installed, revision(2))
            .expect("the planner accepts the second cohort");
        let opened: BTreeSet<SubscriptionId> =
            growing.open.iter().map(|entry| entry.id.clone()).collect();
        let closed: BTreeSet<SubscriptionId> =
            growing.close.iter().map(|entry| entry.id.clone()).collect();

        assert_eq!(
            ids(&accepted(&installed, &growing, &opened, &closed)),
            ids(&apply_plan(&installed, &growing))
        );
    }

    #[test]
    fn a_refused_request_is_not_installed_and_closes_nothing() {
        let planner = StandardSubscriptionPlanner::new();
        let constraints = RelayReadConstraints::unknown();
        let cohort = vec![demand(
            1,
            Filter::new().author(Keys::generate().public_key()),
        )];
        let opening = planner
            .plan(
                &relay(),
                &cohort,
                &constraints,
                &InstalledSubscriptions::empty(),
                revision(1),
            )
            .expect("the planner accepts the cohort");

        let installed = accepted(
            &InstalledSubscriptions::empty(),
            &opening,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );

        assert!(
            installed.is_empty(),
            "a request the transport refused is never installed"
        );
    }
}
