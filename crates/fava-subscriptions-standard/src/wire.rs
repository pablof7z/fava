//! Wire identity and exact REQ sizing.
//!
//! Identity is minted, never derived from content.
//!
//! A content digest recycles by construction: close a subscription, demand the
//! same filter again, and the same id comes back — so a late EOSE or EVENT for
//! the closed request settles the new one. `GOALS:426` (QUERY-010) forbids that
//! by name: *"Reopening dropped demand MUST use fresh request identity so a
//! late EOSE or event from the old request cannot settle the new one."*
//!
//! Two further properties fall out of minting rather than digesting. The id
//! carries no network-controlled bytes, so a hostile relay steering a derived
//! author or tag set cannot aim a collision at an established subscription. And
//! nothing the relay advertises feeds the id, so a NIP-11 refetch can never
//! move an id that is already live.

use fava_subscriptions::SubscriptionPlanError;
use fava_transport::{BoundedText, SUBSCRIPTION_ID_BYTES};
use fava_wire::{ClientMessage, SubscriptionId, encode_client};
use nostr::filter::Filter;

/// Exact encoded byte length of the REQ this content produces.
///
/// # Errors
///
/// [`SubscriptionPlanError::Encoding`] when the exact NIP-01 value cannot be
/// serialized at all.
pub(crate) fn encoded_bytes(filters: &[Filter]) -> Result<usize, SubscriptionPlanError> {
    // The session has not named this subscription yet, and does not need to
    // have: every identifier it mints is exactly `SUBSCRIPTION_ID_BYTES` wide,
    // so a placeholder of that width encodes to the same length as the real
    // frame will.
    let placeholder = SubscriptionId::new("x".repeat(SUBSCRIPTION_ID_BYTES));
    let message = ClientMessage::Req {
        subscription_id: std::borrow::Cow::Owned(placeholder),
        filters: filters
            .iter()
            .map(|filter| std::borrow::Cow::Owned(filter.clone()))
            .collect(),
    };
    encode_client(&message)
        .map(|frame| frame.len())
        .map_err(|error| SubscriptionPlanError::Encoding(BoundedText::new(error.to_string())))
}
