//! Deterministic wire identity and exact REQ sizing.

use std::collections::BTreeSet;

use fava_subscriptions::{RelayReadConstraints, SubscriptionPlanError};
use fava_transport::BoundedReason;
use fava_wire::{ClientMessage, SubscriptionId, encode_client};
use nostr::filter::Filter;

/// Hex digits of the content digest a full-length wire id carries.
const DIGEST_CHARS: usize = 16;

/// Namespace prefix a full-length wire id carries.
const PREFIX: &str = "fava-";

/// Characters a full-length wire id occupies.
const FULL_CHARS: usize = PREFIX.len() + DIGEST_CHARS;

/// Derive one wire id from the exact content it will carry.
///
/// The id is a pure function of the filters and the salt, so an unchanged
/// candidate wears an unchanged id across replans and can be retained rather
/// than reopened. `salt` exists only to step past a digest collision.
pub(crate) fn identity(
    filters: &[Filter],
    constraints: &RelayReadConstraints,
    salt: u64,
) -> Option<SubscriptionId> {
    let digest = format!("{:016x}", digest_of(filters, salt));
    let Some(maximum) = constraints.max_subscription_id_chars.get() else {
        return Some(SubscriptionId::new(format!("{PREFIX}{digest}")));
    };
    let maximum = maximum.get();
    if maximum >= FULL_CHARS {
        return Some(SubscriptionId::new(format!("{PREFIX}{digest}")));
    }
    digest
        .get(..maximum.min(DIGEST_CHARS))
        .map(SubscriptionId::new)
}

/// Allocate the first collision-free wire id for this content.
///
/// A digest collision against a *different* installed or planned subscription
/// is stepped past deterministically; exhausting the declared id space is a
/// caller-visible `None`.
pub(crate) fn allocate(
    filters: &[Filter],
    constraints: &RelayReadConstraints,
    taken: &BTreeSet<SubscriptionId>,
) -> Option<SubscriptionId> {
    for salt in 0..u64::from(u16::MAX) {
        let candidate = identity(filters, constraints, salt)?;
        if !taken.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Exact encoded byte length of the REQ this content produces.
///
/// # Errors
///
/// [`SubscriptionPlanError::Encoding`] when the exact NIP-01 value cannot be
/// serialized at all.
pub(crate) fn encoded_bytes(
    id: &SubscriptionId,
    filters: &[Filter],
) -> Result<usize, SubscriptionPlanError> {
    let message = ClientMessage::Req {
        subscription_id: std::borrow::Cow::Owned(id.clone()),
        filters: filters
            .iter()
            .map(|filter| std::borrow::Cow::Owned(filter.clone()))
            .collect(),
    };
    encode_client(&message)
        .map(|frame| frame.len())
        .map_err(|error| SubscriptionPlanError::Encoding(BoundedReason::new(error.to_string())))
}

/// FNV-1a over the canonical debug encoding of the filters and the salt.
///
/// A repository-owned hash keeps wire identity reproducible across processes
/// and Rust releases, which `RandomState` would not.
fn digest_of(filters: &[Filter], salt: u64) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET;
    for byte in salt.to_be_bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
    }
    for filter in filters {
        for byte in serde_json::to_string(filter)
            .unwrap_or_else(|_| format!("{filter:?}"))
            .as_bytes()
        {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(PRIME);
        }
        hash = (hash ^ 0xff).wrapping_mul(PRIME);
    }
    hash
}
