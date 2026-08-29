# Fava architecture deviation ledger — 2026-08-23

Tree: `f5922f3` (main). Baseline: 306 tests passing, 117/118 targets green, the
3 debug-session falsifiers red. Vocabulary gate: **RED** (exit 1).

All twelve area audits complete. **164 findings: 59 critical, 80 major, 25 minor.**

**Read `evidence.md` first.** It contains the single fact that explains every
other finding in this ledger: nothing in this repository has ever been tested
automatically.
Every finding is grounded in an authority line and a code line. Detail lives in
the per-area reports in this directory; this file is the ranked remediation input.

## The one-sentence verdict

The live-query ownership inversion reported in `observe-ownership-collapse.md` is
real and confirmed, but it is not the disease — it is the most visible symptom of
a workspace that never built its three execution owners (`fava-runtime`,
`fava-session`, `fava-auth`) and therefore distributed their responsibilities into
whatever component happened to be holding the call stack.

## What is NOT broken

Recorded first, because the blast radius must not be overstated:

- **The write path.** `Publication::accept` is fully synchronous with no `.await`
  on the acceptance path; `crates/fava/src/publication.rs` is genuinely thin;
  exact generation identity for stale signing/route/delivery completions exists
  and is enforced at the store. The observe failure shape does **not** repeat here.
- **Pure domain logic.** Query evaluation is pure, total, and panic-free (no
  unwrap/expect/index/arithmetic in the evaluator or state rules). `fava-wire`
  encode/decode is total and non-panicking. `fava-nip65` is pure with correct
  NIP-01 tie-break.
- **The unpublished-event rule holds.** `CachedEvent` cannot hold an unsigned
  event; double signature verification; single `admit` caller. AGENTS.md's
  never-copy-local-writes-into-the-cache rule is genuinely enforced.
- **Protocol crates are clean.** Correct dependency sets, no acquisition or
  lifecycle, no private lifecycle nouns, exact approved vocabulary.
- **No unapproved crate exists.** The sole crate unnamed by ARCHITECTURE.md,
  `fava-subscriptions-no-grouping`, is an approved vocabulary addition.
- Chain acyclicity, dedup with full reason retention, unforgeable router
  attribution, explicit-route bypass, side-effect-free preview: all verified.

## Root cause, restated

`fava-runtime` is named by the architecture (`ARCHITECTURE.md:2339`), carries an
approved vocabulary entry (`vocabulary.toml:270`), owns eleven resources — task
execution, timers, bounded channels, transport sessions, provider operations,
panic isolation, cancellation propagation, resource joining, shutdown deadlines —
and **does not exist**. It has no crate, no Cargo member, and no milestone home.

Everything downstream follows mechanically. With no execution owner:

- provider calls became bare `.await`s inside whichever owner needed them;
- 10 detached `tokio::spawn`s appeared across 5 crates, none joinable, none joined;
- `Fava` acquired no `close()` and stayed `Clone`;
- no Fava-owned deadline exists anywhere except one hardcoded 5 s constant inside
  the default publisher — so any *substituted* provider hangs forever;
- reconnect had to live somewhere, so it became a 50 ms loop in the facade;
- and the facade, being the only thing holding all the providers, became the
  de-facto execution owner. `OpenedRelay` is what that looks like in source.

`fava-session` (signer registry, sitting in `fava-publication` as a frozen
`BTreeMap` copied at build time) and `fava-auth` (NIP-42, discarded into a
diagnostic counter) are the same story with different symptoms.

## Ranked remediation waves

Ordering is forced by dependency, not by severity. Contracts must be reshaped
before ownership can move, because the owning crate currently cannot even name
the facts it is supposed to own.

### Wave 0 — Gate integrity

The gates that should have caught this are themselves untrustworthy. Fix them
first so the remaining waves are measured by an honest instrument.

- `vocab-gate-blind-to-nonpublic-nominals` (critical) — closed by `f5922f3`, but
  that fix has defects: a `len(words(name)) < 2` skip silences 6 single-word
  violations including the `Group` homonym, a "must embed a registered noun"
  filter silences 5 more including 2 of the 9 lifecycle owners, and it introduces
  9 false positives (`fava_nip02::IntoIter` is an associated type in an `impl`
  block, not a declaration).
- `collect_spec_vocabulary` harvests `.planning/**/*.md` as vocabulary authority
  (`check_vocabulary.py:214-216`). Any plan, review, or audit note becomes spec.
  CI is red today on `fava-canary` — a binary path mentioned in a review doc.
