//! Subscription attribution, verification, and cache admission.

use fava_event_cache::{EventCache, EventCacheError};
use fava_state::{CachedEvent, RelayEvidence, RelaySessionKey, Timestamp};
use nostr::event::Event;
use nostr::filter::{Filter, MatchEventOptions};
use nostr::message::SubscriptionId;
use thiserror::Error;

/// Refusal of one relay EVENT before it can affect local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RelayIngestError {
    /// The relay frame belongs to no accepted current subscription.
    #[error("relay EVENT belongs to the wrong subscription")]
    WrongSubscription,
    /// The event does not satisfy the exact accepted NIP-01 filter.
    #[error("relay EVENT does not match its accepted filter")]
    OffFilter,
    /// The event ID or signature is invalid.
    #[error("relay EVENT verification failed: {0}")]
    InvalidEvent(String),
    /// The event-cache provider refused or failed the atomic admission.
    #[error(transparent)]
    Cache(#[from] EventCacheError),
}

/// Attribute, verify, filter, and admit one relay EVENT.
///
/// # Errors
///
/// Returns [`RelayIngestError`] unless the frame belongs to the exact accepted
/// subscription, has a valid ID and signature, matches its filter, and can be
/// admitted by the selected event cache.
pub fn admit_subscription_event(
    cache: &dyn EventCache,
    session: &RelaySessionKey,
    expected_subscription: &SubscriptionId,
    actual_subscription: &SubscriptionId,
    filter: &Filter,
    event: Event,
    now: Timestamp,
) -> Result<bool, RelayIngestError> {
    if actual_subscription != expected_subscription {
        return Err(RelayIngestError::WrongSubscription);
    }
    event
        .verify()
        .map_err(|error| RelayIngestError::InvalidEvent(error.to_string()))?;
    if !filter.match_event(&event, MatchEventOptions::new()) {
        return Err(RelayIngestError::OffFilter);
    }
    cache
        .admit(
            CachedEvent::new(event, RelayEvidence::one(session.clone(), now)),
            now,
        )
        .map_err(Into::into)
}
