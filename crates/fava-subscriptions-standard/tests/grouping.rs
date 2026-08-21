//! Standard planner grouping and exact relay-limit evidence.

use std::num::NonZeroUsize;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{RelayDemand, RelayLimits, SubscriptionPlanError, SubscriptionPlanner};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use nostr::filter::Filter;
use nostr::key::Keys;
use nostr::message::SubscriptionId;

#[test]
fn compatible_author_filters_group_with_exact_logical_attribution() {
    let authors = [Keys::generate().public_key(), Keys::generate().public_key()];
    let demand = authors
        .iter()
        .enumerate()
        .map(|(index, author)| {
            RelayDemand::new(
                SubscriptionId::new(format!("logical-{index}")),
                Filter::new().author(*author),
            )
        })
        .collect::<Vec<_>>();
    let relay = relay();
    let plan = StandardSubscriptionPlanner::default()
        .plan(&relay, &RelayLimits::unknown(), &demand)
        .expect("compatible demand plans");

    assert_eq!(plan.messages.len(), 1);
    assert_eq!(plan.attribution.len(), 1);
    assert_eq!(
        plan.demand.values().next().expect("wire attribution").len(),
        2
    );
    assert_eq!(
        plan.attribution
            .values()
            .next()
            .expect("wire filter")
            .authors
            .as_ref()
            .expect("authors")
            .len(),
        2
    );
}

#[test]
fn relay_subscription_bound_returns_exact_shortfall() {
    let demand = [0, 1]
        .into_iter()
        .map(|index| {
            RelayDemand::new(
                SubscriptionId::new(format!("logical-{index}")),
                Filter::new().search(format!("distinct-{index}")),
            )
        })
        .collect::<Vec<_>>();
    let planner = StandardSubscriptionPlanner::bounded(
        NonZeroUsize::new(1).expect("non-zero"),
        NonZeroUsize::new(1_048_576).expect("non-zero"),
    );

    assert_eq!(
        planner.plan(&relay(), &RelayLimits::unknown(), &demand),
        Err(SubscriptionPlanError::TooManySubscriptions {
            required: 2,
            maximum: 1,
        })
    );
}

fn relay() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("wss://relay.example").expect("relay URL"),
        RelayAccess::public(),
    )
}