- `vocab-walk-root-excludes-three-packages` — 3 workspace packages and 21 public
  declarations sit outside the walk root entirely.
- `vocab-spec-side-never-checked-against-reality` — 24/24 `spec_crates` and 16/16
  `spec_symbols` are never verified to exist. This is precisely why nobody
  noticed `fava-runtime` was missing for six milestones.
- `vocab-openedrelay-and-eight-siblings` — `OpenedRelay` is not unique. Eight
  further unapproved lifecycle owners: `fava/query_source.rs:57`,
  `fava-publication/revision.rs:17,87,94`, `fava-routing/chain.rs:110`,
  `fava-router-outbox:31,180`, `fava-transport-websocket:82`.

### Wave 1 — Neutral contract correction

Nothing in Wave 3 is buildable until these land. All are breaking changes; per
decision, no adapter or compatibility path is permitted.

- `relay-session-trait-cannot-multiplex` (critical) — `next_message(&self)` is a
  competing-consumer shape; two consumers steal each other's frames.
  `open_session(key)` is open-not-acquire: no lookup, no refcount. **Shared relay
  work is physically impossible against this trait.** Needs the specified
  `messages() -> Box<dyn RelayMessageStream>` plus acquire/refcount semantics.
- `planner-contract-shape` (critical) — missing `constraints`, plan-diff return,
  demand identity, `retain`/`close`, and in-plan `shortfalls`. Five of six
  specified capabilities are unexpressible.
- `singleton-demand-per-plan` (critical) — the workspace's only `plan()` call site
  passes a one-element slice. Aggregate demand has never reached any planner.
- `validate-plan-private-conformance` (critical) — 9 assumptions enforced in the
  facade, 8 unspecified; forbids multi-filter REQs and non-REQ messages, so
  planner-driven withdrawal is structurally impossible.
- `query-evidence-cannot-name-relays` (critical) — 12 required evidence facts are
  unrepresentable. Empty-with-EOSE is indistinguishable from an unreachable relay.
- `no-live-relay-query-source` (critical) — no `SourceKind` variant for live
  admitted relay events, so a null cache drops them entirely (violates QUERY-005).
- `transport-has-no-byte-queues` (critical) — `send` awaits the sink under a mutex.
- `no-fava-owned-deadline` (critical) — zero timeouts in the transport layer.
- `diagnostics-snapshot-shape` (critical) — none of the 5 specified categories exists.

### Wave 2 — Create `fava-runtime`

Task execution with a join registry, bounded command channels, Fava-owned
deadlines for every provider call, provider panic/stall isolation, cancellation
propagation, shutdown join. Resolves `runtime-crate-absent`,
`runtime-no-shutdown-join`, `runtime-detached-tasks`, `runtime-no-provider-deadline`,
`unbounded-reconnect-storm`, `signer-no-deadline-no-timed-out`.

### Wave 3 — Restore `fava-observe` ownership; delete the facade layer

`crates/fava-observe/Cargo.toml` currently depends only on `fava-query`,
`thiserror`, and `tokio`. It gains observation identity, a registry, logical
per-relay demand, desired plan + diff, shared-work refcount, relay-session
binding, provider-operation generation, and the route session.

Deleted outright, not adapted: `crates/fava/src/relay.rs` and `OpenedRelay`, the
relay coordination in `live.rs` and `routes.rs`, and `Fava::next_subscription`.

Resolves `route-session-owned-by-facade`, `facade-owns-subscription-identity`,
`facade-owns-ingest-pipeline`, `facade-decides-freshness-policy`,
`explicit-open-produces-no-route-plan`, `observation-close-does-not-join`, and
the three original falsifiers.

### Wave 4 — Facade lifecycle and the remaining owner misplacements

- `no-facade-close-or-command-admission` (critical) — `Fava` has no `close`, no
  lifecycle state, and is `Clone`. QUERY-003's shutdown-vs-source-failure
  distinction is unrepresentable.
- `cancel-write-bypasses-publication-owner` (critical) — two public doors mutate
  one lifecycle with different semantics; in-flight signer/delivery work survives.
- `session-owner-misplaced` / `publication-owns-signer-registry` /
  `frozen-signer-registry-never-wakes-parked-write` (critical) — create
  `fava-session`. **This is Phase 07.2's subject**; it must be re-planned on top
  of this wave rather than beneath it.
