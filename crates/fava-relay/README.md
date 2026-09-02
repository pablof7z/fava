# fava-relay

Neutral logical relay identity, and what work requires of a connection.
Access is not identity: a relay is named by its URL alone, and work states
the `Authority` it needs of a connection — no authentication, or
authentication as one exact account. Connectivity and authentication are
facts a connection carries, not facts a relay is keyed by.

```rust
use fava_relay::Authority;
use nostr::key::Keys;

let unauthenticated = Authority::Unauthenticated;
let authenticated = Authority::As(Keys::generate().public_key());
assert_ne!(unauthenticated, authenticated);
# Ok::<(), Box<dyn std::error::Error>>(())
```
