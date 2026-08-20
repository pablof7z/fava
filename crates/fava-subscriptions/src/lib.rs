//! Exact mapping from logical relay demand to Nostr subscriptions.

use std::collections::BTreeMap;

use fava_query::Query;
use fava_state::RelaySessionKey;
use fava_wire::{ClientMessage, SubscriptionId};
use nostr::filter::Filter;
use thiserror::Error;

/// One logical filter assigned to an exact Nostr subscription ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayDemand {
    /// ID used to correlate wire EVENT, EOSE, and CLOSED messages.
    pub subscription_id: SubscriptionId,
    /// Exact NIP-01 filter requested from the relay.
    pub filter: Filter,
}

impl RelayDemand {
    /// Construct one exact logical relay demand.
    #[must_use]
    pub const fn new(subscription_id: SubscriptionId, filter: Filter) -> Self {
        Self {
            subscription_id,
            filter,
        }
    }
}

/// Complete wire subscriptions and inbound attribution for one relay session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPlan {
    /// Exact relay session receiving the messages.
    pub relay: RelaySessionKey,
    /// Exact NIP-01 messages to hand off.
    pub messages: Vec<ClientMessage<'static>>,
    /// Accepted subscription IDs and their exact filters.
    pub attribution: BTreeMap<SubscriptionId, Filter>,
}

impl SubscriptionPlan {
    /// Filter that authorizes inbound events for one subscription ID.
    #[must_use]
    pub fn filter(&self, id: &SubscriptionId) -> Option<&Filter> {
        self.attribution.get(id)
    }
}

/// Replaceable mapping from logical demand to exact Nostr subscriptions.
pub trait SubscriptionPlanner: Send + Sync {
    /// Produce a complete exact plan for one relay session.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionPlanError`] when demand is empty, ambiguous, or
    /// cannot be represented exactly.
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}

/// Exact subscription planning refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SubscriptionPlanError {
    /// No wire subscription corresponds to empty relay demand.
    #[error("subscription planning requires non-empty demand")]
    EmptyDemand,
    /// Subscription IDs must be unique within one relay session plan.
    #[error("duplicate subscription id: {0}")]
    DuplicateSubscription(SubscriptionId),
}

/// Convert one public Query into one exact NIP-01 relay demand.
#[must_use]
pub fn demand_for_query(subscription_id: SubscriptionId, query: &Query) -> RelayDemand {
    let mut filter = Filter::new();
    if let Some(ids) = &query.selection().ids {
        filter = filter.ids(ids.iter().copied());
    }
    if let Some(authors) = &query.selection().authors {
        filter = filter.authors(authors.iter().copied());
    }
    if let Some(kinds) = &query.selection().kinds {
        filter = filter.kinds(kinds.iter().copied());
    }
    if let Some(limit) = query.result_limit() {
        filter = filter.limit(limit.get());
    }
    RelayDemand::new(subscription_id, filter)
}
