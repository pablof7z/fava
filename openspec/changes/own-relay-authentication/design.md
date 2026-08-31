## Context

See `proposal.md` — Why.

Four facts about the current code shape this design.

`WebSocketTransport` keys live sessions by `RelaySessionKey` in a registry (`crates/fava-transport-websocket/src/lib.rs:37-48`). `reuse()` returns the same `Arc<WebSocketRelaySession>` to every acquirer, and an establish race closes the loser. Exactly one socket exists per key.

`Consumers::fan_out` (`crates/fava-transport-websocket/src/fanout.rs:128-136`) delivers every inbound item to every registered consumer with no content filtering. A relay `AUTH` frame already reaches every holder of that session.

`RelaySessionKey.access` is already `RelayAccess::Public` or `RelayAccess::Authenticated(PublicKey)` (`crates/fava-relay/src/lib.rs:8-22`). An authenticated session is a different key, so a different socket, from the public one.

`PublishPayload::into_intent` already takes `Option<PublicKey>` (`crates/fava/src/publication.rs:341`) and lets each payload decide: `EventBuilder` and `EventEdit` require it, while `AuthoredEventBuilder`, `UnsignedEvent`, and `Event` ignore it.

`nostr` is pinned at `=0.45.3` and carries `ClientAuthentication`, `Nip42Tag`, `is_valid_auth_event`, and `MachineReadablePrefix` with `AuthRequired` and `Restricted` variants.

`AuthenticationState` already exists at `crates/fava-query/src/evidence.rs:270` with five variants. `fava-relay` is 21 lines owning `RelayAccess` and `RelaySessionKey`, depends only on `nostr`, and `fava-query` already depends on it.

## Goals / Non-Goals

**Goals:**

- One authentication per connection, serving reads and writes without either learning the protocol.
- No change to how `fava-observe` or the publisher acquire sessions.
- The account a write was accepted under survives a restart.

**Non-Goals:**

- A second authentication mechanism. NIP-42 is the only one, so no swappable-handshake contract is introduced.
- Migrating existing write stores. Early design, no public consumers; a store from an earlier format is refused and discarded.
- Relay-side validation. Fava is a client; it builds and sends the challenge response and never verifies one.
- Authenticating on demand for a specific query. Authentication belongs to the connection.

## Decisions

### One crate, not a contract and an implementation

`fava-auth` holds the values, the lifecycle state, the policy contract, and the handshake driver together.

`AGENTS.md:58` requires separating a replaceable contract from its implementation, but that rule is about mechanisms with a possible alternative. NIP-42 is the only relay authentication mechanism, so a `fava-auth` / `fava-auth-nip42` split would stabilise a contract no second implementation can ever satisfy. `docs/spec/ARCHITECTURE.md` names one crate.

The genuinely replaceable axis is the application's decision, and that is `AuthenticationPolicy`, a trait inside the crate.

It takes the same shape as `DeliveryPolicy` (`crates/fava-delivery/src/lib.rs:29`): a synchronous decision over facts, performing no effects. A blanket implementation over `Fn(&AuthenticationDemand) -> AuthenticationDecision` makes a closure a policy directly, so there is one way to supply one, not a trait plus a convenience constructor.

### A person's answer parks the work; it does not block the decision

`AuthenticationDecision` is three-valued: `Authenticate`, `Decline`, `Defer`. A policy that needs a person answers `Defer` and returns immediately. Nothing is signed or sent. The demand is retained under an `AuthenticationDemandId`, and work needing that session parks under its existing identity.

The application enumerates deferred demands through `Fava::authentication().pending()`, watches `subscribe()` for changes, and resolves one with `answer(id, decision)`. The answer wakes the parked work.

This reuses a mechanism the repository already proves rather than inventing one. `crates/fava-publication/tests/owner_lifecycle.rs:241` asserts that attaching a signer wakes a write parked under the same receipt identity (WRITE-008). A challenge awaiting a person is the same shape: parked work plus an out-of-band event that wakes it.

*Alternative considered:* an asynchronous `decide` returning a future the application resolves when the dialog closes. Rejected on three counts. A dialog can stay open for minutes, so the owner must either impose a deadline that kills real interaction or await unboundedly inside a decision seam, which is the effect-performing `DeliveryPolicy` deliberately excludes. A pending future has no identity, so a user interface cannot enumerate every relay currently asking. And it cannot be invalidated when the connection is replaced underneath it, so an answer given after a reconnect would authenticate a session the person never saw.

*Alternative considered:* answering `Decline` and relying on the relay to re-challenge once the person approves. Rejected: the work is in flight now, and there is no next connection to re-challenge on.

A deferred demand is scoped to its session generation. When the generation is replaced the demand is dropped, the application's signal fires, and a stale answer resolves nothing.