- `nip42-auth-has-no-owner` (critical) — `fava-auth`. Correctly M8-scheduled;
  listed here so the sequencing is explicit.

### Wave 5 — Independent correctness defects

Real bugs that are not consequences of the ownership inversion and must not be
allowed to ride along unnoticed:

- `ingest-attribution-check-is-a-no-op` (critical) — the sole production caller
  passes the same id as `expected` and `actual`, so `WrongSubscription` is
  unreachable. A relay chooses which accepted filter validates its event.
- `outbox-fabricates-settled-absence-from-source-close` (critical) — a positive
  routing fact manufactured from a query *failure*.
- `router-open-failure-kills-whole-query` (critical) — one refusing router aborts
  the chain and `observe()` returns `Err` with no local view. Violates QUERY-004
  independently of the relay-blocking defect.
- `chain-collapse-tears-down-all-relay-demand` (critical) — silently cancels every
  relay session while the handle stays open and reports nothing.
- `deletion-refused-at-capacity` (critical) — a full bounded cache can never apply
  a deletion, and no tombstone records the attempt.
- `expiry-is-never-swept` (critical) — `EventCache::expire` has no production
  caller; a NIP-40 event stays in every open query forever.
- `only-from-relays-local-shadow` (critical) — a purely local publish empties a
  relay-only query. Blessed by a test named `..._shadows_qualified_cached_predecessor`.
- `auth-required-bypasses-delivery-policy` / `auth-denied-collapsed-into-givenup`
  (critical) — the receipt overstates that bytes never left Fava.
- `router-open-failure-abandons-write` (critical) — one router refusal leaves a
  durably accepted write permanently `Open`: never signed, no lane, no owner.
- `no-nip11-invented-planner-limits` (critical) — zero NIP-11 anywhere; 64/1 MiB
  invented globally, so grouping can change query meaning.

### Wave 6 — Evidence reconstruction and verdict revocation

- `grouping-unprovable-through-observe` (critical) — the RELAY-003 300-query
  acceptance drives the planner, transport, ingest, and CLOSE directly, then reads
  results through a *second* Fava with no transport using `.cache_only()`.
- `router-acquisition-starts-from-fabricated-empty-state` (critical) — the outbox
  canary builds a second engine with its own transport and write store,
  contradicting WRITE-014's "no separate transport stack".
- `router-source-fabricates-empty-initial` (critical) — route revision 1 is empty
  even with a fully warm cache.
- `diagnostics-facade-sole-producer` (critical) — no owner publishes facts;
  OPS-003 has no data path.
- `observe-has-no-evidence-at-the-owner` — `crates/fava-observe/` has no `tests/`
  directory at all. This is the mechanism by which the entire class went undetected.
- `group-relay-acquisition-unproven` — all seven group facade tests use
  `.cache_only()` with a transport that refuses every open.
- No `Router` conformance testkit, no signer conformance kit,
  `fava-subscriptions-testkit` does not exist, `fava-transport-testkit` ships no
  relay fake.

M2, M3, and M4 `passed` / `no gaps` verdicts rest on this evidence and must be
revoked, not amended.

## Note on the falsifier that encodes the defect

Of the three RED tests, `cancelling_observe_while_another_relay_opens_closes_provisional_work`
asserts that `observe()` still times out. Under the specified model `observe()`
returns immediately, so this test must be **rewritten**, not made to pass. See
`REMEDIATION-CORE.md`.

## Audit integrity caveats

- HEAD moved mid-audit from `b221203` to `f5922f3`. The branch tip carried a
  57-line `fava-session` runtime-signer contract in `ARCHITECTURE.md` that main
  does **not** have; citations past `ARCHITECTURE.md:2204` may be off by 57 lines
  depending on when each agent read. Findings are unaffected; line numbers past
  that point need normalizing against `f5922f3`.
- These audit reports themselves break the vocabulary gate, because it ingests
  `.planning/**/*.md` as vocabulary authority. Wave 0 fixes that; until it does,
  gate runs must be read with this directory excluded.

---

## Addendum — process audit (received after the above)

`requirements-process`: 9 critical, 1 major, 1 minor. The process findings are
worse than the code findings, because they explain why the code findings survived.

- `.planning/REQUIREMENTS.md` was created at `277d839` on 2026-08-21 07:44:48 —
  **3h41m after M6 shipped** at `309e421` 04:03:09. All 66 M1–M6 requirements were
  reverse-engineered from finished code and were born checked.
