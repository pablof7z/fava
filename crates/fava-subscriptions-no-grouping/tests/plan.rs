//! No-grouping planner contract evidence.

use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{RelayDemand, SubscriptionPlanner};
use fava_subscriptions_no_grouping::planner;
use nostr::filter::Filter;
use nostr::message::{ClientMessage, SubscriptionId};

#[test]
fn each_logical_demand_becomes_one_exact_req_with_attribution() {
    let relay = RelaySessionKey::new(
        RelayUrl::parse("wss://relay.example").expect("relay URL"),
        RelayAccess::public(),
    );
    let first_id = SubscriptionId::new("first");
    let second_id = SubscriptionId::new("second");
    let first = Filter::new().limit(1);
    let second = Filter::new().limit(2);
    let demand = [
        RelayDemand::new(first_id.clone(), first.clone()),
        RelayDemand::new(second_id.clone(), second.clone()),
    ];

    let plan = planner().plan(&relay, &demand).expect("plan is exact");

    assert_eq!(plan.relay, relay);
    assert_eq!(plan.messages.len(), 2);
    assert!(matches!(
        &plan.messages[0],
        ClientMessage::Req { subscription_id, filters }
            if subscription_id.as_ref() == &first_id && filters[0].as_ref() == &first
    ));
    assert_eq!(plan.filter(&second_id), Some(&second));
}