`AuthenticationState` gains `AwaitingAnswer` so query evidence distinguishes "waiting on you" from "waiting on the relay".

### A held publication is not a denied one

`crates/fava-publication/src/delivery.rs:196` maps `PublishOutcome::AuthenticationRequired` to `RelayDeliveryOutcome::AuthenticationDenied`. While a demand for that session is deferred, the attempt is held under its existing receipt identity and woken by the answer, rather than recorded as denied. A write must not fail while its dialog is still open.

No new delivery outcome noun is introduced: the attempt parks and resumes through the same path a parked write already uses.

*Alternative considered:* a `fava-nip42` vocabulary crate on the `fava-nip65` model. Rejected: `AGENTS.md:38` forbids naming a crate after a NIP document, the existing `fava-nipXX` crates are pure and stateless while this owns a lifecycle, and the architecture already named the owner.

### The authenticator watches; it is not called

`fava-auth` acquires its own lease on each `Authenticated` session key and watches the shared inbound stream. When a challenge arrives it consults the policy, signs, sends, and records.

This works only because the socket is shared per key and inbound is broadcast. The consequence is that neither existing `acquire_session` caller changes, and a relay's unsolicited challenge — which the spec permits at any moment — is seen even when no publication is in flight.

*Alternative considered:* invoking `fava-auth` from whoever received the challenge. Rejected: it requires `fava-observe` to depend on `fava-auth` and gain an auth code path, it lets two callers race the same challenge, and it cannot see a challenge that arrives while only a reader is attached.

The cost is one long-lived lease per authenticated relay, and attribution to specific query or publication work is indirect, reached through the session identity rather than a direct call edge.

### One lifecycle enum, owned where relay identity is owned

`AuthenticationState` moves from `fava-query` to `fava-relay` and gains the terminal states a lifecycle owner produces: `Accepted`, `Failed { reason }`, and the relay's own text on `AcceptedButStillRefused { message }`, which currently carries none.

`fava-relay` is where `RelayAccess` already lives — who Fava claims to be on a connection. How far that claim got is the same concept's state. The crate depends only on `nostr` and `fava-query` already depends on it, so nothing gains an edge and no cycle appears. `fava-auth` owns the lifecycle that produces the value, exactly as `fava-transport` owns the socket whose identity `fava-relay` names.

Leaving it in `fava-query` would make a query-evidence crate the definer of authentication vocabulary it does not own, and putting it in `fava-auth` would force `fava-query` to depend on a crate that pulls in transport, session, and runtime.

*Alternative considered:* a second enum in `fava-auth` for terminal outcomes, translated into the query-facing one. Rejected under `AGENTS.md:44` — a synonym or alternate representation of an existing noun is itself an architecture change, and a translation function between two near-identical enums is the accretion this design exists to remove.

`BoundedText` moves down with it. Its doc at `crates/fava-query/src/evidence.rs:423` records that `fava_transport::BoundedReason` is a deliberate copy with identical 512-byte semantics, kept to avoid a contract dependency. Placing the enum in `fava-relay` would make a third copy; instead the type lands in `fava-relay`, which everything already depends on, and both copies are deleted.

### One fact has one source

`fava-observe` stops decoding `RelayMessage::Auth`. `Accepted::AuthenticationRequired` (`crates/fava-observe/src/ingest.rs:38`) and its branch at `completions.rs:384` are deleted, and the observe path reports authentication state from the owner's published outcome, keyed by session identity.

Under a watching authenticator both components see the same frame on the same broadcast stream. Leaving observe to interpret it as well would leave two components deriving one fact from one wire event, able to disagree about a challenge observe read and the owner declined.

### Authentication state is keyed by session and generation

`SessionAuthentication` tracks one `RelaySessionKey` and the transport generation its verdict belongs to. A reconnect advances the generation and the prior verdict stops applying.

This is what makes OWN-07's two falsifiers hold. `nip42_challenge_state_lives_only_in_fava_auth` holds because no other component keeps a copy — notably `Nip42Publisher`'s local `authed: bool` is gone. `auth_denied_for_one_access_context_leaves_another_running` holds because `Authenticated(a)` and `Authenticated(b)` are different keys on different sockets.

### One `with_account` verb across both paths

`Fava::with_account(public_key)` replaces `Fava::by(author)`. It names the account the work runs as; the payload keeps the last word on authorship.

`by` set authorship only and could not select relay access, so a write to an auth-gated relay was impossible. One selection now reaches both. The existing `into_intent(Option<PublicKey>)` signature already produces the required behaviour with no new machinery: an authorless payload takes the account as its author, an authored one ignores it.

`AuthorlessPayload` becomes dead and is deleted. Its only job was rejecting an authored payload under `PublishAs::publish`, correct when the verb claimed authorship and wrong once it names an account.

