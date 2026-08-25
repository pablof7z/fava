//! Exact neutral relay identity behavior.

use fava_relay::{RelayAccess, RelaySessionKey};
use nostr::key::Keys;
use nostr::types::RelayUrl;
use std::collections::BTreeSet;

#[test]
fn relay_identity_1_is_access_exact() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Keys::generate().public_key();
    let relay = RelayUrl::parse("wss://relay.example")?;
    let public = RelayAccess::Public;
    let authenticated = RelayAccess::Authenticated(alice);
    assert_ne!(public, authenticated);
    let public_key = RelaySessionKey {
        relay: relay.clone(),
        access: public,
    };
    let alice_key = RelaySessionKey {
        relay: relay.clone(),
        access: authenticated,
    };
    assert_eq!(public_key.relay, relay);
    assert_eq!(public_key.access, RelayAccess::Public);
    assert_eq!(alice_key.access, RelayAccess::Authenticated(alice));
    assert_ne!(public_key, alice_key);
    assert_eq!(BTreeSet::from([public_key, alice_key]).len(), 2);
    Ok(())
}
