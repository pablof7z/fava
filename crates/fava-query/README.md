# fava-query

Declarative event queries and neutral current-source snapshots. A query keeps
acquisition, result authority, and exact relay access as independent identity.
Evaluation qualifies each atomic relay contribution by access and, for
`OnlyRelays`, URL before same-id aggregation and one coordinate winner.

```rust
use fava_query::Query;
use fava_relay::RelayAccess;
use nostr::key::Keys;

let public = Query::events().with_relay_access(RelayAccess::Public);
let authenticated = Query::events()
    .with_relay_access(RelayAccess::Authenticated(Keys::generate().public_key()));
assert_ne!(public, authenticated);
```
