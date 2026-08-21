//! Standard planner grouping and exact relay-limit evidence.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{RelayDemand, SubscriptionPlanError, SubscriptionPlanner};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use nostr::filter::{Filter, SingleLetterTag};
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
        .plan(&relay, &demand)
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
fn identical_filters_deduplicate_with_exact_logical_attribution() {
    let filter = Filter::new().search("identical");
    let demand = [0, 1]
        .into_iter()
        .map(|index| {
            RelayDemand::new(
                SubscriptionId::new(format!("identical-{index}")),
                filter.clone(),
            )
        })
        .collect::<Vec<_>>();

    let plan = StandardSubscriptionPlanner::default()
        .plan(&relay(), &demand)
        .expect("identical demand plans");

    assert_eq!(plan.messages.len(), 1);
    assert_eq!(plan.attribution.len(), 1);
    assert_eq!(
        plan.demand.values().next().expect("wire attribution"),
        &demand
            .iter()
            .map(|item| item.subscription_id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn one_exact_non_empty_tag_axis_groups_with_exact_values_and_logical_ids() {
    let key = tag('e');
    let demand = (0..300)
        .map(|index| {
            RelayDemand::new(
                SubscriptionId::new(format!("tag-logical-{index:03}")),
                Filter::new().custom_tags(key, [format!("value-{index:03}")]),
            )
        })
        .collect::<Vec<_>>();

    let plan = StandardSubscriptionPlanner::default()
        .plan(&relay(), &demand)
        .expect("compatible tag demand plans");

    assert_eq!(plan.messages.len(), 1);
    assert_eq!(plan.attribution.len(), 1);
    assert_eq!(
        plan.demand.values().next().expect("wire attribution"),
        &demand
            .iter()
            .map(|item| item.subscription_id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        plan.attribution
            .values()
            .next()
            .expect("wire filter")
            .generic_tags
            .get(&key),
        Some(
            &(0..300)
                .map(|index| format!("value-{index:03}"))
                .collect::<BTreeSet<_>>()
        )
    );
}

#[test]
fn unsafe_multi_axis_limit_case_and_empty_axis_candidates_remain_separate() {
    let lower_e = tag('e');
    let upper_e = tag('E');
    let lower_p = tag('p');
    let authors = [Keys::generate().public_key(), Keys::generate().public_key()];
    let cases = [
        (
            "limit",
            Filter::new().custom_tags(lower_e, ["one"]).limit(1),
            Filter::new().custom_tags(lower_e, ["two"]).limit(1),
        ),
        (
            "author-plus-tag",
            Filter::new()
                .author(authors[0])
                .custom_tags(lower_e, ["one"]),
            Filter::new()
                .author(authors[1])
                .custom_tags(lower_e, ["two"]),
        ),
        (
            "two-tag-axes",
            Filter::new()
                .custom_tags(lower_e, ["one"])
                .custom_tags(lower_p, ["left"]),
            Filter::new()
                .custom_tags(lower_e, ["two"])
                .custom_tags(lower_p, ["right"]),
        ),
        (
            "opposite-case-keys",
            Filter::new().custom_tags(lower_e, ["one"]),
            Filter::new().custom_tags(upper_e, ["two"]),
        ),
        (
            "present-empty-axis",
            Filter::new().custom_tags(lower_e, std::iter::empty::<String>()),
            Filter::new().custom_tags(lower_e, ["one"]),
        ),
        (
            "other-unequal-field",
            Filter::new().search("left"),
            Filter::new().search("right"),
        ),
    ];

    for (name, left, right) in cases {
        let demand = [
            RelayDemand::new(SubscriptionId::new(format!("{name}-left")), left),
            RelayDemand::new(SubscriptionId::new(format!("{name}-right")), right),
        ];
        let plan = StandardSubscriptionPlanner::default()
            .plan(&relay(), &demand)
            .unwrap_or_else(|error| panic!("{name} plans separately: {error}"));

        assert_eq!(plan.messages.len(), 2, "{name}");
        assert_eq!(plan.attribution.len(), 2, "{name}");
        assert!(
            plan.demand.values().all(|logical| logical.len() == 1),
            "{name}"
        );
    }
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
        planner.plan(&relay(), &demand),
        Err(SubscriptionPlanError::TooManySubscriptions {
            required: 2,
            maximum: 1,
        })
    );
}

#[test]
fn empty_duplicate_and_frame_bounds_return_exact_refusals() {
    let planner = StandardSubscriptionPlanner::default();
    assert_eq!(
        planner.plan(&relay(), &[]),
        Err(SubscriptionPlanError::EmptyDemand)
    );

    let duplicate = SubscriptionId::new("duplicate");
    let duplicate_demand = [
        RelayDemand::new(duplicate.clone(), Filter::new()),
        RelayDemand::new(duplicate.clone(), Filter::new()),
    ];
    assert_eq!(
        planner.plan(&relay(), &duplicate_demand),
        Err(SubscriptionPlanError::DuplicateSubscription(duplicate))
    );

    let frame_id = SubscriptionId::new("frame");
    let frame_demand = [RelayDemand::new(frame_id, Filter::new())];
    let frame_planner = StandardSubscriptionPlanner::bounded(
        NonZeroUsize::new(1).expect("non-zero"),
        NonZeroUsize::new(1).expect("non-zero"),
    );
    assert_eq!(
        frame_planner.plan(&relay(), &frame_demand),
        Err(SubscriptionPlanError::FrameTooLarge {
            bytes: r#"["REQ","frame",{}]"#.len(),
            maximum: 1,
        })
    );
}

fn tag(key: char) -> SingleLetterTag {
    SingleLetterTag::from_char(key).expect("ASCII letter tag key")
}

fn relay() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("wss://relay.example").expect("relay URL"),
        RelayAccess::public(),
    )
}
