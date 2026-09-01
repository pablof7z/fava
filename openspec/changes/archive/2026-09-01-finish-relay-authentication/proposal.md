## Why

`fava-auth` exists and answers challenges, but nothing assembles it. `FavaBuilder` has no way to supply an authentication policy and `crates/fava/src` names `fava_auth` nowhere, so no `Fava` an application builds can authenticate to a relay. Every `AuthenticationState` variant now has a producer inside `fava-auth`, and none of them can reach an observation: `fava-query`'s `RelaySourceState::AuthenticationRequired` (`crates/fava-query/src/evidence.rs:218`) has no producer at all since `fava-observe` stopped decoding challenges. Authenticated reads remain unreachable, which is the gap `own-relay-authentication` opened to close.

That change is 21 of 51 tasks in and its remaining plan no longer matches the code.

**Its design describes a transport that was replaced.** It has `fava-auth` "watch the shared inbound stream for challenges" and cites the challenge arriving at `crates/fava-observe/src/ingest.rs:114`. There is no shared inbound stream — `RelayInbound`, `RelayMessageStream`, and `RelaySession::messages()` were deleted — and `ingest.rs` is gone. Challenges reach exactly one named reader, and connection resets reach their own. Its task 3.1, deleting `fava-observe`'s authentication decoding, is already done.

**Its gate on `Nip42Publisher` protects nothing.** The change keeps that duplicate NIP-42 implementation alive until the handshake is proven against a real relay, because that proof "must not lapse". The proof lived in the downstream acceptance application, deleted on 2026-08-31. A search for a real-relay harness across `crates/` finds none, and the only remaining mention of `communities-relay` is `docs/issues/0050-simple-groups-live-proof.md:117` recording that it is unavailable. So a second component holds NIP-42 challenge state — which that change's own spec forbids by name — to preserve evidence that no longer exists.

**It is three changes bundled.** Relay authentication, account selection (`Fava::with_account` replacing `by`, deleting `AuthorlessPayload`, removing `Query::with_relay_access`), and write access authority (`RelayAccess` carried through `WriteIntent`, `Receipt`, and `RouteRequest::Write`, with a redb schema bump from 4 to 5). Authenticated reads need none of the latter two: `Query::with_relay_access` (`crates/fava-query/src/lib.rs:184`) already names an authenticated session key today. Bundling them gates a persisted-format bump and a public API break on NIP-42, and gates NIP-42 on neither.

This change finishes authentication and nothing else.

## What Changes

- Supersede `own-relay-authentication`. Its authentication work is carried here against the transport that now exists; account selection and write access authority are unbundled into their own changes, named under Impact and not started here.
- Add `FavaBuilder::authentication_policy` and assemble `fava-auth` into the engine, so an application can build a `Fava` that authenticates. This is the one thing standing between a working owner and a reachable capability.
- Read challenges from the session's challenge reader and connection resets from its connection reader, rather than from a shared stream that no longer exists. `fava-auth` already does this; the design records it as the shape rather than as a migration.
- Give `RelaySourceState::AuthenticationRequired` a producer again, sourced from the authentication owner's outcome keyed by session identity rather than decoded from the wire by the observation owner. One fact, one source, and every `AuthenticationState` variant reachable through a real observation.
- Release the authentication lease when no authenticated work remains for a relay, so watching for challenges does not hold a session open forever.
- **BREAKING** Delete `Nip42Publisher` and its `build_auth_event` from `fava-publisher-nip01`, and the crate doc line claiming a NIP-42 variant. It is a second component holding challenge state, which `identity/relay-authentication` forbids; the evidence that justified keeping it is already gone, so keeping it costs a duplicate handshake and buys nothing.
- Establish a real-relay NIP-42 proof owned by `fava-auth` against a relay that demands `AUTH`, replacing what the deleted acceptance application used to assert. This is a new obligation with no current home, and it is why `Nip42Publisher`'s deletion is stated as a separate task rather than folded into it.
- Hold a publication attempt whose session has a deferred demand rather than recording `AuthenticationDenied` at `crates/fava-publication/src/delivery.rs:196`. A write must not fail while the person who owns the decision is still deciding.

## Capabilities

### New Capabilities

- `identity/relay-authentication`: who Fava authenticates as on a relay connection, when it answers a challenge, what happens when a person owns the decision or the connection is replaced, and how the outcome reaches query evidence.

### Modified Capabilities

None. `openspec/specs/` holds no main specs yet, so the capability is introduced here. It replaces the same-named delta in `own-relay-authentication`, which is superseded.

## Impact

Deletes `openspec/changes/own-relay-authentication`. Two of its concerns are unbundled and need their own changes, neither started here:

- **Account selection.** Replace `Fava::by(author)` (`crates/fava/src/lib.rs:197`) with `Fava::with_account(public_key)` naming the account work runs as for reads and writes alike, delete `AuthorlessPayload` (`crates/fava/src/publication.rs`), and remove `Query::with_relay_access` from the public surface. A naming reshape of the public API; it improves how an authenticated session is selected but is not required to select one.
- **Write access authority.** Carry the accepted `RelayAccess` through `WriteIntent` and `Receipt`, reshape `RouteRequest::Write`, and stop `RouteRequest::access()` returning `RelayAccess::Public` unconditionally (`crates/fava-routing/src/lib.rs:84`), so an automatically routed write can request an authenticated session and a parked write resumes under the access it was accepted under. Bumps the redb write-store schema from 4 to 5.

Changed public API: `FavaBuilder::authentication_policy` added; `Fava::authentication()` exposing `pending`, `subscribe`, and `answer` reaches the public surface with the engine assembly.

Removed public API: `Nip42Publisher` and `build_auth_event`.

Unchanged: no persisted format changes, so no schema bump and no migration. `Query::with_relay_access` stays until account selection replaces it.

Satisfies `.planning/REQUIREMENTS.md` OWN-07 and its falsifiers `nip42_challenge_state_lives_only_in_fava_auth` and `auth_denied_for_one_access_context_leaves_another_running`. Constrained by OWN-05: `fava-auth` spawns nothing directly and runs through `fava-runtime`.
