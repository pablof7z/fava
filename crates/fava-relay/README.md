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

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_relay` |  |
| Enum | `fava_relay::RelayAccess` |  |
| Enum variant | `fava_relay::RelayAccess::Authenticated` |  |
| Public field | `fava_relay::RelayAccess::Authenticated::0` |  |
| Enum variant | `fava_relay::RelayAccess::Public` |  |
| Struct | `fava_relay::RelaySessionKey` |  |
| Public field | `fava_relay::RelaySessionKey::access` |  |
| Public field | `fava_relay::RelaySessionKey::relay` |  |
<!-- END crate-readme-api inventory -->
