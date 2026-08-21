//! One exact Nostr subscription per logical relay demand.

use std::collections::BTreeMap;

use fava_state::RelaySessionKey;
use fava_subscriptions::{
    RelayDemand, RelayLimits, SubscriptionPlan, SubscriptionPlanError, SubscriptionPlanner,
    enforce_limits,
};
use fava_wire::ClientMessage;

/// Fava's own bounds for this policy, independent of any relay claim.
const MAX_SUBSCRIPTIONS: usize = 64;
const MAX_FRAME_BYTES: usize = 1_048_576;

struct OnePerDemand;

impl SubscriptionPlanner for OnePerDemand {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        limits: &RelayLimits,
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        if demand.is_empty() {
            return Err(SubscriptionPlanError::EmptyDemand);
        }
        let mut attribution = BTreeMap::new();
        let mut logical = BTreeMap::new();
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
            logical.insert(
                item.subscription_id.clone(),
                vec![item.subscription_id.clone()],
            );
        }
        enforce_limits(limits, MAX_SUBSCRIPTIONS, MAX_FRAME_BYTES, &messages)?;
        Ok(SubscriptionPlan {
            relay: relay.clone(),
            messages,
            attribution,
            demand: logical,
        })
    }
}

/// Construct the policy that preserves one wire subscription per demand.
#[must_use]
pub const fn planner() -> impl SubscriptionPlanner {
    OnePerDemand
}