- **113 of the 131 authoritative spec requirement IDs appear nowhere in
  `.planning/`.** There is no `QUERY-004` → `LOCAL-08` edge, so no conjunct loss
  was structurally detectable.
- `LOCAL-08` narrows `QUERY-004` from "a query" to "a **local** query" and parks it
  in M1 — whose own exit gate forbids any networking dependency. The invariant was
  assigned to the one milestone structurally unable to falsify it.
- Five more lost conjunctions of the same shape: QUERY-002 shared work (dropped
  twice, independently), QUERY-003 refusal-leaks-no-relay-work, QUERY-012 (4 of 8
  pull conjuncts), QUERY-011 observation memory bound, WRITE-004 (deadline weakened
  from "before `Accepted` returns" to "before relay acknowledgement").
- No requirement anywhere names the ownership ledger. All 16 architecture
  falsifiers are collapsed into `SUB-08` and deferred to Phase 10.
- For M2–M6 the cited `docs/issues/000N` record's first and only commit **is** the
  implementation commit. 02 and 03 state verbatim "External scenarios were
  inspected, not rerun." Every cited external evidence bundle is absent from disk
  and git-ignored.
- **The 20-second contradiction:** `b184aae` at 08:44:48 recorded 8 known bugs and
  5 High coverage gaps in M1/M3/M5, including the project's own description of the
  `QUERY-004` hang. `da8db46` at **08:45:08** declared "No M1/M2/M3/M4/M5/M6 gaps
  remain."
- Phase 07.1 shipped nine phantom requirements `R1`–`R9` that exist in no registry.
  Phase 07.1.1 has no `VERIFICATION.md` at all, self-marked `passed` rows, and 84
  later commits including behavioral fixes to GROUP-04/07/08/10.

**Verdicts to revoke: M1, M2, M3, M5, M6, Phase 07.1.1.** Downgrade M4 (3 of 4
gates genuinely hold). **Retain Phase 7** — genuine independent rerun, distinct
implementation and verification heads, and it resolved a PLAN-vs-authority
conflict in favour of the authority. It is the one phase that did the job.

Proposed `OWN-01`…`OWN-08` requirements for the ownership ledger are in the area
report, modelled on `SESSION-07`, the only correctly-formed ownership requirement
in the existing corpus.


---

## Addendum — public surface

`public-surface`: 0 critical, 7 major, 6 minor. It deliberately did not re-file
three findings it would have rated critical; they are already held by other areas.

- `source-role-impersonation` — `SourceKind` is a closed two-variant enum stamped
  on every snapshot; `fava-observe` routes changes by it and silently discards
  unmatched ones, and `impl QuerySource for Fava` claims `EventCache` while
  emitting write-store contributions. Query evidence misreports its own provenance.
- `observe-relay-variant-unproducible-by-its-owner` — `ObserveError::Relay(String)`
  is declared by `fava-observe` and produced only by the facade, at 9 sites.
- `runtime-primitives-in-the-public-surface` — `Fava::receipt_changes` returns a
  `tokio::broadcast::Receiver`, and `Observation::attach_cancellation` takes a
  `tokio::watch::Sender` purely so the facade can graft private relay tasks onto
  the owner's handle. Both contrary to `partial-spec-api-semantics.md:330`.
- `write-store-contract-half-optional` — 10 of 21 `WriteStore` methods have
  default bodies; a minimal third-party store builds and fails every edit publish
  at runtime with a string indistinguishable from a transient refusal.
- `live-assembly-accepted-then-refused-at-observe` — `build()` accepts an assembly
  with no transport or planner, then every default `Freshness::Live` query fails.
- `canary-declares-itself-external-then-is-not` — `apps/canary/README.md:3-5` says
  it must not depend on Fava internal crates; it links 9 of them and uses them to
  *be* the client engine, then writes `result_equivalence: true` into the retained
  manifest as a hard-coded literal.
- `canary-m7-scenarios-run-with-no-transport` — the four M7 scenarios advertised
  as public Fava executions install a no-op transport and canary publisher/store.

**Verified clean, and worth recording:** no universal owner or facade depends on
any `-standard`/`-memory`/`-redb`/`-websocket`/`-local`/`-no-grouping` crate; no
contract crate depends on an implementation; no dependency cycles; zero
downcasting and zero feature flags workspace-wide; root `README.md` is accurate
clause-by-clause. The dependency-direction gate genuinely holds. Drift is
concentrated in `docs/issues/`, including eight documents recording
`check_vocabulary.py` as exit 0 when it exits 1 on a clean checkout.


