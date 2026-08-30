# fava-state

Pure universal Nostr event-state values and rules. The crate owns no cache,
provider, observation, serialization, or lifecycle. Callers supply finite
current atomic relay contributions and commit each returned mutation batch
atomically.

An admitted `RelayEvent` contains one signed event and one exact
`RelaySessionKey` occurrence. `relay_occurrences_for_event` creates the private,
event-id-bound aggregate used by query records. Replacement is per exact
session; deletion and expiration may retract several exact contributions.

Winner order is universal: later `created_at` wins, then the lower event id.
Malformed optional or sibling tags remain scoped to themselves.
