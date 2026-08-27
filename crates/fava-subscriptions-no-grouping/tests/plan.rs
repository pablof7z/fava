//! No-grouping planner contract evidence.

use std::num::{NonZeroU64, NonZeroUsize};

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions::{
    DeclaredLimit, DemandId, InstalledSubscriptions, PlanRevision, PlanRevisions, RelayDemand,
    RelayReadConstraints, ShortfallReason, SubscriptionPlanError, SubscriptionPlanner,
    WithdrawalReason, validate_plan,
};
use fava_subscriptions_no_grouping::planner;
use nostr::filter::Filter;
use nostr::types::RelayUrl;

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

fn demand(value: u64, filter: Filter) -> RelayDemand {
    RelayDemand::new(
        ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity")),
        QueryBranchId::ROOT,
        filter,
        QueryBounds::default(),
    )
}

fn demand_id(value: u64) -> DemandId {
    DemandId {
        owner: ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity")),
        branch: QueryBranchId::ROOT,
    }
}

fn plan_for(
    asked: &[RelayDemand],
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    revision: PlanRevision,
) -> fava_subscriptions::SubscriptionPlan {
    let relay = relay();
    let plan = planner()
        .plan(&relay, asked, constraints, installed, revision)
        .expect("plan is exact");
    validate_plan(&relay, asked, constraints, installed, &plan).expect("plan is conformant");
    plan
}

#[test]
fn each_logical_demand_becomes_one_exact_req_with_attribution() {
    let first = Filter::new().limit(1);
    let second = Filter::new().limit(2);
    let asked = [demand(1, first.clone()), demand(2, second.clone())];

    let plan = plan_for(
        &asked,
        &RelayReadConstraints::unknown(),
        &InstalledSubscriptions::empty(),
        revision(1),
    );

    assert_eq!(plan.relay, relay());
    assert_eq!(plan.open.len(), 2);
    assert_eq!(plan.attribution.len(), 2);
    for planned in &plan.open {
        assert_eq!(planned.serves.len(), 1, "never more than one demand");
        let attributed = plan.attribution.get(&planned.id).expect("attributed");
        assert_eq!(attributed.filters, planned.filters);
    }
    let served: Vec<_> = plan
        .attribution
        .ids()
        .flat_map(|id| plan.attribution.serves(id).iter().copied())
        .collect();
    assert!(served.contains(&demand_id(1)));
    assert!(served.contains(&demand_id(2)));
}

#[test]
fn an_unchanged_demand_is_retained_across_replans() {
    let filter = Filter::new().search("stable");
    let asked = [demand(1, filter.clone())];
    let first = plan_for(
        &asked,
        &RelayReadConstraints::unknown(),
        &InstalledSubscriptions::empty(),
        revision(1),
    );
    let installed = fava_subscriptions::InstalledSubscriptions::from_entries(
        first.open.iter().map(|planned| {
            (
                planned.id.clone(),
                fava_subscriptions::InstalledSubscription {
                    filters: planned.filters.clone(),
                    serves: planned.serves.clone(),
                },
            )
        }),
    );

    let second = plan_for(
        &asked,
        &RelayReadConstraints::unknown(),
        &installed,
        revision(2),
    );

    assert!(second.is_noop());
    assert_eq!(second.retain.len(), 1);
}

#[test]
fn withdrawn_demand_closes_its_own_subscription() {
    let filter = Filter::new().search("leaving");
    let first = plan_for(
        &[demand(1, filter.clone())],
        &RelayReadConstraints::unknown(),
        &InstalledSubscriptions::empty(),
        revision(1),
    );
    let installed = fava_subscriptions::InstalledSubscriptions::from_entries(
        first.open.iter().map(|planned| {
            (
                planned.id.clone(),
                fava_subscriptions::InstalledSubscription {
                    filters: planned.filters.clone(),
                    serves: planned.serves.clone(),
                },
            )
        }),
    );

    let second = plan_for(
        &[],
        &RelayReadConstraints::unknown(),
        &installed,
        revision(2),
    );

    assert_eq!(second.close.len(), 1);
    assert_eq!(
        second.close[0].reason,
        WithdrawalReason::DemandWithdrawn {
            released: [demand_id(1)].into_iter().collect()
        }
    );
}

#[test]
fn a_declared_ceiling_produces_typed_shortfall_not_an_error() {
    let asked: Vec<_> = (1..=3)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let constraints = RelayReadConstraints {
        max_subscriptions: DeclaredLimit::Declared(NonZeroUsize::new(2).expect("non-zero")),
        ..RelayReadConstraints::unknown()
    };

    let plan = plan_for(
        &asked,
        &constraints,
        &InstalledSubscriptions::empty(),
        revision(1),
    );

    assert_eq!(plan.open.len(), 2);
    assert_eq!(plan.shortfalls.len(), 1);
    assert_eq!(
        plan.shortfalls[0].reason,
        ShortfallReason::SubscriptionsExhausted {
            required: 3,
            maximum: 2
        }
    );
}

#[test]
fn two_demands_with_one_identity_are_refused() {
    let filter = Filter::new().search("duplicate");
    let asked = [demand(1, filter.clone()), demand(1, filter)];

    let error = planner()
        .plan(
            &relay(),
            &asked,
            &RelayReadConstraints::unknown(),
            &InstalledSubscriptions::empty(),
            revision(1),
        )
        .expect_err("one logical identity cannot appear twice");

    assert_eq!(error, SubscriptionPlanError::DuplicateDemand(demand_id(1)));
}