---

## Addendum — evidence audit (the explanation)

`evidence`: 11 critical, 24 major, 3 minor.

**`ci-runs-no-tests`.** `.github/workflows/` contains one file,
`architecture.yml`, with two steps: `python3 tools/check_vocabulary.py` and
`python3 -m unittest tools/tests/test_vocabulary_check.py`. There is no
`cargo test`, no `cargo clippy`, no `cargo build`, no bazel, no canary, no
falsifier run. Verified directly against the file, not only via the audit.

**306 tests pass. No automated process has ever run them.** Every green result
in this repository's history is a result somebody chose to run, on a machine
they chose, at a moment they chose, and then described in a document. That is
the mechanism. Everything else in this ledger is a consequence of it.

Supporting findings:

- `deliberate-breaks-unexecuted` — 41 named deliberate breaks in `features/`;
  5 were ever executed as named. **Zero of 510 commits** carry the `Red:` /
  `Mutation:` record that `FAVA_TDD_BDD_TESTING_GUIDE.md` §16 requires. The
  repository's central anti-self-deception mechanism was never operated.
- `grouping-break-is-inert-production-already-does-it` — SUBSCRIPTION-GROUPING-001's
  named break describes behaviour that already ships: `relay.rs:211` discards
  `plan.demand` and `:269` admits with `&id, &id`. The break cannot fail because
  the defect it describes is the current implementation.
- `vacuous-thread-assertion` — OPS-009's only evidence asserts
  `thread::current().id()` inside a `current_thread` runtime. It cannot fail.
- `grouping-differential-absent` — no grouped-vs-ungrouped result differential
  exists anywhere; both planners are tested in isolation on hand-built demands.
- `nip42-unproven-and-contradicted` — RELAY-007 requires Fava to answer
  challenges. It increments a counter. The scenario is green *because* nothing
  happens.
- `zero-test-crates` — 13 crates have no tests at all, including the 1,478-line
  `fava-publication` and the 542-line `fava-write-store`.
- `canary-evidence-is-neither-executed-nor-retained` — 18 canary scenarios are
  marked `built` but are CLI-only, and `apps/canary/runs/` is gitignored with
  zero tracked bundles. Every external evidence artifact cited by an M2–M6
  verification record is absent from disk.
- `tautological-preview-oracle`, `fixture-asserts-its-own-input`,
  `provenance-supplied-by-fixture` — route plans compared against the same
  planner call that produced them; receipts, revision identity, and
  EVENT-003 provenance asserted back out of struct literals the fixture wrote.

**Requirement coverage, of 131 spec requirements: 56 proven, 39 weak or
non-distinguishing, 36 with no evidence at all.** `OPS` is 0 of 11 proven. `ID`
is 1 of 8. `PROFILE` is 1 of 8. Zero coverage workspace-wide for shutdown join,
shared-work refcount, slow-peer backpressure, blocked-provider isolation, and
provider panic outside a single applier.

### Consequence for the remediation plan

Phase 07.3's scope must widen. A vocabulary gate that runs in CI while the test
suite does not is not a weak gate — it is the wrong gate running alone. Before
any other phase is measured, CI must run the workspace test suite, clippy, the
falsifier corpus, and the canary, and must record the `Red:`/`Mutation:` evidence
the testing guide already requires. Until that lands, every "green" verdict this
remediation produces would carry exactly the same defect as the ones it revokes.

---

## Open cross-owner question raised during remediation (2026-08-23)

**Total router refusal makes an automatic write terminal immediately.**

After the routing failure-isolation fix (`chain::open` now isolates a router's
refusal and returns `Ok` with an attributed shortfall), an automatic write whose
routers all refuse settles as terminal `NoDestination` with coverage reported
settled-absent. The publication owner reads that as contradicting WRITE-027,
which requires the receipt to stay non-terminal while the shortfall is typed and
the write remains open to later destinations.

Raised by the publication agent, which deliberately did not act on it: the
behaviour belongs to the routing owner, and the routing change's own rationale
was to stop fabricating absence. Two owners, one disputed transition.

Neither agent was willing to overrule the other, which is correct. Resolve in
Phase 07.8 or 07.9 with a falsifier that states which outcome WRITE-027 requires
when every router refuses but the chain itself is healthy.
> Historical audit record. Superseded by STATE-ARCH-1; not current implementation guidance.
