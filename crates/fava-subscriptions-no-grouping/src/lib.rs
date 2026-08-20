//! One exact Nostr subscription per logical relay demand.

use std::collections::BTreeMap;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    RelayDemand, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
};
use fava_wire::ClientMessage;

struct OnePerDemand;

impl SubscriptionPlanner for OnePerDemand {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        if demand.is_empty() {
            return Err(SubscriptionPlanError::EmptyDemand);
        }
        let mut attribution = BTreeMap::new();
        let mut messages = Vec::with_capacity(demand.len());
        for item in demand {
            if attribution
                .insert(item.subscription_id.clone(), item.filter.clone())
                .is_some()
            {
                return Err(SubscriptionPlanError::DuplicateSubscription(
                    item.subscription_id.clone(),
                ));
            }
            messages.push(ClientMessage::req(
                item.subscription_id.clone(),
                item.filter.clone(),
            ));
        }
        Ok(SubscriptionPlan {
            relay: relay.clone(),
            messages,
            attribution,
        })
    }
}

/// Construct the policy that preserves one wire subscription per demand.
#[must_use]
pub const fn planner() -> impl SubscriptionPlanner {
    OnePerDemand
}
