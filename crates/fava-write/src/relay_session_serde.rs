use fava_relay::{RelayAccess, RelaySessionKey};
use nostr::key::PublicKey;
use nostr::types::RelayUrl;
pub(super) fn encode(value: &RelaySessionKey) -> (RelayUrl, Option<PublicKey>) {
    let public_key = match value.access {
        RelayAccess::Public => None,
        RelayAccess::Authenticated(public_key) => Some(public_key),
    };
    (value.relay.clone(), public_key)
}

pub(super) fn decode(value: (RelayUrl, Option<PublicKey>)) -> RelaySessionKey {
    RelaySessionKey {
        relay: value.0,
        access: value
            .1
            .map_or(RelayAccess::Public, RelayAccess::Authenticated),
    }
}
