---
phase: 08-authentication-hostile-boundaries-and-boundedness
milestone: M8
mode: mvp
status: in-progress
depends_on: [M3, M6]
requirements: [HARD-01, HARD-02, HARD-03, HARD-04, HARD-05, HARD-06, HARD-07, HARD-08, HARD-09, HARD-10]
---

# Phase 8 Plan: Authentication, Hostile Boundaries, and Boundedness

**Goal:** Applications receive exact, isolated outcomes under relay authentication,
malformed or hostile input, overload, provider failure, retry, ambiguity, and
shutdown pressure.

**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` §M8 owns the required
behavior, canary scenarios, exit gates, and falsifier. `docs/spec/ARCHITECTURE.md`
owns `fava-auth`, `fava-nip11`, provider isolation, and Falsifier M.

M8 depends on M3 and M6 only (implementation plan §5). Phase 7 (M7) is owned by a
concurrent agent and is not a dependency of this phase.

## Slices

Each slice is one commit series with observable behavior, executable evidence
that fails before the implementation, and a named deliberate break.

### S1 — `fava-auth`: explicit generation-scoped NIP-42 (HARD-01, HARD-02)

- New crate `crates/fava-auth` realizing the approved `Authentication` concept.
- `RelayChallenge` binds an exact `RelaySessionKey` plus transport generation to
  one bounded challenge string. A challenge from a retired generation is inert.
- `AuthenticationPolicy` is the application-supplied replaceable decision. It
  returns an authorizing `PublicKey` or an exact decline reason. It never sees
  the query filter, the event author, or the signer registry.
- `Authentication` correlates policy, signer selection, NIP-42 kind 22242 event
  construction, bounded challenge/response, and the session-scoped outcome.
- Read path answers challenges and restores demand after acceptance.
- `Nip01Publisher` answers a mid-attempt challenge and terminates the exact
  destination with an auth-denied outcome when policy declines.
- Isolation is structural: `RelayAccess` is part of `RelaySessionKey`, so two
  accounts occupy two sessions and one denial cannot reach the other.

### S2 — `fava-nip11` relay limits reach planning and publication (HARD-04)

- New crate `crates/fava-nip11` owning NIP-11 document values, `RelayLimitation`,
  validation, and the `RelayInformationFetcher` contract.
- New crate `crates/fava-nip11-http` owning bounded HTTP acquisition only.
  Freshness, negative caching, and `FetchCache` use remain M9 work and are not
  claimed here.
- `fava-subscriptions` owns the neutral `RelayLimits` planning value and takes it
  in `SubscriptionPlanner::plan`. `fava-nip11` projects `RelayLimitation` into it.
- `fava-publisher` owns the neutral `RelayWriteLimits` value; the NIP-01 publisher
  refuses knowingly-invalid work before handoff with an exact terminal outcome.
- Absent, malformed, or unfetchable documents stay unknown. No invented default.

### S3 — Hostile relay ingress stays scoped and attributable (HARD-03)

- Invalid id/signature, off-filter, malformed frame, oversized frame, post-CLOSED
  event, stale-generation frame, unattributed frame, EOSE-then-event, NOTICE, and
  mid-frame truncation each produce an exact scoped diagnostic and no state.
- Terminal subscription state is tracked per session generation so a post-CLOSED
  EVENT is refused by identity rather than by luck.
- A never-EOSE relay stays distinguishable from silence and from failure.
- Healthy relays and queries keep running while a hostile relay misbehaves.

### S4 — Offline, ceilings, and ambiguity (HARD-05, HARD-06, HARD-07)

- A destination that was never reachable records offline evidence and does not
  consume the attempt budget; only a real attempt does.
- Real retryable attempts reach `GaveUp` inside the declared ceiling.
- A proven crossed handoff without a relay outcome stays `Unknown` and is never
  rewritten into acknowledged, rejected, or never-sent.

### S5 — Boundedness and provider isolation (HARD-08, HARD-09)

- Explicit bounds with typed refusal or shortfall for inbound frame size, relay
  session pool, wire subscriptions, diagnostics categories, challenge and relay
  text, provider operation deadlines, and shutdown.
- Application-supplied provider work runs under a bounded deadline outside owner
  locks and durable transactions, so a blocking, panicking, late, malformed, or
  cancellation-ignoring provider cannot block unrelated work or shutdown.

### S6 — Real-process evidence, second relay, envelopes, falsifier (HARD-10)

- Seven canary scenarios named by §M8, each through real sockets.
- Hostile relays run as separate processes driven by a deterministic script.
- `nostr-rs-relay` proves NIP-42 challenge behavior and persistence.
- A second relay implementation (khatru, Go) passes the core read/publish subset
  and supplies the advertised NIP-11 limits the limit scenario needs.
- Every run publishes a resource envelope and failure evidence.
- Falsifier: routing malformed relay input straight into event-cache mutation,
  bypassing admission, must fail `hostile-relay-ingress`.

## Bounds Introduced

| Bound | Value | Refusal |
|-------|-------|---------|
| Relay AUTH challenge text | 1,024 bytes | typed `AuthenticationError` |
| Authentication round trip | 10 s | `AuthenticationOutcome::Failed` |
| Inbound relay text frame | 512 KiB | scoped session failure |
| Concurrent relay sessions per transport | 256 | `TransportError::SessionPoolExhausted` |
| Relay message text retained | 4,096 bytes | truncation is reported, never silent |
| Provider operation deadline | per-call, explicit | typed timeout outcome |
| Diagnostics per category | 256 | oldest-first with an exact loss count |

## Exit Gates

- Deterministic hostile scenarios run through real sockets and a separate process.
- At least one real third-party relay proves NIP-42 and persistence behavior.
- A second relay implementation passes the core read/publish subset.
- Resource envelopes and failure evidence are published for every run.

## Explicitly Not Claimed

- NIP-11 freshness, staleness, negative caching, single-flight, and `FetchCache`
  use remain M9.
- NIP-05 remains M9.
- Persistent event-cache profiles remain M9.
- Native platform hostile evidence remains M11.