`EventBuilder::by` in `fava-write` is untouched: it sets the author on an unsigned event, a different thing.

`Query::with_relay_access` is removed rather than kept as a primitive. A query has no author, so it and `with_account` select the same thing by two names, and keeping it would leave the parallel path this change exists to close.

*Alternative considered:* keeping `by` for writes and adding `authenticated_as` for reads. Rejected: two verbs for one idea, and it leaves the write-side access gap unfixed.

### The access authority is durable on the write

`WriteIntent` and `Receipt` carry the `RelayAccess` the write was accepted under. `RouteRequest::Write` carries it, and `RouteRequest::access()` returns it instead of `RelayAccess::Public`.

`RelayAccess`, not the account's public key. `WritePayload::Edit` already stores `author: PublicKey` resolved once at acceptance; a second public key beside it would be equal in almost every case and free to diverge silently. Routing does not want the account, it wants the authority its destinations execute under, which is what `RouteRequest::access()` returns.

Reading the current selection at route time would be cheaper and wrong. A write parked awaiting a signer and resumed after restart would find whatever is selected then — possibly nothing, possibly a different account. Authorship is already durable in `WritePayload`, so the event would still sign correctly and route publicly, the relay would answer `auth-required:`, and `crates/fava-publication/src/delivery.rs:196` would record `AuthenticationDenied` blaming the relay. Nothing in the evidence would say the identity was lost. This is the same failure shape as a verdict held in a local `bool`: a fact about identity kept somewhere shorter-lived than the work it governs.

Persisting it bumps the redb schema from 4 to 5.

### The schema check stays

`validate_schema` (`crates/fava-write-store-redb/src/schema.rs:75-89`) and `redb_schema_mismatch_refuses_without_fallback` remain. A store from an earlier build is a wrong version every time the schema moves, and a named refusal beats whatever a partial deserialize produces.

The four `schema_v4_*` tests are unrelated to versions despite the prefix: they mutate rows in the current schema — a swapped author, a contradictory receipt id, a signature flipped to signed, an oversized shortfall — and assert reconstruction refuses each. They gain one case for a tampered account field and lose the misleading prefix.

### The challenge is bounded by refusal

`Challenge` refuses empty or over-bound text rather than truncating, following the `fava-nip02` boundary style. The relay's challenge arrives as an unbounded `Cow<'a, str>`, and `fava-query::BoundedText` truncates, which is right for evidence text and wrong for a value that must match exactly on the wire.

Attempts per generation are capped. Mid-connection re-challenge is permitted by the spec, and at least one deployed relay re-challenges continuously regardless of whether the client answers.

### Machine-readable prefixes come from `nostr`

`MachineReadablePrefix::parse` already distinguishes `auth-required:` from `restricted:`. `fava-auth` defines no prefix type: `AGENTS.md:40` forbids repeating what a primitive owns, and `RelayShortfall` already means something else in query evidence.

`AuthenticationDemand` carries `fava_transport::RelaySessionIdentity`, which is already `{ key: RelaySessionKey, generation: RelaySessionGeneration }`, rather than restating those two fields and flattening the generation to a raw integer.

## Risks / Trade-offs

**The only real-relay NIP-42 proof is lost if `Nip42Publisher` is deleted before a replacement exists** → the assertion of the `AUTH` challenge and kind-22242 response against `communities-relay` used to live in the downstream acceptance application's `src/phase_f.rs`; that application was removed from the repository on 2026-08-31 before the proof was migrated to `fava-auth`. A replacement real-relay proof must exist, move to `fava-auth`, and pass before the publisher is removed, not after.

**A long-lived lease per authenticated relay holds a socket open** → the lease is released when no authenticated work remains for that key, and the session registry already refcounts holders.

**Attribution is indirect under a watching authenticator** → the outcome is published against the session identity, and query evidence already carries `RelaySessionKey`, so work joins its outcome by key rather than by call edge. ARCHITECTURE's "exact attribution" is satisfied by identity, not by call stack.

**A relay may re-challenge faster than the attempt bound allows useful work** → the bound is per generation and the outcome says the bound was reached, so an application can distinguish a hostile relay from a refusal.

**Moving `AuthenticationState` and `BoundedText` touches every crate that names them** → both moves are mechanical re-imports with no behaviour change, and `fava-relay` is already a dependency of every crate involved.

**Boxing and reshaping public types breaks call sites** → Symbol Gate currently reports 3 of 1375 symbols signed with no signed policy, and `AGENTS.md:17` makes public API breaks routine, so the cost is mechanical.

## Migration Plan

No data migration. Schema 5 refuses a schema 4 store with an exact reason; the file is deleted.

Ordering matters in one place: the real-relay proof moves to `fava-auth` and passes before `Nip42Publisher` is deleted.

## Open Questions

None.
