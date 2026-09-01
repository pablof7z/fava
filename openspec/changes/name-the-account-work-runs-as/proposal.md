## Why

`Fava::by(author)` (`crates/fava/src/lib.rs:197`) asserts who signs an event. `Query::with_relay_access` (`crates/fava-query/src/lib.rs:184`) names whose connection a read runs over. They are two different facts and today they are named by two unrelated verbs, one of which does not exist for writes at all.

NIP-42 forced the distinction into the open. A relay session is authenticated as one account; an event is signed by one author; and those need not be the same. Publishing Bob's event over Alice's authenticated connection is an ordinary thing to want — a shared host posting on a member's behalf — and today it cannot be said.

`AuthorlessPayload` (`crates/fava/src/publication.rs:353`) is what stands in the way. It is a marker trait implemented for `EventBuilder` and `EventEdit` whose whole job is to *reject* an authored payload under `by`. That was right while `by` asserted authorship: naming two authors would be a contradiction. It is wrong once the verb names a connection, because then refusing an authored payload refuses exactly the case above.

The ground has already shifted underneath it. `authorless-event-builder` landed: `EventBuilder::new(kind)` takes no author, and `EventBuilder::by(author)` (`crates/fava-write/src/builder.rs:238`) yields an `AuthoredEventBuilder`. The author now rides on the payload. So the two facts are already separate values in separate places — only the verb still conflates them.

## What Changes

- **BREAKING** Replace `Fava::by(author)` with `Fava::with_account(public_key)`, which names the account work runs as: the relay-session access authority for both reads and writes, and the author of a payload that carries none.
- **BREAKING** Delete `AuthorlessPayload`. `with_account(alice).publish(builder.by(bob))` publishes Bob's event over Alice's connection, which is the case the marker existed to reject.
- One verb names the account for reads and writes alike: a selection opens a query as well as publishing. `Query::with_relay_access` stays, because a router uses it to forward the authority its request was handed (`crates/fava-router-outbox/src/lib.rs:51`) and routers are a replaceable boundary that works through public contracts. It is a router's mechanism, not an application's verb.
- Resolve the author of a payload that carries none in one stated order: the payload's own author, then the selected account, then `Session::current_account()`. A payload with no author and no account to fall back on is refused before acceptance rather than accepted and stranded.
- Keep the two facts separate in the types, not merely in the docs: the selected account reaches the relay session key, and the payload's author reaches the event. Nothing copies one into the other.

## Capabilities

### New Capabilities

- `identity/account-selection`: how one selection names the account work runs as, how an event's author is resolved when the payload carries none, and what happens when the two differ or neither is present.

### Modified Capabilities

- `publication/author-scope`: `by` no longer asserts authorship at the publication verb, and an authored payload is no longer refused. The settled spec describes the verb this change replaces.

## Impact

Changed public API: `Fava::by` removed and `Fava::with_account` added; `Query::with_relay_access` removed; `AuthorlessPayload` removed. Every call site across `crates/fava`, the protocol crates, their tests, and the doctests moves.

Unchanged: `EventBuilder::by` and `AuthoredEventBuilder` stay exactly as `authorless-event-builder` left them. This change does not touch how a payload names its author; it stops the publication verb competing with it.

Does not carry the access authority into persistence or routing. A write still routes under `RelayAccess::Public` because `RouteRequest::access()` returns it unconditionally (`crates/fava-routing/src/lib.rs:84`), so an automatically routed write cannot yet reach an authenticated relay. That is `carry-write-access-authority`, which depends on this one for the verb that names the account.

Unbundled from `own-relay-authentication`, which is superseded. That change bundled this with relay authentication and with the write-access work, gating a public API break on NIP-42 and NIP-42 on nothing.
