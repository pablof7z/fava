# fava-relay

Neutral logical relay identity. This crate owns only public/authenticated access
and the relay URL plus access key. Transport generations remain transport-owned.

```rust
use fava_relay::{RelayAccess, RelaySessionKey};
use nostr::key::Keys;
use nostr::types::RelayUrl;

let relay = RelayUrl::parse("wss://relay.example")?;
let public = RelaySessionKey {
    relay: relay.clone(),
    access: RelayAccess::Public,
};
let authenticated = RelaySessionKey {
    relay,
    access: RelayAccess::Authenticated(Keys::generate().public_key()),
};
assert_ne!(public, authenticated);
# Ok::<(), Box<dyn std::error::Error>>(())
```
