//! Exact mapping from logical relay demand to Nostr subscriptions.

use std::collections::BTreeMap;

use fava_query::Query;
use fava_state::RelaySessionKey;
use fava_wire::{ClientMessage, SubscriptionId};
use nostr::filter::Filter;
use thiserror::Error;

/// Limits one relay declares for the requests it will accept.
///
/// Every field is optional and means exactly "the relay did not tell us".
/// A missing, stale, malformed, or unsupported claim stays unknown; it never
/// becomes an invented default. Planners combine what a relay declares with
/// their own configured bounds and honor whichever is stricter.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RelayLimits {
    /// Largest number of concurrent wire subscriptions per session.
    pub max_subscriptions: Option<usize>,
    /// Largest number of filters in one REQ.
    pub max_filters: Option<usize>,
    /// Largest accepted frame in bytes.
    pub max_message_length: Option<usize>,
    /// Largest accepted subscription id in bytes.
    pub max_subscription_id_length: Option<usize>,
    /// Largest accepted per-filter `limit` value.
    pub max_filter_limit: Option<usize>,
}

impl RelayLimits {
    /// No relay claim is currently known for this relay.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            max_subscriptions: None,
            max_filters: None,
            max_message_length: None,
            max_subscription_id_length: None,
            max_filter_limit: None,
        }
    }

    /// Strictest of one configured bound and any relay claim.
    #[must_use]
    pub fn strictest(configured: usize, declared: Option<usize>) -> usize {
        declared.map_or(configured, |declared| configured.min(declared))
    }
}

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
    /// Logical subscription IDs represented by each exact wire subscription.
    pub demand: BTreeMap<SubscriptionId, Vec<SubscriptionId>>,
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
    /// Produce a complete exact plan for one relay session within its limits.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionPlanError`] when demand is empty, ambiguous, or
    /// cannot be represented exactly inside the declared and configured bounds.
    fn plan(
        &self,
        relay: &RelaySessionKey,
        limits: &RelayLimits,
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
    /// Exact logical demand requires more subscriptions than the relay permits.
    #[error("relay allows {maximum} subscriptions but exact demand requires {required}")]
    TooManySubscriptions {
        /// Exact wire subscription count required.
        required: usize,
        /// Declared maximum wire subscription count.
        maximum: usize,
    },
    /// One exact REQ frame exceeds the relay's declared message-size limit.
    #[error("REQ frame uses {bytes} bytes but relay allows {maximum}")]
    FrameTooLarge {
        /// Exact encoded frame size.
        bytes: usize,
        /// Declared maximum frame size.
        maximum: usize,
    },
    /// One exact REQ carries more filters than the relay permits.
    #[error("REQ carries {filters} filters but relay allows {maximum}")]
    TooManyFilters {
        /// Exact filter count in one REQ.
        filters: usize,
        /// Declared maximum filter count.
        maximum: usize,
    },
    /// One subscription id exceeds the relay's declared identifier length.
    #[error("subscription id {id} uses {bytes} bytes but relay allows {maximum}")]
    SubscriptionIdTooLong {
        /// Exact subscription id that cannot be represented.
        id: SubscriptionId,
        /// Exact identifier size.
        bytes: usize,
        /// Declared maximum identifier size.
        maximum: usize,
    },
    /// A filter result bound exceeds the relay's declared maximum.
    #[error("filter limit {requested} exceeds relay maximum {maximum}")]
    FilterLimitTooLarge {
        /// Exact requested per-filter limit.
        requested: usize,
        /// Declared maximum per-filter limit.
        maximum: usize,
    },
    /// Exact Nostr REQ encoding failed before handoff.
    #[error("REQ encoding failed: {0}")]
    Encoding(String),
}

/// Refuse a candidate wire plan the relay has told us it cannot accept.
///
/// Every check uses the stricter of Fava's configured bound and the relay's
/// declared claim. An unknown claim leaves Fava's own bound in force. Nothing
/// is truncated, clamped, or renamed: an exceeded bound is an exact refusal
/// naming the actual and maximum values, produced before any handoff.
///
/// # Errors
///
/// Returns the exact [`SubscriptionPlanError`] for the first exceeded bound.
pub fn enforce_limits(
    limits: &RelayLimits,
    max_subscriptions: usize,
    max_frame_bytes: usize,
    messages: &[ClientMessage<'static>],
) -> Result<(), SubscriptionPlanError> {
    let subscription_bound = RelayLimits::strictest(max_subscriptions, limits.max_subscriptions);
    if messages.len() > subscription_bound {
        return Err(SubscriptionPlanError::TooManySubscriptions {
            required: messages.len(),
            maximum: subscription_bound,
        });
    }
    let frame_bound = RelayLimits::strictest(max_frame_bytes, limits.max_message_length);
    for message in messages {
        let ClientMessage::Req {
            subscription_id,
            filters,
        } = message
        else {
            continue;
        };
        if let Some(maximum) = limits.max_subscription_id_length
            && subscription_id.as_str().len() > maximum
        {
            return Err(SubscriptionPlanError::SubscriptionIdTooLong {
                id: subscription_id.clone().into_owned(),
                bytes: subscription_id.as_str().len(),
                maximum,
            });
        }
        if let Some(maximum) = limits.max_filters
            && filters.len() > maximum
        {
            return Err(SubscriptionPlanError::TooManyFilters {
                filters: filters.len(),
                maximum,
            });
        }
        if let Some(maximum) = limits.max_filter_limit {
            for filter in filters {
                if let Some(requested) = filter.limit
                    && requested > maximum
                {
                    return Err(SubscriptionPlanError::FilterLimitTooLarge { requested, maximum });
                }
            }
        }
        let bytes = fava_wire::encode_client(message)
            .map_err(|error| SubscriptionPlanError::Encoding(error.to_string()))?
            .len();
        if bytes > frame_bound {
            return Err(SubscriptionPlanError::FrameTooLarge {
                bytes,
                maximum: frame_bound,
            });
        }
    }
    Ok(())
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
