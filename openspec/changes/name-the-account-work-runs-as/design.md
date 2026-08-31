## Context

See `proposal.md` — Why. What already holds:

- `EventBuilder::new(kind)` takes no author, and `EventBuilder::by(author)` (`crates/fava-write/src/builder.rs:238`) yields an `AuthoredEventBuilder`. The author rides on the payload, which `authorless-event-builder` settled.
- `RelayAccess::Authenticated(PublicKey)` (`crates/fava-relay/src/lib.rs`) carries the account a session is authenticated as. It is already a separate value from an event's `pubkey`.
- `Fava::by(author)` (`crates/fava/src/lib.rs:197`) yields a `PublishAs` holding an author, and `AuthorlessPayload` (`crates/fava/src/publication.rs:353`) restricts what it accepts to `EventBuilder` and `EventEdit`.
- `Query::with_relay_access` (`crates/fava-query/src/lib.rs:184`) is the only way to name a read's authority, and there is no equivalent for a write.

## Goals / Non-Goals

**Goals:**

- One verb names the account, for reads and writes alike.
- Whose event it is and whose connection it goes over stay separate values, in separate places, all the way down.
- The common case stays short: an application that is one account and publishes as itself names that account once.

**Non-Goals:**

- Carrying the access authority into routing or persistence. A write still routes under public access until `carry-write-access-authority` lands. This change gives that one the verb it needs.
- Changing how a payload names its author. `EventBuilder::by` and `AuthoredEventBuilder` are untouched.
- Any persisted-format change.

## Decisions

### `with_account` names the connection, and authors only when nothing else does

The verb could mean either "publish as this account" or "connect as this account". Making it mean only the second is cleaner, but it costs the common case a second call every time: an application that is Alice publishing as Alice would write both.

So it means the connection, and it *also* supplies the author when the payload has none. The order is stated rather than implicit: the payload's own author, then the selected account, then the session's current account. A payload that states its author always keeps it, so the fallback never overrides — it only fills a gap.

Alternative considered: keep the two entirely separate and refuse an authorless payload with no author. Rejected because it makes the 95% case wordier for a distinction that case does not have, and because a stated resolution order is easy to read where a silent override would not be.

### `AuthorlessPayload` is deleted rather than widened

The marker exists to reject an authored payload under `by`. That rejection was right when the verb asserted authorship — two authors is a contradiction. Under a verb that names a connection it refuses the case the change exists to allow.

Nothing replaces it. `publish` accepts any payload; the author comes from the payload if it has one and from the selection if it does not.

### Reads and writes take the same door

`Query::with_relay_access` goes rather than staying as a second way to say the same thing. A query built under a selection carries that authority; one built without carries public access. Two verbs for one fact is how the read and write paths came to disagree in the first place — writes never got one at all.

## Risks / Trade-offs

**Every call site of `by` moves, across crates, tests, and doctests.** → Mechanical, and the compiler finds all of them. `by` is removed rather than deprecated, so nothing is left half-migrated.

**`with_account` doing two jobs could confuse.** → Mitigated by the resolution order being a stated requirement with its own scenarios, and by the payload's author always winning. The failure mode a silent override would cause — publishing as the wrong account — cannot occur.

**Authenticated writes still do not route.** → Stated as a non-goal and named as the next change. An explicit relay route under a selection works; automatic routing does not until `RouteRequest::access()` stops returning public unconditionally.

## Migration Plan

**Stage 1 — the verb.** `with_account` added, `by` removed, every call site moved. `AuthorlessPayload` deleted and `publish` widened. Author resolution in its stated order, with the refusal when there is nothing to author with.

**Stage 2 — one door for reads.** `Query::with_relay_access` removed and its callers moved to the selection.

Rollback is per stage, and no stage changes a persisted format.
