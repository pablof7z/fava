# fava-query

Declarative event queries and neutral current-source snapshots. A query keeps
acquisition, result authority, and exact relay access as independent identity.
Evaluation qualifies each atomic relay contribution by access and, for
`OnlyRelays`, URL before same-id aggregation and one coordinate winner.

```rust
use fava_query::Query;
use fava_relay::Authority;
use nostr::key::Keys;

let public = Query::events().with_relay_access(Authority::Unauthenticated);
let authenticated = Query::events()
    .with_relay_access(Authority::As(Keys::generate().public_key()));
assert_ne!(public, authenticated);
```

`Query::authors_current_account()` and
`Query::tag_value_current_account(key)` retain one reactive session dependency.
Fava binds it automatically for every concrete observation generation. No
current account matches nothing and opens no relay demand; applications never
rebuild or reopen the query after a switch.
