## Why

`RouteRequest::access()` returns `RelayAccess::Public` for every write, unconditionally (`crates/fava-routing/src/lib.rs:84`). No automatically routed write can reach a relay that demands authentication, whatever account the application named. Authenticated reads work; authenticated writes are unreachable through routing.

A write also outlives the moment it was accepted. It is taken into durable custody, parked while it waits for a signer or a route, and resumed later — possibly after a process restart. `WriteIntent` and `Receipt` record what the write is and where it goes, but not whose authority it was accepted under. So a write accepted under Alice's authenticated session resumes, after restart, as public work, and either fails or silently goes somewhere it was never meant to.

## What Changes

- **BREAKING** Carry the accepted `RelayAccess` on `WriteIntent` and `Receipt`, so a write resumes under the authority it was accepted under rather than under whatever the process defaults to.
- **BREAKING** Reshape `RouteRequest::Write` to carry that authority, and make `RouteRequest::access()` return it rather than `RelayAccess::Public`.
- **BREAKING** Bump the redb write-store schema from 4 to 5. `validate_schema` and `redb_schema_mismatch_refuses_without_fallback` stay as they are: a store written by an earlier build refuses to open with a named error rather than partially deserializing a row whose shape changed.
- Record the authority as one value beside the existing author, not as a second public key that could disagree with it. Whose event it is and whose connection it goes over are separate facts; duplicating either into the other is how they come to contradict.
- Refuse reconstruction of a stored row whose access authority is absent, malformed, or contradicts its routed destinations, rather than defaulting it to public.
- Rename the four `schema_v4_*` write-store tests. They mutate rows in the current schema and assert reconstruction refuses them; the prefix reads as though they test an earlier version.

## Capabilities

### New Capabilities

- `publication/write-access-authority`: which relay authority a write is accepted under, how that survives durable custody and a restart, and what happens when a stored row's authority is missing or contradicts where the write was routed.

### Modified Capabilities

None. No settled capability describes a write's relay authority, because writes have never had one.

## Impact

Changed public API: `WriteIntent` and `Receipt` carry a `RelayAccess`; `RouteRequest::Write` is reshaped; `RouteRequest::access()` returns the carried authority. Every construction and destructuring site in `fava-publication`, the router crates, and their tests moves.

Changed persisted format: redb write-store schema 5. A store written by an earlier build refuses to open. This is the only change in this line of work that touches persistence, which is why it is its own change rather than riding along with relay authentication or with naming the account.

Depends on `name-the-account-work-runs-as` for the verb that names the account. Without it there is a value to carry and no way for an application to set it.

Completes what `own-relay-authentication` opened: with this, an authenticated write routes and resumes under its own authority, and NIP-42 covers reads and writes alike.
