//! Neutral immutable logical relay/access identity.

use nostr::key::PublicKey;
use nostr::types::RelayUrl;

/// Exact application-selected access authority for relay work.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RelayAccess {
    /// Public unauthenticated relay authority.
    Public,
    /// Relay authority authenticated as one exact protocol public key.
    Authenticated(PublicKey),
}

/// Stable logical relay and access identity shared across owners.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionKey {
    /// Exact normalized relay URL.
    pub relay: RelayUrl,
    /// Exact access authority.
    pub access: RelayAccess,
}
