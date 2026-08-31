## Context

See `proposal.md` — Why. The facts this rests on:

- `RouteRequest::access()` matches on the request and returns `RelayAccess::Public` for every `Write` (`crates/fava-routing/src/lib.rs:84`). The read arm returns the query's own access, so reads already work.
- `SCHEMA_VERSION` is 4 (`crates/fava-write-store-redb/src/schema.rs:17`), and `validate_schema` refuses a store stamped with a different value.
- Four `schema_v4_*` tests mutate rows in the current schema and assert reconstruction refuses them. The prefix names the version they run against, not a version they test compatibility with.
- `name-the-account-work-runs-as` supplies the verb an application uses to name the account. Without it the authority exists in the types with no way to set it.

## Goals / Non-Goals

**Goals:**

- An automatically routed write can reach a relay that demands authentication.
- A write resumes under the authority it was accepted under, across a restart.
- A stored row whose authority cannot be trusted refuses rather than quietly becoming public work.

**Non-Goals:**

- Changing who authors an event. The authority is recorded beside the author, never instead of it.
- Changing how authentication itself works. That owner is unchanged; this change gives writes a way to ask for an authenticated session.
- Backwards compatibility with schema 4 stores. They refuse to open, which is the existing contract for a version change.

## Decisions

### One authority beside the author, never a second author

The tempting shape is to store the account's public key next to the event's. Two public keys in one row, and nothing saying which is which, is how they come to disagree: a later reader picks the wrong one and the write goes out as the wrong account or to the wrong session.

So the row carries a `RelayAccess` — a type whose two shapes are "public" and "authenticated as this key" — and the author stays exactly where it is. The distinction is in the type rather than in a comment.

### Refuse a row whose authority contradicts its destinations

A stored write records both its authority and where it was routed. Those can disagree only if a row was tampered with or written by a build with a different idea of the shape. Choosing one and proceeding sends work somewhere it was never accepted for.

Refusing is consistent with how the store already treats a row it cannot reconstruct, and the four existing mutation-recovery tests are exactly the pattern to extend.

### The schema bump rides alone

This is the only change in this line of work that touches persistence. Bundled with relay authentication, as it originally was, it would have gated a store migration on a NIP-42 capability that has nothing to do with it — and, since that capability was itself gated on evidence that no longer existed, gated it indefinitely.

## Risks / Trade-offs

**Every `RouteRequest::Write` construction and destructuring site moves.** → Mechanical and compiler-found, across `fava-publication`, the router crates, and their tests.

**Existing stores refuse to open.** → The existing contract for a schema change, and `redb_schema_mismatch_refuses_without_fallback` already asserts it. Named here so it is a decision rather than a surprise.

**A write accepted under an account whose signer later goes away.** → Out of scope: the parked-write path already handles a missing signer, and this change only ensures it resumes under the right authority rather than the default one.

## Migration Plan

**Stage 1 — carry it.** `WriteIntent` and `Receipt` gain the authority; `RouteRequest::Write` is reshaped; `access()` returns it. Nothing persists differently yet because the store is written last.

**Stage 2 — persist it.** Schema 4 to 5, reconstruction refusals for absent, malformed, and contradictory authorities, and the four mutation tests extended and renamed.

**Stage 3 — prove it.** A parked write resuming under its authority across a real process restart, and an authenticated write routing to an authenticated session.

Rollback is per stage until stage 2; after it, a store written under schema 5 is not readable by an earlier build, which is the point of the version.
