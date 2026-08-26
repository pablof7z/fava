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
### `fava_relay` (Module)

Compiler-visible module `fava_relay`.
<!-- api-item {"kind":"Module","item":"fava_relay","signature":"pub mod fava_relay","evidence":"cargo-public-api@0.52.0: pub mod fava_relay"} -->

### `RelayAccess` (Enum)

Compiler-visible enum `fava_relay::RelayAccess`.
<!-- api-item {"kind":"Enum","item":"fava_relay::RelayAccess","signature":"pub enum fava_relay::RelayAccess","evidence":"cargo-public-api@0.52.0: pub enum fava_relay::RelayAccess"} -->

| Item | Purpose |
| --- | --- |
| **`Authenticated`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_relay::RelayAccess::Authenticated","signature":"pub fava_relay::RelayAccess::Authenticated(nostr::key::public_key::PublicKey)","evidence":"cargo-public-api@0.52.0: pub fava_relay::RelayAccess::Authenticated(nostr::key::public_key::PublicKey)"} --> | Compiler-visible enum variant owned by `fava_relay::RelayAccess`. |
| **`Field `0` of `Authenticated``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_relay::RelayAccess::Authenticated::0","signature":"nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: nostr::key::public_key::PublicKey"} --> | Compiler-visible public field owned by `fava_relay::RelayAccess`. |
| **`Public`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_relay::RelayAccess::Public","signature":"pub fava_relay::RelayAccess::Public","evidence":"cargo-public-api@0.52.0: pub fava_relay::RelayAccess::Public"} --> | Compiler-visible enum variant owned by `fava_relay::RelayAccess`. |

### `RelaySessionKey` (Struct)

Compiler-visible struct `fava_relay::RelaySessionKey`.
<!-- api-item {"kind":"Struct","item":"fava_relay::RelaySessionKey","signature":"pub struct fava_relay::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub struct fava_relay::RelaySessionKey"} -->

| Item | Purpose |
| --- | --- |
| **`access`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_relay::RelaySessionKey::access","signature":"pub fava_relay::RelaySessionKey::access: fava_relay::RelayAccess","evidence":"cargo-public-api@0.52.0: pub fava_relay::RelaySessionKey::access: fava_relay::RelayAccess"} --> | Compiler-visible public field owned by `fava_relay::RelaySessionKey`. |
| **`relay`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_relay::RelaySessionKey::relay","signature":"pub fava_relay::RelaySessionKey::relay: nostr::types::url::RelayUrl","evidence":"cargo-public-api@0.52.0: pub fava_relay::RelaySessionKey::relay: nostr::types::url::RelayUrl"} --> | Compiler-visible public field owned by `fava_relay::RelaySessionKey`. |
<!-- END crate-readme-api inventory -->
