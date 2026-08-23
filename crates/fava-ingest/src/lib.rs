//! Subscription attribution, verification, and cache admission.

use std::collections::BTreeMap;

use fava_event_cache::{EventCache, EventCacheError};
use fava_state::{CachedEvent, RelayEvidence, RelaySessionKey, Timestamp};
use nostr::event::Event;
use nostr::filter::{Filter, MatchEventOptions};
use nostr::message::SubscriptionId;
use thiserror::Error;

/// Refusal of one relay EVENT before it can affect local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RelayIngestError {
    /// The relay attributed the EVENT to a subscription this session never accepted.
    #[error("relay EVENT belongs to the wrong subscription")]
    WrongSubscription,
    /// The subscription was accepted with no filter, so it authorizes nothing.
    #[error("relay EVENT belongs to a subscription that authorizes no filter")]
    UnauthorizedSubscription,
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
/// `accepted` is the exact set of subscription IDs this relay session accepted
/// and the complete filter set each one authorizes. `attributed` is the
/// subscription ID the relay itself put on the EVENT frame. Attribution is
/// resolved here, so a relay can never select which accepted filter validates
/// its event and no caller can pair an ID with a filter the session did not
/// accept for it.
///
/// A NIP-01 REQ carries one or more filters and the relay is required to serve
/// their union, so an authorized subscription is satisfied by *any* of its
/// accepted filters. Retaining only one of them would silently discard every
/// event a multi-filter REQ asked for under its later filters, which is loss
/// with no evidence. An accepted subscription with no filters authorizes
/// nothing and is refused rather than admitting everything.
///
/// # Errors
///
/// Returns [`RelayIngestError`] unless the frame is attributed to an accepted
/// subscription with at least one filter, has a valid ID and signature,
/// matches one of that subscription's accepted filters, and can be admitted by
/// the selected event cache.
pub fn admit_subscription_event(
    cache: &dyn EventCache,
    session: &RelaySessionKey,
    accepted: &BTreeMap<SubscriptionId, Vec<Filter>>,
    attributed: &SubscriptionId,
    event: Event,
    now: Timestamp,
) -> Result<bool, RelayIngestError> {
    let Some(filters) = accepted.get(attributed) else {
        return Err(RelayIngestError::WrongSubscription);
    };
    if filters.is_empty() {
        return Err(RelayIngestError::UnauthorizedSubscription);
    }
    event
        .verify()
        .map_err(|error| RelayIngestError::InvalidEvent(error.to_string()))?;
    if !filters
        .iter()
        .any(|filter| filter.match_event(&event, MatchEventOptions::new()))
    {
        return Err(RelayIngestError::OffFilter);
    }
    cache
        .admit(
            CachedEvent::new(event, RelayEvidence::one(session.clone(), now)),
            now,
        )
        .map_err(Into::into)
}
