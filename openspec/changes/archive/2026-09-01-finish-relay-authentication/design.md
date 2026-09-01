## Context

See `proposal.md` — Why. What already exists, and what the previous plan assumed that no longer holds:

- `fava-auth` is built: `Challenge` with an explicit bound that refuses rather than truncates, `AuthenticationDemand` and `AuthenticationDecision`, `auth_event` building the unsigned kind-22242 response, `SessionAuthentication` tracking one relay across connections with an attempt bound, `AuthenticationPolicy` with a blanket implementation over a closure, and the deferred-demand path with a stable identity and a change signal.
- It reads what it needs and nothing else: challenges from `RelaySessionExt::challenges`, connection replacement from `RelaySessionExt::connection`. It does not decode frames. The verdict for a proof it sent arrives on that proof's own acknowledgement handle, awaited in its own task so a relay that re-challenges before answering cannot wedge the watch.
- Every `AuthenticationState` variant has a producer inside `fava-auth`. `ChallengeReceived` is produced at `crates/fava-auth/src/state.rs:54`.
- Nothing assembles it. `crates/fava/src` names `fava_auth` nowhere and `FavaBuilder` has no policy input.
- `RelaySourceState::AuthenticationRequired` exists at `crates/fava-query/src/evidence.rs:218` with no producer, because `fava-observe` stopped decoding challenges when its decoder was deleted.
- `Nip42Publisher` still performs its own handshake inline per attempt (`crates/fava-publisher-nip01/src/lib.rs`), holding challenge state a second time.

## Goals / Non-Goals

**Goals:**

- An application can build an engine that authenticates, which is the single missing link between a working owner and a reachable capability.
- Authentication outcomes reach an observation's evidence from one source.
- Exactly one component holds challenge state.

**Non-Goals:**

- Account selection. `Fava::by` stays, `Query::with_relay_access` stays, `AuthorlessPayload` stays. Naming an authenticated session better is a separate change; naming one at all already works.
- Write access authority. `RouteRequest::access()` keeps returning `RelayAccess::Public` for writes, so an automatically routed write still cannot request an authenticated session. That is the same separate concern, and it carries the schema bump.
- Any persisted-format change. Nothing here touches the write store, so there is no schema bump and no migration.
- Re-deriving the transport. Challenges, connection state, and acknowledgements arrive the way they now arrive.

## Decisions

### Split rather than finish the bundle

`own-relay-authentication` bundles authentication, account selection, and write access authority. The evidence that they are separable is that authenticated reads need neither of the other two: `Query::with_relay_access` already produces a `RelaySessionKey` whose access is `Authenticated`, which is all the authentication owner keys on.

The cost of the bundle is concrete. It gates a persisted-format bump from redb schema 4 to 5, and a break of the publication API, on a NIP-42 capability — and gates NIP-42 on a real-relay proof that no longer exists. Splitting lets authentication ship against the same public API applications use today, and lets the schema bump be argued on its own merits when write access is done.

Alternative considered: finish the bundle as written. Rejected because its remaining plan describes a transport that was replaced, and because a schema bump should not ride along with an unrelated capability.

### Delete `Nip42Publisher` rather than keep it gated

The previous change keeps it until the NIP-42 handshake is proven against a real relay, on the ground that the proof "must not lapse". The proof lived in the acceptance application deleted on 2026-08-31. A search across `crates/` finds no real-relay harness, and the only remaining reference to `communities-relay` records that it is unavailable (`docs/issues/0050-simple-groups-live-proof.md:117`).

So the gate holds a duplicate NIP-42 implementation in the tree to protect evidence that is already gone. Meanwhile that duplicate is precisely what this capability's first requirement forbids: a publisher holding challenge state of its own.

Establishing the real-relay proof is a real obligation and is a task here. It is stated separately from the deletion, so that the proof's absence is visible rather than a silent condition on someone else's task list, and so neither blocks the assembly work that makes the capability reachable at all.

### The observation's evidence is a report, not a second source

`RelaySourceState::AuthenticationRequired` exists and has no producer. The tempting fix is to have `fava-observe` notice challenges again. That would put challenge state in two components, which the capability forbids by name and which is the defect the whole change exists to remove.

Instead the observation owner consumes the authentication owner's outcome, keyed by session identity — the same key it already uses for connection state. It reports what another component determined; it derives nothing. That also makes every `AuthenticationState` variant reachable through a real observation, which is what proves the enum is not over-built.

### The lease is released when the last authenticated work ends

The authentication owner takes its own lease per authenticated session key so that an unsolicited challenge is seen whether or not a query or publication is attached. A lease never released is a session never closed. Releasing when no authenticated work remains for that relay keeps the watch honest about what it is for: it exists to serve work, not to hold a socket open.

## Risks / Trade-offs

**The real-relay proof has no home, and this change does not create the harness for free.** → It is a named task, and the deletion of `Nip42Publisher` is a separate task, so the sequence is visible. Deleting the duplicate does not reduce evidence, because the evidence was already deleted with the application that held it.

**Splitting leaves `RouteRequest::access()` returning `RelayAccess::Public` for writes.** → Accepted and stated as a non-goal. Authenticated writes through automatic routing remain unreachable until the write-access change lands. Authenticated writes through explicit relay selection are unaffected.

**Two changes now describe the same capability path.** → `own-relay-authentication` is deleted by this change rather than left to rot, and its two unbundled concerns are named in Impact so they are proposed rather than forgotten.

**`Fava::authentication()` reaches the public surface with the assembly.** → It is the seam a deferring policy needs; a policy that never defers never calls it. Its shape is already built and tested inside `fava-auth`.

## Migration Plan

**Stage 1 — make it reachable.** `FavaBuilder::authentication_policy`, assembly into the engine, and the end-to-end proof that a challenge on an authenticated session is answered through the public API. Nothing else can be verified until this exists.

**Stage 2 — make the outcome visible.** The observation owner consumes authentication outcomes by session identity, giving `RelaySourceState::AuthenticationRequired` a producer and making every variant reachable. The lease is released when the last authenticated work ends.

**Stage 3 — remove the duplicate.** The real-relay proof, then `Nip42Publisher`'s deletion, then the deferred-demand hold in publication delivery.

Rollback is per stage, and no stage changes a persisted format.
