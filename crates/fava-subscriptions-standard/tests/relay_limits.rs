//! Declared relay limits produce an exact plan or an exact shortfall.

use std::num::NonZeroUsize;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{
    RelayDemand, RelayLimits, SubscriptionPlanError, SubscriptionPlanner, demand_for_query,
};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_wire::SubscriptionId;
use nostr::key::Keys;

fn relay() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("ws://127.0.0.1:9").expect("relay URL"),
        RelayAccess::public(),
    )
}

/// Distinct demand that a grouping planner cannot merge into one REQ.
fn ungroupable(count: usize) -> Vec<RelayDemand> {
    (0..count)
        .map(|index| {
            let query = fava_query::Query::events()
                .authors([Keys::generate().public_key()])
                .limit(index + 1)
                .expect("limit is non-zero");
            demand_for_query(SubscriptionId::new(format!("fava-{index}")), &query)
        })
        .collect()
}

#[test]
fn an_unknown_relay_claim_leaves_favas_own_bound_in_force() {
    let planner = StandardSubscriptionPlanner::bounded(
        NonZeroUsize::new(2).expect("non-zero"),
        NonZeroUsize::new(1_048_576).expect("non-zero"),
    );
    let plan = planner
        .plan(&relay(), &RelayLimits::unknown(), &ungroupable(2))
        .expect("two subscriptions are inside Fava's own bound");
    assert_eq!(plan.messages.len(), 2);

    assert_eq!(
        planner.plan(&relay(), &RelayLimits::unknown(), &ungroupable(3)),
        Err(SubscriptionPlanError::TooManySubscriptions {
            required: 3,
            maximum: 2,
        })
    );
}

#[test]
fn a_stricter_relay_subscription_claim_produces_exact_shortfall_not_omission() {
    let planner = StandardSubscriptionPlanner::default();
    let limits = RelayLimits {
        max_subscriptions: Some(1),
        ..RelayLimits::unknown()
    };

    let error = planner
        .plan(&relay(), &limits, &ungroupable(3))
        .expect_err("the relay declares it accepts one subscription");

    assert_eq!(
        error,
        SubscriptionPlanError::TooManySubscriptions {
            required: 3,
            maximum: 1,
        },
        "the refusal names the exact required and permitted counts"
    );
}

#[test]
fn a_stricter_relay_message_claim_refuses_the_frame_before_handoff() {
    let planner = StandardSubscriptionPlanner::default();
    let limits = RelayLimits {
        max_message_length: Some(64),
        ..RelayLimits::unknown()
    };

    let error = planner
        .plan(&relay(), &limits, &ungroupable(1))
        .expect_err("one REQ already exceeds a 64-byte frame claim");

    let SubscriptionPlanError::FrameTooLarge { bytes, maximum } = error else {
        panic!("expected an exact frame-size shortfall, got {error:?}");
    };
    assert_eq!(maximum, 64);
    assert!(bytes > 64);
}

#[test]
fn a_declared_subscription_id_length_refuses_the_identifier_it_cannot_carry() {
    let planner = StandardSubscriptionPlanner::default();
    let limits = RelayLimits {
        max_subscription_id_length: Some(4),
        ..RelayLimits::unknown()
    };

    let error = planner
        .plan(&relay(), &limits, &ungroupable(1))
        .expect_err("the subscription id is longer than the relay accepts");

    assert!(
        matches!(error, SubscriptionPlanError::SubscriptionIdTooLong { maximum, .. } if maximum == 4),
        "expected an exact identifier-length shortfall, got {error:?}"
    );
}

#[test]
fn a_declared_filter_limit_refuses_a_larger_requested_bound() {
    let planner = StandardSubscriptionPlanner::default();
    let query = fava_query::Query::events()
        .authors([Keys::generate().public_key()])
        .limit(500)
        .expect("limit is non-zero");
    let demand = vec![demand_for_query(SubscriptionId::new("fava-1"), &query)];
    let limits = RelayLimits {
        max_filter_limit: Some(50),
        ..RelayLimits::unknown()
    };

    assert_eq!(
        planner.plan(&relay(), &limits, &demand),
        Err(SubscriptionPlanError::FilterLimitTooLarge {
            requested: 500,
            maximum: 50,
        })
    );
    planner
        .plan(&relay(), &RelayLimits::unknown(), &demand)
        .expect("an undeclared relay limit refuses nothing");
}
