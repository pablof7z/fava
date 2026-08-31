## Why

NIP-42 authenticates a relay connection, not a message, but Fava owns the handshake inside a publisher. `Nip42Publisher` performs the whole exchange inline per publish attempt and drops the result into a local `authed: bool` when `publish()` returns. Nothing carries that fact to the next attempt or to a reader on the same socket.

The read path pays for this. `fava-observe` receives the challenge at `crates/fava-observe/src/ingest.rs:114`, discards the challenge text, records `AuthenticationState::ChallengeReceived`, and stops. It imports no signer and no session, so it cannot answer. Of the five `AuthenticationState` variants, only `ChallengeReceived` has a production producer. Authenticated reads are unreachable today.

Writes are no better. `fava-routing::RouteRequest::access()` hardcodes `RelayAccess::Public` for every write, so no automatic write route can request an authenticated session at all.

`docs/spec/ARCHITECTURE.md` already names `fava-auth` as the owner of this lifecycle, and `.planning/REQUIREMENTS.md` OWN-07 already assigns it exclusive ownership with named falsifiers. Neither has an implementation.

## What Changes

- Add `fava-auth`, owning one NIP-42 challenge lifecycle per `(RelaySessionKey, generation)` where the key's access is `Authenticated`. It leases that session, watches the shared inbound stream for challenges, consults an application policy, signs through `fava-session`, sends the client `AUTH` frame, and records the outcome.
- Move `AuthenticationState` from `fava-query` to `fava-relay`, beside `RelayAccess`, and extend it with the terminal states a lifecycle owner produces: accepted, failed, and the relay's own `restricted:` text on acceptance-with-refusal. One enum describes the lifecycle; `fava-auth` owns the lifecycle that produces it.
- Move `BoundedText` to `fava-relay` and delete both existing copies, `fava_query::BoundedText` and `fava_transport::BoundedReason`, which its own doc records as a deliberate duplication.
- **BREAKING** Remove `Query::with_relay_access`. `Fava::with_account` is the one way an application names the account work runs as.
- Stop `fava-observe` interpreting `RelayMessage::Auth`. It consumes authentication outcomes by session identity instead of deriving state from the wire, so one fact has one source. `Accepted::AuthenticationRequired` is deleted.
- Add `AuthenticationPolicy` as the application's decision seam, supplied once at assembly through `FavaBuilder::authentication_policy`. `decide` is synchronous and performs no effects, matching `DeliveryPolicy`; a blanket implementation makes a closure a policy directly. One policy answers every challenge on every session.
- Let a policy answer `Defer` when a person owns the decision. Nothing is signed or sent, the demand is retained with an identity, and work that needs the session parks under its existing identity rather than failing. The application enumerates deferred demands, watches a signal for changes, and answers out of band; the answer wakes the parked work, exactly as attaching a signer wakes a parked write (`crates/fava-publication/tests/owner_lifecycle.rs:241`, WRITE-008).
- A deferred demand is invalidated when its session generation is replaced, so an answer given after a reconnect resolves nothing rather than authenticating a connection the person never saw.
- **BREAKING** Replace `Fava::by(author)` with `Fava::with_account(public_key)`, which names the account a query or publication runs as. It selects the relay-session access authority for both paths, and the author of a payload that carries none. Work with no selection uses `Session::current_account()`, then public access.
- **BREAKING** Delete `AuthorlessPayload`. It exists only to reject an authored payload under `PublishAs::publish`, which was correct while the verb asserted authorship and is wrong once it names an account. `with_account(y).publish(builder.by(x))` publishes `x`'s event over `y`'s session.
- **BREAKING** Carry the `RelayAccess` a write was accepted under through `WriteIntent` and `Receipt`, so a write parked awaiting a signer resumes after restart under the same access authority. `RouteRequest::Write` gains it and `RouteRequest::access()` stops hardcoding `RelayAccess::Public`.
- **BREAKING** Bump the redb write-store schema from 4 to 5. `validate_schema` and `redb_schema_mismatch_refuses_without_fallback` stay: a store written by an earlier build refuses to open with a named error rather than partially deserializing.
- Bound the relay challenge. It arrives as `nostr::message::RelayMessage::Auth { challenge: Cow<'a, str> }` with no length limit; `fava-auth` accepts it through a `Challenge` type that refuses rather than truncates.
- Bound re-authentication. Mid-connection re-challenge is permitted by the spec and observed in deployment, so attempts per session generation are capped.
- Re-authenticate after reconnect. A reconnect mints a new transport generation with no memory of a prior `AUTH`, and nothing re-authenticates today.
- Delete `Nip42Publisher` once the NIP-42 handshake is proven against a real relay (`communities-relay`) through `fava-auth`. That real-relay assertion is the only real-relay NIP-42 evidence in the repository and must not lapse. It previously lived in the downstream acceptance application, which was removed from the repository on 2026-08-31 before the proof was migrated (task 6.1 below was never completed); there is currently no in-repo mechanism to produce this evidence, and `Nip42Publisher` must not be deleted until a replacement one exists.
- Hold a publication attempt whose session has a deferred demand instead of recording `AuthenticationDenied` at `crates/fava-publication/src/delivery.rs:196`. A write must not fail while its dialog is still open.
- Rename the four `schema_v4_*` write-store tests. They mutate rows in the current schema and assert reconstruction refuses them; the prefix wrongly suggests they test an old schema version.

## Capabilities

### New Capabilities

- `identity/relay-authentication`: Who Fava authenticates as on a relay connection, when it answers a challenge, what it does when the relay refuses or the connection is replaced, and how the outcome reaches query and publication evidence.
- `identity/account-selection`: How one account selection reaches both the relay session identity and event authorship across queries and publications, and what happens when nothing is selected.

### Modified Capabilities

None. `openspec/specs/` holds no main specs yet, so both capabilities are introduced here.

## Impact

New crate `fava-auth`, added to the workspace members and `[workspace.dependencies]`.

New public API: `Fava::authentication()` exposing `pending`, `subscribe`, and `answer`; `AuthenticationDemandId` and `PendingAuthentication`; `AuthenticationDecision::Defer`; `AuthenticationState::AwaitingAnswer`.

Changed public API: `Fava::by` and `Query::with_relay_access` removed in favour of `Fava::with_account`; `FavaBuilder::authentication_policy` added; `AuthorlessPayload` removed; `RouteRequest::Write` reshaped; `WriteIntent` and `Receipt` carry a `RelayAccess`.

Moved public API: `AuthenticationState` and `BoundedText` now live in `fava-relay`; `fava_transport::BoundedReason` is deleted.

Changed persisted format: redb write-store schema 5. Stores written by earlier builds refuse to open.

Removed: `Nip42Publisher` and its `build_auth_event`, after the real-relay proof moves to `fava-auth`.

`fava-observe` stops decoding `RelayMessage::Auth` and reports authentication state from the owner's outcomes, gaining every reachable state without learning the protocol.

Satisfies `.planning/REQUIREMENTS.md` OWN-07 and its named falsifiers `nip42_challenge_state_lives_only_in_fava_auth` and `auth_denied_for_one_access_context_leaves_another_running`. Constrained by OWN-05's `no_crate_outside_fava_runtime_spawns_a_task`: `fava-auth` spawns nothing directly and runs through `fava-runtime`.

`docs/spec/ARCHITECTURE.md` and OWN-07 were reworded in commit `24049a6a` to state that one account selection supplies both identities while they remain distinct values.
