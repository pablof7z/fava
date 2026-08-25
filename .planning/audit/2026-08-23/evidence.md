# Evidence audit

**Area:** every executable proof in the workspace — `crates/**/tests/**`, `#[cfg(test)]` modules in
`crates/**/src/**`, `apps/canary/**`, `falsifiers/**`, `features/**`, `tools/tests/**`.

**Authority:** `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` (whole file), `AGENTS.md` gate 6, and the
`AGENTS.md` delivery-workflow rule "Confirm new evidence fails before the implementation and under
its named deliberate break."

**Verdict in one line.** The corpus is 306 green tests that cannot distinguish the implemented
architecture from the specified one, because (a) the workspace's only CI job is a Python vocabulary
check — `cargo test` has never run in CI; (b) 35 of the 41 named deliberate breaks have never been
executed and 2 of the 6 that were executed broke a different mechanism than the one named; (c) the
provider doubles model failure and blocking for exactly one contract (`Signer`) and are
always-succeed-immediately for the rest; and (d) 13 crates — including the 1,478-line publication
lifecycle owner — have zero tests of any kind.

---

## Scope checked

Files read (not merely listed):

- Authority: `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` (all 509 lines), `AGENTS.md`,
  `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` (requirement headings + the MUST
  clauses of QUERY-003/004/009/011/012/013, EVENT-003/004, WRITE-019/020/021/022/029/030,
  RELAY-004/005/006/007/008/011/012, ROUTER-001..004, OPS-004/009/010, GOAL-009),
  `docs/spec/ARCHITECTURE.md` §"The architecture is a hypothesis" — Falsifiers A–P (lines 3130–3440).
- All 12 `features/*.feature` files, all 41 `# fava:falsifier=` lines, all `# fava:evidence=` links.
- All 20 `docs/issues/*.md` deliberate-break records.
- `crates/fava/tests/**` — 19 top-level files + 11 submodules + 4 support modules, 10,119 lines.
- `apps/canary/**` and `falsifiers/**`.
- Every `crates/fava-*/tests/**` and every `#[cfg(test)]` module under `crates/*/src/**`.
- Every `impl <ProviderTrait> for <T>` in test/testkit code (43 doubles).
- `.github/workflows/architecture.yml`, root `Cargo.toml`, every `BUILD.bazel`,
  `apps/canary/scenarios.json`, `git log` (510 commits).

Counts established by search, not assumption:

| Fact | Value |
|---|---|
| Test functions in the workspace | 293 |
| Test functions in `crates/fava/tests/**` | 118 |
| `cargo test --workspace` baseline (BASELINE.md) | 306 passing, 3 failing |
| Named deliberate breaks in `features/` (`fava:falsifier=`) | 41 |
| Deliberate breaks with a recorded execution in `docs/issues/` | 6 (2 of which broke a different mechanism) |
| Commits (of 510) containing the §16 evidence record (`Red:`/`Green:`/`Mutation:`) | 0 |
| CI jobs that run any Rust test | 0 |
| Crates with zero tests | 13 |
| Provider contracts with zero adversarial double | 5 |

---

## Findings

### `ci-runs-no-tests` — critical — behavioral proof

**Authority.** `AGENTS.md:49` gate 6: "public promises have falsifiable evidence at the owning
component". `FAVA_TDD_BDD_TESTING_GUIDE.md:498` review checklist: "Formatting, focused crate tests,
public capstones, and workspace tests pass according to the current merge policy."

**Implementation.** `.github/workflows/architecture.yml` is the only workflow in the repository. Its
only two steps are `python3 tools/check_vocabulary.py` and
`python3 -m unittest tools/tests/test_vocabulary_check.py`. There is no `cargo test`, no `cargo
clippy`, no `cargo fmt`, no `bazel test`, no canary invocation, and no falsifier invocation. Bazel
`rust_test` targets exist in 21 `BUILD.bazel` files (`crates/fava/BUILD.bazel` alone declares 20) and
are never executed by CI.

**Observable distinction.** Every "green" claim in `docs/issues/*.md` is a manual local run recorded
in prose. A regression landing on `main` is caught by nothing. `apps/canary` and `falsifiers/*` are
not even workspace members (root `Cargo.toml:3-38` lists 37 `crates/*` entries and nothing else), so
`cargo test --workspace` cannot reach them regardless.

**Proposed falsifier.** Not a Rust test — a CI job. `cargo test --workspace --all-targets`, `cargo
test --manifest-path apps/canary/Cargo.toml`, and `cargo test --manifest-path
falsifiers/external-null-cache/Cargo.toml` must run on every pull request. Today, deleting the body
of any test function and pushing produces a green pipeline.

**Confidence.** confirmed.

---

### `deliberate-breaks-unexecuted` — critical — behavioral proof

**Authority.** `AGENTS.md:35`: "Confirm new evidence fails before the implementation and under its
named deliberate break." `FAVA_TDD_BDD_TESTING_GUIDE.md:105` §3.6: "Every newly claimed invariant
needs a mechanism-disable check … The linked evidence must fail for the claimed reason. A scenario
that remains green after its protection is removed does not prove the invariant."
`FAVA_TDD_BDD_TESTING_GUIDE.md:487` §14: "Removing/bypassing the protection makes the linked
evidence fail." §15 anti-pattern: "a green test that was never shown to fail when its protection was
removed."

**Implementation.** `features/` declares 41 named deliberate breaks
(`grep -c "fava:falsifier=" features/*.feature`: automatic-publication 5, automatic-routing 5,
explicit-live-query 6, explicit-publication 5, local-source-merge 12, multi-relay-observation 4,
subscription-planning 2, write-recovery 2). `docs/issues/*.md` records an executed break in only six
places: `0001` (line 47), `0004` (line 50), `0005` (line 47), `0006` (line 47), `0007` (line 46),
`0008` (line 51), plus three fully instrumented M7 breaks in `0010` and one in `0018`. No commit in
510 contains the §16 record (`git log --grep="Mutation:"` → 0; `--grep="Red:"` → 0; `--grep="Green:"`
→ 0).

Two of the six recorded breaks are not the break the feature names:

- `features/explicit-live-query.feature:7` names the QUERY-LIVE-001 break as "Treat silence or a
  local timeout as EOSE". `docs/issues/0004-explicit-live-query.md:50` records removing **signature
  verification** instead — which is the INGEST-001 break at `explicit-live-query.feature:43`. The
  QUERY-LIVE-001 break was never run.
- `features/subscription-planning.feature:7` names the SUBSCRIPTION-GROUPING-001 break as "Discard
  wire-to-logical attribution after grouping". `docs/issues/0018-literal-tag-value-filters.md:72`
  records a **case-folded grouping-axis** mutation instead. The attribution break was never run.

**Observable distinction.** 35 invariants are claimed `fava:status=built` with no evidence that
their protection is load-bearing. All 12 local-source-merge breaks and all 5 explicit-publication
breaks are entirely unexecuted.

**Proposed falsifier.** A CI-executable break registry: each `fava:falsifier=` line carries a patch
or a test-only seam, and a job applies each in turn and asserts the named evidence fails.

**Confidence.** confirmed.

---

### `straw-break-signature-verification` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:105-124` §3.6 requires that the linked evidence "fail
for the claimed reason", and §14 requires "The failure is causal, not an unrelated panic or setup
error."

**Implementation.** `docs/issues/0004-explicit-live-query.md:50-54`: "Removing signature verification
from relay ingest, event-cache admission, and the memory provider made `explicit-read-eose` fail."
Disabling verification in three independent owners at once is not a mistake that ships; and it is
already independently protected by
`crates/fava-event-cache-memory/src/lib.rs:203` `invalid_signed_event_is_refused_without_mutation`,
`crates/fava-ingest/tests/admission.rs:20` `forged_wrong_subscription_and_off_filter_events_never_enter_the_cache`,
and `crates/fava/tests/explicit_live.rs:314-325`. Three tests already cover it; the break proves
nothing the corpus did not already prove.

**Observable distinction.** The break tells you nothing about whether a single-site verification gap
(the realistic bug) would be caught — and in fact `fava-ingest` verifies, then `fava-event-cache`
verifies again, so a gap at exactly one site is invisible to this break.

**Proposed falsifier.** Break exactly one site — e.g. delete only the `verify()` call in
`fava-ingest` — and confirm `explicit_live_query_attributes_event_eose_and_exact_cancellation` fails
while the cache-level test still passes. Then the break is attributable.

**Confidence.** confirmed.

---

### `straw-break-constant-off-by-one` — major — behavioral proof

**Authority.** §3.6 examples name real mechanism disables ("accept before write-store commit",
"apply a stale signer completion", "silently drop subscription-planner overflow").

**Implementation.** Three breaks in the repo are off-by-one edits to a constant that the test
literally hardcodes:

- `docs/issues/0010-m7-semantic-writes-and-capability-composition.md`
  `DELIBERATE_BREAK_M7_EVENT_BUILDER_BOUND` — "changed only `MAX_TAGS` from 2000" to 2001; the test
  asserts a 2000/2001 boundary.
- `features/explicit-publication.feature:56` — "Accept 257 explicit relays or 4097 bytes of receipt
  text". `crates/fava/tests/write_bounds.rs:38-53` asserts exactly
  `TooManyExplicitRelays { actual: 257, maximum: 256 }`.
- `features/explicit-live-query.feature:64` — "Report an oversized rejected frame as handed off";
  `crates/fava-transport-websocket/tests/conformance.rs:47-63` builds a 4-byte bound and sends a
  9-byte frame.

**Observable distinction.** None of these can distinguish "the bound is enforced at the right
boundary, atomically, on the real path" from "a constant is compared somewhere". The interesting
case — the bound reached through the assembled path with partial mutation already committed — is
untested: `crates/fava/tests/write_bounds.rs:144` `automatic_route_fanout_is_bounded_before_receipt_mutation`
hand-builds a `RoutePlan` with 257 destinations (`write_bounds.rs:154-176`) and hands it straight to
`MemoryWriteStore::apply_route`. No router in the workspace can produce 257 destinations, so the
path the bound is supposed to defend is never driven.

**Proposed falsifier.** `route_fanout_bound_refuses_a_real_router_chain_without_partial_mutation`:
compose 257 `AppRelayRouter` contributions through `Fava::publish` with automatic routing, assert the
typed shortfall and that `receipt.route_revision` is unchanged.

**Confidence.** confirmed.

---

### `straw-break-compile-error` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:96` §3.3: "A test that merely fails to compile because
everything is absent is useful only until the first real behavior boundary can be exercised."

**Implementation.** `docs/issues/0010-m7-...md`, `DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY`: the break
inserts `use fava_signer as _deliberate_break_m7_forbidden_dependency;` into
`crates/fava-nip02/src/lib.rs` and records "failed with Rust error E0432: no external crate
fava_signer". That proves `Cargo.toml` does not list the dependency; it proves nothing about
behavior, and the same E0432 would fire for any absent crate name including a typo.

**Observable distinction.** The dependency-direction gate it claims to exercise
(`ARCHITECTURE.md:3410` Falsifier O) is about *importable* crates, not *absent* ones. The realistic
violation — a protocol crate reaching a runtime owner through an already-declared transitive
dependency — is not covered.

**Confidence.** confirmed.

---

### `vacuous-thread-assertion` — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1492` OPS-009: "Ordinary observation
does not allocate one operating-system thread per query."
`features/multi-relay-observation.feature:46` declares the break: "Assign a dedicated
operating-system thread per Observation; this scenario no longer remains on one current-thread
runtime." Status `built`.

**Implementation.** `crates/fava/tests/observation_bounds.rs:27-48`
`one_thousand_idle_observations_share_the_current_runtime_thread`. Under
`#[tokio::test(flavor = "current_thread")]` the test body always runs on the calling thread, so
`assert_eq!(std::thread::current().id(), thread)` at lines 37 and 47 **holds no matter what Fava
does** — spawning 1,000 OS threads elsewhere never changes the test's own thread id. Line 39
`assert_eq!(observations.len(), 1_000)` re-counts a `Vec` the loop just pushed to. Every query is
`Query::events().cache_only()` (line 32), so no relay work, no transport, and no `fava-observe`
relay task is ever created.

**Observable distinction.** Apply the named break — spawn `std::thread::spawn` per observation in
`Observer::open` — and this test still passes.

**Proposed falsifier.**
```rust
let before = active_thread_count();               // /proc or mach thread enumeration
let obs: Vec<_> = (0..1_000).map(|_| fava.observe(live_query())).collect();
assert!(active_thread_count() - before < 8, "one OS thread per observation");
```

**Confidence.** confirmed.

---

### `grouping-differential-absent` — critical — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:283` §8: "`fava-subscriptions` — Use differential
tests: **the grouped wire plan and an ungrouped reference plan must produce identical logical query
results and evidence.**" `ARCHITECTURE.md:3321` Falsifier J: "Run the same routed logical demand
through: no grouping; standard grouping; an alternative exact grouping implementation … Applications
must observe identical query results, source evidence, EOSE attribution, cancellation, access
isolation, and limit shortfalls." `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1045` RELAY-003.

**Implementation.** No differential exists anywhere. The two planners are tested in isolation, each
against hand-built demand:

- `crates/fava-subscriptions-standard/tests/grouping.rs:14`, `:49`, `:75`, `:112` — every test builds
  `RelayDemand` values by hand and calls `StandardSubscriptionPlanner::default().plan(...)` directly.
  `one_exact_non_empty_tag_axis_groups_with_exact_values_and_logical_ids` (line 75) constructs **300
  `RelayDemand` values in a loop** (lines 76-85) — the exact pattern the crisis named in the canary,
  reproduced at the owner layer.
- `crates/fava-subscriptions-no-grouping/tests/plan.rs:10` — the crate's *only* test; two hand-built
  demands, asserts message shape.
- No test file imports both planners. `fava-subscriptions-standard` is not even a dev-dependency of
  `crates/fava`.

The assertions are plan-*shape* assertions (`plan.messages.len() == 1`, `plan.attribution.len() ==
1`), never *result* equivalence. An implementation that groups correctly on the wire but mis-attributes
one logical subscription's events to another logical id passes every one of them, because no event
ever flows.

**Observable distinction.** An application running the same query set under the two planners could
receive different `EventRecord` sets. Nothing in the workspace would notice.

**Proposed falsifier.** `grouped_and_ungrouped_planners_produce_identical_query_results`:
```rust
for planner in [standard(), no_grouping()] {
    let fava = assembly_with(planner, scripted_relay_serving(corpus.clone()));
    let obs: Vec<_> = queries.iter().map(|q| fava.observe(q.clone())).collect();
    results.push(obs.iter().map(|o| o.current().events.clone()).collect::<Vec<_>>());
}
assert_eq!(results[0], results[1]);   // identical records AND identical relay evidence
```

**Confidence.** confirmed.

---

### `nip42-unproven-and-contradicted` — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1091` RELAY-007: "The application
supplies an auth policy for exact relay access. **Fava answers challenges**, supports challenge
timing before or after a request, and **re-authenticates after reconnect**. If the application
declines authentication for a publication, that destination terminates with an auth-denied outcome
while unrelated accounts and destinations continue independently."

**Implementation.** `crates/fava/src/relay.rs:300` is the entire NIP-42 handling in the engine:
`RelayMessage::Auth { .. } => diagnostics.authentication_required(key, generation)`. No `ClientMessage::Auth`
is ever constructed anywhere (`grep -rn "ClientMessage::Auth" crates apps` → 0 hits). Fava never
answers a challenge, never re-authenticates, and has no auth-policy surface.

The scenario that claims this is proved — `features/explicit-live-query.feature:55`
QUERY-EVIDENCE-001, status `built` — is backed by two tests, and both pass *because* Fava does
nothing:

- `crates/fava-diagnostics/tests/relay_facts.rs:17` calls `diagnostics.authentication_required(...)`
  directly on the recorder and asserts the snapshot has a distinct `authentication_required` field.
  It is a struct-field test with no engine in it.
- `crates/fava/tests/explicit_live.rs:342` `silence_eose_auth_closed_and_disconnect_are_distinct_facts`
  drives the assembled path, pushes `RelayMessage::auth("challenge")` at line 366, and asserts only
  `facts.authentication_required.len() == 1`. It never checks that an `AUTH` frame was sent back. The
  scripted transport's `sent()` log is available at `explicit_live.rs:39` and is not consulted.

**Observable distinction.** An application against a NIP-42 relay gets no events and no auth-denied
outcome; it gets a diagnostics counter. The green scenario says the opposite.

**Proposed falsifier.** `relay_auth_challenge_is_answered_and_replayed_after_reconnect`:
```rust
script.receive(&RelayMessage::auth("challenge-1"));
wait_until(|| script.sent().iter().any(|f| f.starts_with(r#"["AUTH""#)));
transport.disconnect(relay, 0);
script.receive(&RelayMessage::auth("challenge-2"));
assert_eq!(script.sent().iter().filter(|f| f.starts_with(r#"["AUTH""#)).count(), 2);
```

**Confidence.** confirmed.

---

### `ambiguous-handoff-never-originates` — major — failure isolation / behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:932` WRITE-020: "If Fava cannot
determine whether event bytes reached the destination, it MUST record `OutcomeUnknown` for that exact
attempt." `ARCHITECTURE.md:3335` Falsifier K: "A scripted transport must be able to produce: definite
pre-handoff refusal; definite handoff; **ambiguous loss**; reconnect generation change; **malformed
inbound bytes**." `FAVA_TDD_BDD_TESTING_GUIDE.md:305` §8: "`fava-publication`… Use headless schedules
and model tests for … ambiguous handoff."

**Implementation.** `HandoffOutcome::Ambiguous` is produced by exactly one line of production code
(`crates/fava-transport-websocket/src/lib.rs:127`) and by **no test double anywhere** — the
`Ambiguous` variant is never constructed in `crates/*/tests`, `apps/canary/src`, or `falsifiers/`.
`TransportError::InvalidFrame` is likewise produced only at
`crates/fava-transport-websocket/src/lib.rs:152` and never exercised.

The only `OutcomeUnknown` evidence is `crates/fava/tests/write_settlement.rs:147`, where a
`ManualPublisher` is handed the answer directly. The transport→publisher→receipt chain that turns
ambiguity into a durable fact is never driven.

`crates/fava-transport-websocket/tests/conformance.rs` (the TRANSPORT-001 evidence) has four tests:
success, oversized-refusal, remote disconnect, idempotent close. It has no ambiguous handoff, no
malformed inbound frame, no `open_session` deadline/unreachable-host case, and no generation-change
case.

**Observable distinction.** A durable delivery policy retrying after a genuinely ambiguous socket
write, and an at-most-once policy terminating the lane, are the two behaviors WRITE-020 says must not
be confused. Neither is reachable in any test.

**Proposed falsifier.** Add `Ambiguous`/`InvalidFrame` to a shared scripted transport in
`fava-transport-testkit`, then
`ambiguous_socket_write_records_outcome_unknown_and_the_policy_decides`.

**Confidence.** confirmed.

---

### `transport-testkit-has-no-double` — major — replaceability / behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:375` §11: "A trait with one implementation is not
evidence of substitutability" and step 4: "implement a second materially different provider,
preferably outside the owning crate/workspace boundary." `ARCHITECTURE.md:3139` Falsifier A requires
external implementations of, among others, "a scripted transport", "a static-table router", "a
no-grouping subscription planner", "a no-retry delivery policy".

**Implementation.** `crates/fava-transport-testkit/src/lib.rs` is 56 lines containing four assertion
helpers (`require_handoff_success`, `require_handoff_refusal`, `require_disconnect`,
`require_idempotent_close`). It ships **no transport double at all**. As a consequence eight
private, near-duplicate scripted transports exist across the test corpus
(`crates/fava/tests/explicit_live.rs:51`, `:55`, `:73`; `multi_relay.rs:22`; `automatic_routes.rs:24`;
`simple_groups.rs:768`; six copies of `NoopTransport`; `falsifiers/external-semantic-capability/tests/support/mod.rs:280`),
each modelling a different subset of the contract and none modelling the whole of it.

`falsifiers/` contains exactly two external crates: `external-null-cache` (an `EventCache`) and
`external-semantic-capability` (a materializer). Falsifier A's required external router, planner,
transport, delivery policy, and persistent write store do not exist.

**Confidence.** confirmed. (Overlaps `transport-wire-ingest.md` finding `testkit-ships-no-relay-fake`;
recorded here for the evidence consequence, not re-reported as a new architecture defect.)

---

### `delivery-policy-has-no-second-implementation` — major — replaceability / behavioral proof

**Authority.** §11 as above; `ARCHITECTURE.md:3335` Falsifier K: "standard delivery policy with
no-retry and alternative-fairness policies"; WRITE-019 (`…:922`) requires "the selected delivery
policy MUST eventually stop under a finite declared bound".

**Implementation.** `grep -rn "impl .*DeliveryPolicy for"` across the repository returns exactly one
hit: `crates/fava-delivery-standard/src/lib.rs:27`. There is no second implementation and no test
double. `crates/fava-delivery` is 32 lines with zero tests. No test in the workspace runs Fava
against a policy that gives up, that decides differently on the second call, or that is hostile.

**Observable distinction.** WRITE-019's finite give-up bound and WRITE-020's at-most-once-vs-durable
distinction are both policy-selected and both unproven.

**Confidence.** confirmed.

---

### `zero-test-crates` — critical — behavioral proof

**Authority.** `AGENTS.md:49` gate 6: evidence must exist "at the owning component".
`FAVA_TDD_BDD_TESTING_GUIDE.md:270` §7: "Put proof in the smallest stable component responsible for
it."

**Implementation.** Thirteen crates have no `tests/` directory and no `#[cfg(test)]` module
(verified by enumerating every directory under `crates/`):

| Crate | Source lines | Owns (ARCHITECTURE.md) |
|---|---|---|
| `fava-publication` | 1,478 | acceptance-before-effect, signer/route independence, stale-generation rejection, recovery |
| `fava-write-store` | 542 | the write-store contract + `validate_current_materialization` |
| `fava-router-fallback-relays` | 178 | ROUTER-004 reactive fallback policy |
| `fava-router-app-relays` | 120 | ROUTER-003 app-relay policy |
| `fava-publisher-nip01` | 116 | `OK`/`CLOSED`/`AUTH` interpretation, ambiguity mapping |
| `fava-event-cache` | 93 | the cache contract + admission/expiry defaults |
| `fava-router-testkit` | 91 | the shared router double |
| `fava-transport` | 72 | the transport contract |
| `fava-publisher` | 65 | the publisher contract |
| `fava-signer-local` | 57 | ID-006 key custody |
| `fava-transport-testkit` | 56 | the transport conformance kit |
| `fava-signer` | 50 | the signer contract |
| `fava-delivery` | 32 | WRITE-019/020 policy contract |

`fava-publication` is the sharpest case: it is the owner of the publication lifecycle, and the M7
deliberate break `DELIBERATE_BREAK_M7_STALE_COMPLETION`
(`docs/issues/0010-...md`) removed a predicate from `fava-write-store` — a crate that also has zero
tests — and could only be detected two layers away at `crates/fava/tests/semantic_write_publication`.

**Observable distinction.** Every promise these crates own is proved, if at all, through the
`crates/fava` facade with the surrounding providers replaced by always-succeed doubles. A change that
moves a decision from `fava-publication` into the facade (exactly the crisis's failure mode) breaks
nothing.

**Confidence.** confirmed.

---

### `wire-has-one-happy-path-test` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:299` §8: "`fava-transport` / `fava-wire` /
`fava-ingest` — Use byte-exact scripted relay tests for framing, generation identity, **malformed
input**, off-filter events, signature rejection, **AUTH**, CLOSED, EOSE, **reconnect**, and **stale
frames**."

**Implementation.** `crates/fava-wire/tests/nip01.rs` is 28 lines with one test,
`exact_nip01_req_close_event_eose_and_closed_shapes_round_trip`, covering REQ/CLOSE encode and
EOSE/CLOSED decode on well-formed input. There is no malformed-input decode test, no `AUTH`, no `OK`,
no `NOTICE`, no oversized frame, and no unknown-message-type case. `decode_relay`'s error path is
never exercised.

**Confidence.** confirmed.

---

### `restart-proof-does-not-use-the-public-path` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:333` §9.2: a durability proof must "1. create the fact
through a supported operation; … 4. reopen **through the supported construction path**; 5. observe
the public result". And: "Opening a second engine in the same process is not a process-restart
proof." `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1011` WRITE-029.

**Implementation.** `crates/fava-write-store-redb/tests/process_kill.rs` **is** a real process kill —
it spawns the test binary as a child (`:107-116`), `child.kill()` at `:118`, reaps and asserts
non-success at `:119-120`, then reopens the database. That part is genuinely strong. But:

- the child creates the durable fact by calling `RedbWriteStore::accept/install_signed/begin_attempt/
  record_outcome` directly (`process_kill.rs:35-80`), not through `Fava::publish`;
- recovery is observed via `store.receipt(ReceiptId::from_u64(1))` (`:123-126`), not via
  `Fava::receipt` or `Fava::open_receipts`;
- no test anywhere constructs a `Fava` over a `RedbWriteStore` — `grep -rIl "redb" crates/fava/tests`
  returns nothing, even though `fava-write-store-redb` is a dev-dependency of `crates/fava`.

`features/write-recovery.feature:7` WRITE-RECOVERY-001 claims the facade promise ("the same receipt
and event identity are **queryable** without resubmission") and links this test as evidence. The test
never builds an engine.

Meanwhile the three facade tests whose names say "recovery"
(`crates/fava/tests/semantic_write_failures.rs:357`, `semantic_write_publication/author.rs:65`,
`semantic_write_failures/transient_reads.rs:18`) construct a second `Fava` in the same process over
the same in-memory store — the exact construction §9.2 rules out.

**Proposed falsifier.** `sigkilled_engine_reattaches_the_same_receipt_through_the_public_facade`:
child does `Fava::builder().write_store(RedbWriteStore::open(p)).build()?.publish(event)`, SIGKILL,
parent rebuilds `Fava` over the same path and asserts `fava.open_receipts()` returns one receipt with
the identical `ReceiptId` and event id.

**Confidence.** confirmed.

---

### `grep-gates-presented-as-tests` — minor — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md` §15 anti-pattern: "a line/count/grep gate presented as
behavioral proof."

**Implementation.** Ten `#[test]` functions in the corpus assert on source text rather than
behavior:

- `crates/fava/tests/facade_surface.rs:20` `facade_has_no_write_intent_compatibility_door` —
  `include_str!("../src/lib.rs")` + substring assertions.
- `crates/fava/tests/facade_surface.rs:66` `facade_root_stays_below_the_repository_soft_limit` —
  asserts `lines < 500`.
- `crates/fava/tests/facade_surface.rs:8` `neutral_contracts_remain_available_to_providers` — three
  `std::mem::size_of` discards and a function-pointer coercion; **asserts nothing and cannot fail at
  runtime**.
- `crates/fava-simple-groups/tests/architecture.rs:161`, `:182`, `:240`, `:288`, `:337` — parse
  `Cargo.toml` and `BUILD.bazel` as text.
- `crates/fava-nip02/tests/architecture.rs:51`, `:120` — same.
- `crates/fava-write-store-redb/tests/semantic_write_store.rs:29`
  `redb_semantic_owners_stay_below_the_code_soft_limit`.

`docs/issues/0019-simple-groups.md` lists `cargo test -p fava-simple-groups --test architecture` under
"**Executable falsifiers**". They are useful lint gates; they are not falsifiers and they inflate the
306-test count.

**Confidence.** confirmed.

---

### `empty-assertion-helper` — major — behavioral proof

**Authority.** §15: "a green test that was never shown to fail when its protection was removed."

**Implementation.** `crates/fava/tests/simple_groups/saved.rs:166`:
`fn assert_ordinary_write(_write: &fava::Write) {}` — a function whose name asserts a property and
whose body is empty. Three call sites read as proof: `crates/fava/tests/simple_groups.rs:321`,
`simple_groups/saved.rs:135`, `saved.rs:136`.

**Confidence.** confirmed.

---

### `dead-deliberate-break-branch` — minor — behavioral proof

**Implementation.** `crates/fava/tests/publication_scopes.rs:166-179`
`publication_scopes_are_inert_before_valid_payload` matches on `fava.to(empty)`. The `Ok(scope)` arm
(lines 169-178) is commented `"deliberate-break payload builds"` and asserts a refusal — but in the
correct implementation the `Err` arm at line 167 is taken and the "break" arm never executes. §3.6
requires the mutation to make linked evidence *fail*, not to occupy an unreachable branch.

**Confidence.** confirmed.

---

### `tautological-preview-oracle` — critical — behavioral proof

**Authority.** §9.3: "Diagnostics report what Fava believes. They do not prove their own claims. Use
an independent witness." §6.2: "is this fixture input a cause, or is it the result under proof?"
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:890` WRITE-016 (route preview is side-effect free)
and `:896` WRITE-017 (partial routing uses one receipt).

**Implementation.** Three tests prove a route plan by comparing the system's output against the same
planner called directly on the same routers:

- `crates/fava/tests/automatic_publication.rs:75-96`
  `known_destinations_deliver_now_and_later_route_uses_same_receipt` — line 75 calls
  `fava_routing::preview(&routers, ...)` directly, line 93-96 asserts
  `partial.desired_destinations == preview.destinations.keys()`. Both sides come from the same
  function over the same router vector. Compounding: the `RouteContribution` fed to the
  `DelayedRouter` is hand-built at lines 121-149, including
  `coverage.insert(target, CoverageState::Covered(...))` (129-132) and `CoverageState::Unresolved`
  (141) — §6.2 "Do not set internal coverage flags".
- `crates/fava/tests/semantic_write_capabilities.rs:93-107` — builds a `Publication` provider by hand
  (line 79-86) to reach `preview_semantic_routes`, which **is not on `Fava`** (it lives at
  `crates/fava-publication/src/lib.rs:189`); §15 "a canary helper that conceals an awkward or missing
  public Fava API". Both sides share the same `Arc<CountingRouter>`, whose contribution
  (`crates/fava/tests/support/semantic_write.rs:355-366`) is one constant destination, so the
  assertion reduces to `{relay_url()} == {relay_url()}`. Lines 97-98 additionally assert the double's
  private `previews()`/`opens()` counters.
- `crates/fava/tests/semantic_write_publication.rs:409-465` — identical shape.

**Observable distinction.** A route-derivation defect that affects `preview` and the assembled path
identically is invisible.

**Proposed falsifier.** Assert against a literal expected destination set derived from the seeded
protocol facts, not from a second call into the router chain.

**Confidence.** confirmed.

---

### `fixture-asserts-its-own-input` — critical — behavioral proof

**Authority.** §6.2 "Set up causes, not conclusions"; §14 "Setup provides causes, not expected
conclusions. Production constructors and operations create state."

**Implementation.**

- `crates/fava/tests/write_settlement.rs:28-45`
  `receipt_counts_preserve_complete_mixed_destination_evidence` — the `Receipt` is a hand-written
  struct literal at `write_settlement.rs:261-324` (destinations map 270-290,
  `outcome: ReceiptOutcome::Complete` 317, `route_settled: true` 319,
  `signature: SignatureState::Signed` 302, `desired_destinations` 321). Lines 31-34 then count that
  map. A publication pipeline that never records a `Pending` or `Unknown` destination passes
  unchanged.
- `crates/fava/tests/semantic_write_contract.rs:104-141`
  `materialization_identity_changes_but_receipt_identity_does_not` — constructs
  `PublicationEvidence { .. }` as a literal (120-129) and asserts the fields read back (135-139). No
  generation ever changes.
- `crates/fava/tests/semantic_write_contract.rs:77-90` — the mock `ExactMaterializer`
  (`semantic_write_contract.rs:50-75`) contains `assert!(source.is_none())` **inside its own
  `materialize`** at line 68 (§15 "a mock that implements the behavior under test"); the test then
  passes `None` and `timestamp` explicitly and asserts them back.
- `crates/fava/tests/semantic_write_contract.rs:93-101` — pure getter round-trip.
- `crates/fava/tests/support/semantic_write_capability_lifecycle.rs:113-122` and `:146-153` — the
  fixture builds both the initial cached source and the successor by invoking the *same*
  `materializer` whose output the test later verifies.

**Confidence.** confirmed.

---

### `provenance-supplied-by-fixture` — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:544` EVENT-003: "An event record MUST
name only relays that actually delivered that exact event occurrence." §6.2: "Have a relay send an
event and EOSE. Do not set internal coverage flags."

**Implementation.** `RelayEvidence::one(...)` is hand-constructed by four fixture helpers and pushed
into the cache with `CacheMutation::Upsert`, then asserted back:

- `crates/fava/tests/support/semantic_write.rs:440-445` `relay_evidence()` — 30+ call sites across
  `semantic_write_capability_protocol.rs`, `semantic_write_capability_lifecycle.rs`,
  `semantic_write_failures/**`, `semantic_write_publication*`.
- `crates/fava/tests/local_source_merge.rs:82-90` `evidence()` — used at 143, 181, 194, 219, 243,
  284, 303, 375.
- `crates/fava/tests/simple_groups.rs:551-556` `evidence()` — used at 106, 156, 213, 254.
- `crates/fava/tests/source_contract.rs:70-76`.

The sharpest instance is `crates/fava/tests/simple_groups.rs:79`
`simple_group_records_require_actual_host_evidence` — the name is EVENT-003's promise. Lines 101-109
write the provenance for hosts `a` and `b` by hand; lines 120-126 assert the record names `a` and `b`
and not `contacted-but-not-serving`. The negative case is proved only by the fixture never having
written it. Compounding: `SpyTransport` (`simple_groups.rs:768`) always refuses and `SpyRouter`
(`:738`) always refuses, so **no relay is contacted anywhere in that 978-line file**.

`crates/fava/tests/local_source_merge.rs:172` `relay_echo_enriches_one_record_without_erasing_receipt`
has the same shape: the "relay echo" is two `cache.commit(Upsert)` calls (179-184, 192-197).

**Contrast that is done right.** `crates/fava/tests/multi_relay.rs:183-227`
`duplicate_event_merges_only_actual_serving_relays` uses three real scripted sessions, has relays 0
and 1 *send* the event and relay 2 send only EOSE, and asserts relay 2 is absent (`:225`). That is
EVENT-003 proved from causes.

**Confidence.** confirmed.

---

### `writes-created-by-hand-writing-the-store` — major — behavioral proof

**Authority.** §6.2: "Create a write through the public acceptance operation. **Do not hand-write
write-store state.**"

**Implementation.** `WriteStore::accept_materialized` / `accept` / `accept_materialized_edit` /
`cancel` called directly in facade tests:
`crates/fava/tests/local_source_merge.rs:111, 148, 158, 176, 261, 384, 387, 390`;
`crates/fava/tests/observation_bounds.rs:70-73` (256 events);
`crates/fava/tests/write_settlement.rs:242-244` (300 events);
`crates/fava/tests/semantic_write_publication/author.rs:70-80` (the persisted author fact that
`recovery_uses_persisted_author_when_only_bob_signer_is_selected` is supposed to prove);
`crates/fava/tests/semantic_write_failures/source_isolation.rs:63-65`;
`crates/fava/tests/support/semantic_write_capability_protocol.rs:285-289`;
`crates/fava/tests/semantic_write_capabilities.rs:169-173`.

**Confidence.** confirmed.

---

### `facade-suite-contains-non-facade-tests` — minor — behavioral proof

**Authority.** §7: "Put proof in the smallest stable component responsible for it." §15: "several
copies of the same test at different layers."

**Implementation.** Eight files under `crates/fava/tests/` never construct a `Fava` (verified by
grepping each for `Fava::builder|fava.observe|fava.publish|\.by\(|\.to\(`):
`write_bounds.rs` (372 lines), `semantic_write_store.rs` (560, doc comment line 1 claims "**Public
contract evidence**"), `semantic_write_store/author.rs`, `semantic_write_store/current_guard.rs`,
`semantic_write_publication/interleavings.rs` (359; parent doc comment claims "**Public-facade
evidence**"), `semantic_write_contract.rs` (141, claims "**Public** neutral-contract evidence"),
`facade_surface.rs`, `source_contract.rs`.

Several are individually rigorous (see Conforming). The finding is that they inflate apparent
facade coverage while their owning crates (`fava-write-store`, `fava-write`) have zero tests.

**Confidence.** confirmed.

---

### `feature-mapping-gate-covers-only-break-free-features` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:242` §5: "Avoid ambiguous `@wip` or untagged prose that
looks built when it is not." §13: a milestone exits only when "no unexecuted scenario appears built."

**Implementation.** Three Python validators exist that assert every feature-to-Rust mapping resolves
to a real cargo test and that steps are observable:
`tools/tests/test_nip02_contact_list_feature.py:9` → `features/nip02-contact-lists.feature`;
`tools/tests/test_publication_door_feature.py:9` → `features/publication-door.feature`;
`tools/tests/test_semantic_write_feature.py:10` → `features/semantic-writes.feature`.

Those three feature files are precisely the three that declare **zero** `fava:falsifier=` lines. The
nine feature files that carry all 41 named deliberate breaks — `explicit-live-query`,
`multi-relay-observation`, `automatic-routing`, `automatic-publication`, `explicit-publication`,
`local-source-merge`, `subscription-planning`, `write-recovery`, `relay-lab` — have **no validator at
all**.

Worse, none of the three validators runs in CI either: `.github/workflows/architecture.yml` names
only `tools/tests/test_vocabulary_check.py`.

**Observable distinction.** A `fava:evidence=` line can name a deleted or renamed test in any of the
nine break-bearing features and nothing notices. (I checked by hand: all 57 currently resolve — see
Conforming — but nothing keeps them resolving.)

**Confidence.** confirmed.

---

### `requirement-ids-are-untraceable` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:456` §13: "Before implementation begins, every
milestone slice must name: requirement/behavior IDs; owner; first failing proof; …".
`FAVA_TDD_BDD_TESTING_GUIDE.md:196` §5 recommends `# requirement: ROUTER-004` in feature headers.

**Implementation.** None of the 121 numbered requirements in
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` (GOAL-001..010, QUERY-001..017 incl. 007A/013A,
EVENT-001..014, WRITE-001..030, RELAY-001..012, ROUTER-001..004, ID-001..008, PROTO-001..010,
OPS-001..011, PROFILE-001..008, OPEN-001..005) appears anywhere in `crates/`, `apps/`, `falsifiers/`,
or `features/`. The feature files use a disjoint namespace (`QUERY-LIVE-001`, `ROUTING-ASYNC-001`,
`WRITE-RECOVERY-001`, …) with no mapping table. Only `features/relay-lab.feature:2` carries a
`# requirement:` line at all, and its value is the prose string "M0 evidence prerequisite".

**Observable distinction.** Coverage cannot be computed. Section 4 of this report had to be produced
by semantic matching.

**Confidence.** confirmed.

---

## Findings, part 2 — canary and owner-crate layers

### `grouping-break-is-inert-production-already-does-it` — critical — behavioral proof

**Authority.** `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1045` RELAY-003 "Subscription grouping
cannot change meaning". `features/subscription-planning.feature:7` names the break: "Discard
wire-to-logical attribution after grouping; logical query results cannot be reconstructed exactly."
Status `built`.

**Implementation.** `SubscriptionPlan.demand: BTreeMap<SubscriptionId, Vec<SubscriptionId>>`
(`crates/fava-subscriptions/src/lib.rs:41`) is the wire→logical attribution map. Fava's only reader
is `validate_plan` at `crates/fava/src/relay.rs:224-228`, which checks
`plan.demand.keys().eq(plan.attribution.keys())` and `!plan.demand.values().any(Vec::is_empty)` —
then `crates/fava/src/relay.rs:211` returns `Ok((session, plan.attribution))` and **the demand map is
dropped**. `handle_message` at `crates/fava/src/relay.rs:269-273` admits every EVENT with
`admit_subscription_event(cache, session.key(), &id, &id, filter, ...)` — the **wire** subscription
id is used as the **logical** id.

Compounding: `StandardSubscriptionPlanner` is installed into exactly **one** `Fava` in the entire
repository — `falsifiers/external-semantic-capability/tests/support/mod.rs:46`, which tests semantic
writes and never groups anything. Every canary assembly wires
`fava_subscriptions_no_grouping::planner()` (`apps/canary/src/live.rs:122`, `multi.rs:266`,
`routing.rs:243`, `publication.rs:394`, `automatic_support.rs:75`, `:91`, `hostile.rs:65`,
`croissant_nip02.rs:318`, `croissant_simple_groups_flow.rs:387`), as does every facade test
(`fava-subscriptions-standard` is not a dev-dependency of `crates/fava`).

**Observable distinction.** Applying the named break changes nothing, because production already
discards the attribution and grouping is never installed. An application that selected the standard
planner would get events attributed to the wrong logical query, and no test in the workspace covers
that path.

**Proposed falsifier.** `grouped_planner_attributes_each_wire_event_to_its_logical_query`: build a
`Fava` with `StandardSubscriptionPlanner`, open three tag-distinct live queries over one scripted
relay, serve one event per tag on the single grouped REQ, and assert each observation sees exactly
its own event.

**Confidence.** confirmed.

---

### `canary-evidence-is-neither-executed-nor-retained` — critical — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:471` §13: a milestone exits only when "the canary
scenario passes where one is required" and "**no unexecuted scenario appears built**."
§14 completion checklist: "Unrun live/platform checks are stated."

**Implementation.** 18 feature scenarios carry `# fava:evidence=canary:<name>` and
`# fava:status=built`. Every one of those scenarios is reachable only from the CLI dispatcher
`apps/canary/src/main.rs:185-236`; grep across `apps/`, `crates/`, and `.github/` finds no other
caller. Of the 19 canary evidence names, exactly one (`slow-consumer-latest-state`,
`apps/canary/src/lib_tests.rs:19`) has a `#[test]` wrapper, and that one is in-memory with no relay.

Retained evidence: `.gitignore:3` ignores `apps/canary/runs/`, and `git ls-files apps/canary/runs`
returns **0 tracked files**. The only bundles on disk are six `phase-07.1.1-pair.*` simple-groups
runs from 2026-08-22/23. There is no artifact showing `subscription-grouping-equivalence`,
`crash-after-acceptance`, `explicit-read-eose`, `reconnect-generation`, or any other M0–M6 scenario
ever ran.

No feature scenario carries an "unrun" note.

**Confidence.** confirmed.

---

### `canary-is-not-a-downstream-application` — major — replaceability / behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:277` §7 lists the Rust canary's job as "ordinary app
usability". §15 anti-pattern: "a canary helper that conceals an awkward or missing public Fava API".
`apps/canary/README.md:3-5` states the canary "must not depend on Fava internal crates or use Fava
diagnostics as the sole witness for external effects."

**Implementation.** `apps/canary/Cargo.toml` declares **31 direct `fava-*` path dependencies**,
including `fava-subscriptions`, `fava-ingest`, `fava-wire`, `fava-transport`, `fava-routing`,
`fava-publisher`, `fava-write-store`, and `fava-state`. That dependency set is what enables every
bypass listed in Deliverable 1: the canary can call the planner, open a transport session, and
perform ingest attribution itself.

The clearest instance of the missing-API pattern: `apps/canary/src/automatic_publication.rs:342-344`
```rust
fn preview_write_routes(routers: &[Arc<dyn Router>], event: EventValue) -> CanaryResult<RoutePlan> {
    fava_routing::preview(routers, &RouteRequest::Write(event)).map_err(error)
}
```
`Fava` exposes only a *read* preview (`crates/fava/src/lib.rs:235` `preview_routes(&self, query: &Query)`).
There is no public write-route preview, so ROUTER-PREVIEW-001's parity assertion
(`apps/canary/src/automatic_publication.rs:269`) compares the receipt against a plan computed by a
function `Fava` does not own, over a **cloned** router vec that is not the engine's chain
(`automatic_publication.rs:113,249,299-300`).

**Confidence.** confirmed.

---

### `tautological-timing-claim` — major — behavioral proof

**Authority.** §9.1 "Control schedules": "Use controlled clocks, barriers, channels, proxy gates, and
witness signals. A longer sleep is not proof."

**Implementation.** `apps/canary/src/automatic_publication.rs:147` `let first_handoff = unix_ms()?;`
and `:150` `let discovery_seeded = unix_ms()?;` are two sequential wall-clock reads in one function
body. `:158` then refuses if `first_handoff > discovery_seeded`, and `:170` reports
`"first_handoff_before_third_list": first_handoff <= discovery_seeded` as the headline fact of the
`async-recipient-routing` scenario. Program order makes this unfailable.

The substantive proof in that scenario is real (`:142-146` asserts relay 3 receives no early EVENT;
`:152-166` asserts expansion under one receipt with no duplicate sends), but the *named* break
(WRITE-AUTOMATIC-001, "the first handoffs cannot precede the final relay-list publication") is
attached to the tautology.

**Confidence.** confirmed.

---

### `cancel-assertion-cannot-fire` — major — behavioral proof

**Authority.** §3.6: "The linked evidence must fail for the claimed reason."
`features/explicit-publication.feature:32` WRITE-CANCEL-001: "Allow signing to continue into
transport after cancellation; the wire records an EVENT and the scenario fails."

**Implementation.** `apps/canary/src/publication.rs:311-320` (`cancel-pre-handoff`) waits for
`signer.calls() == 1`, cancels, `tokio::time::sleep(Duration::from_millis(100))` at `:317`, then
asserts `wire_count(&relay.log, "EVENT")? != 0` is false. The `GatedSigner`
(`apps/canary/src/publication_child.rs:84`) is **never released** in this scenario, so signing can
never complete and **no EVENT can cross the wire under any mutation**. The assertion is structurally
unable to fire. It is also the only bare sleep-as-proof in the canary.

The scenario's real oracles are `wait_empty(&mut observation)` (`:316`) and idempotent removal
(`:321-329`); those are sound.

**Confidence.** confirmed.

---

### `feature-mapping-gate-is-a-name-regex` — major — behavioral proof

*(refines `feature-mapping-gate-covers-only-break-free-features` above)*

**Implementation.** The three validators match a **function name by regex** against the test source:
`RUST_TEST = re.compile(r"^(?:async )?fn (?P<name>...)")`. They never check that the named test
proves the scenario. The suite's own fixture demonstrates the hole:
`tools/tests/test_semantic_write_feature.py:312` writes
`"#[test]\nfn mapped() { assert_eq!(2 + 2, 4); }"` and that satisfies the mapping gate. The step
check is `assertGreaterEqual(len(scenario["steps"]), 3)` — a count gate. `parse_feature` is
copy-pasted verbatim across all three files.

**Confidence.** confirmed.

---

### `evidence-pinned-to-one-machine` — major — behavioral proof

**Authority.** §14 "Fixtures": "Clocks, ports, identities, stores, processes, and teardown are
isolated."

**Implementation.** `apps/canary/src/environment.rs:18,19,20` hard-code
`/Users/pablo/.local/bin/croissant`, `/Users/pablo/Work/croissant`, and
`/Users/pablo/.local/bin/bazelisk`, and `environment.rs:90-107` **asserts those exact absolute
paths**. `apps/canary/src/croissant_simple_groups_tests/public_flow.rs:30` hard-codes the source path
and `:108` shells out to `go build`. The single largest and most rigorous canary scenario
(~5,000 lines, `croissant-simple-groups-public-flow`) can only run on one developer's machine — and
it is referenced by no feature file at all (grep for `GROUP-04`..`GROUP-12` in `features/` → 0 hits).

**Confidence.** confirmed.

---

### `no-generated-input-anywhere` — major — behavioral proof

**Authority.** `FAVA_TDD_BDD_TESTING_GUIDE.md:274` §7: "Property/model/differential — algebra, many
operation orders, invariants across broad input space." §8: "`fava-state` — Use property/model tests
for deduplication, replacement, deletion, expiration, provenance merging, and **order-independence**";
"`fava-query` — Use algebraic and differential tests".

**Implementation.** `grep -rn "proptest\|quickcheck\|arbitrary" --include=Cargo.toml` over the
repository returns **zero hits**. No property-testing dependency exists, so the three §8 mandates
that require generated input are structurally unimplementable today.
`crates/fava-state/tests/event_state.rs:146` `replaceable_winners_are_independent_per_relay_url`
claims order-independence and runs exactly one fixed order. `crates/fava-query/tests/query_identity.rs`
covers identity only; union/intersection/difference and retraction have no tests.

**Confidence.** confirmed.

---

### `fixture-reimplements-the-logic-under-test` — major — behavioral proof

**Authority.** §15: "a mock that implements the behavior under test."

**Implementation.**

- `crates/fava-state/tests/event_state.rs:23-42` — the helper `apply_admission` **reimplements
  mutation application** (upsert-vs-merge, retract-by-id). `admission_mutations` only returns a list;
  the "state" every assertion reads is the test's own `Vec<CachedEvent>`.
- `crates/fava-simple-groups/src/tests/saved.rs:196-214` — the fixture hand-rolls the crate's private
  binary codec: `vec![1]` (`:206`) duplicates the `SAVE_GROUP` opcode (`src/edit.rs:13`); local
  `fn text` (`:197-204`) duplicates `encode_text` (`src/edit.rs:165-176`); `2_u16.to_be_bytes()`
  (`:208`) duplicates `encode_group`'s host framing (`src/edit.rs:145-153`). `decode_edit` has nine
  distinct refusals (`src/edit.rs:178-296`), so any framing drift yields `"truncated saved-list edit"`
  instead of the intended `"duplicate saved-list group host"` (`src/edit.rs:288`) and the test stays
  green with `is_err()`.
- `crates/fava-simple-groups/src/tests/saved.rs:175-182` — the expected value is built with
  `BTreeSet::from([alice, bob]).into_iter().collect()`, the same construction the implementation uses
  at `src/query.rs:165,176`. Compared against itself.
- `crates/fava/tests/support/semantic_write_capability_lifecycle.rs:113-122,146-153` — the fixture
  builds both the initial source and the successor by invoking the same materializer under test.

**Confidence.** confirmed.

---

### `compile-only-public-capstones` — major — behavioral proof

**Authority.** §3.7: "Add the public capstone only when it proves something additional."
§15: "untagged/unmarked scenarios that are not executable but look built."

**Implementation.** Several "external public API" tests bind function pointers and never call them:

- `crates/fava-simple-groups/tests/public_api.rs:230` `readme_facade_flow_compiles_externally`
  assigns four functions to `fn`-pointer variables (`:231-235`);
  `readme_publishes_prepared_unsigned` (`:190`), `readme_publishes_prepared_signed` (`:199`),
  `readme_publishes_saved_edit` (`:208`), and `readme_cancels_and_closes` (`:219`) are **never
  called**. No `Fava` is built; no publish and no `cancel_publication` executes. This is the only
  place in the crate that touches the publish/cancel path.
- `crates/fava-simple-groups/tests/public_api.rs:127` `metadata_parser_accessors_compile_externally`
  contains **no assertion of any kind** (`:129-130`).
- `crates/fava-simple-groups/tests/public_api.rs:147-148` — `let target: Option<PinnedItem> = None;
  assert!(target.is_none());`
- `crates/fava-nip02/tests/public_api.rs:71` — `assert!(decode as usize != 0)` asserts a function
  pointer is non-null.
- `crates/fava-bookmarks/tests/public_api.rs:22` — `assert!(event_functions.len() == 2)` on a literal
  `[EventEdit; 2]`.
- `crates/fava/tests/facade_surface.rs:8` — three `std::mem::size_of` discards and one
  function-pointer coercion; asserts nothing.

**Confidence.** confirmed.

---

### `fake-custody-counter` — major — behavioral proof

**Implementation.** `crates/fava-simple-groups/src/tests/group.rs:285`
`signed_invalid_context_refuses_before_custody`: "custody" is a test-local `AtomicUsize` incremented
by the test's own helper `prepare_then_custody` (`group.rs:273-281`) *after* `group.prepare(event)?`.
The `?` short-circuits, so `assert_eq!(custody_calls, 0)` at `:297` and `:313` is tautologically
equivalent to the `is_err()` the loop at `:294-296` already asserts. Nothing about the real
write-store custody boundary is proved. The genuine proof exists one crate away, at
`crates/fava/tests/simple_groups.rs:430-443`.

**Confidence.** confirmed.

---

### `simulated-cancellation-in-query-standard` — major — behavioral proof

**Authority.** §6.1: "pre-handoff cancellation versus post-handoff uncertainty"; §6.2 causes not
conclusions.

**Implementation.** Three tests in `crates/fava-query-standard/tests/source_merge.rs` name a
cancellation lifecycle and simulate it by re-calling the pure `evaluate()` with a hand-trimmed source
array that omits the write-store snapshot:
`:145` `local_replacement_overlays_then_reveals_cached_predecessor` (trim at `:174-176`),
`:182` `nonmatching_local_replacement_shadows_cached_predecessor_until_cancelled` (trim at `:208-210`),
and `:238`. No cancellation occurs. Relatedly, `SourceStatus::Closed`
(`crates/fava-query/src/lib.rs:54`) is never constructed in any `fava-query-standard` test — every
`SourceSnapshot` is hand-built with `SourceStatus::Open` and `SourceRevision(1)`.

**Confidence.** confirmed.

---

### `in-process-reopen-labelled-process-loss` — major — behavioral proof

**Authority.** §9.2: "Opening a second engine in the same process is not a process-restart proof."

**Implementation.** `crates/fava-write-store-redb/tests/semantic_write_store.rs:451`
`redb_pre_custody_reservation_disappears_on_reopen` asserts at `:472` that "process loss releases
pre-custody reservation" — but it only drops the store object and reopens in the same process. Same
shape at the facade: `crates/fava/tests/semantic_write_failures.rs:378`, `:404`, and
`crates/fava/tests/semantic_write_publication/author.rs:83` construct a second `Fava` in the same
process over the same in-memory store while their names say "recovery".

Relatedly, `crates/fava-write-store-redb/tests/semantic_write_store.rs:182`
`redb_generation_and_failure_state_match_memory` never touches `MemoryWriteStore` — the crate does
not even declare a dependency on `fava-write-store-memory`. The name asserts a differential that is
not run.

**Confidence.** confirmed.

---

### `sealed-evidence-verifiers-accept-self-asserted-booleans` — major — behavioral proof

**Authority.** §9.3: "Diagnostics report what Fava believes. They do not prove their own claims. Use
an independent witness."

**Implementation.**

- `apps/canary/src/croissant_nip02.rs:507-511` writes `"foreign_tags_preserved": true`,
  `"foreign_content_preserved": true`, `"typed_decode_exact": true` as **literals**;
  `validate_manifest` at `:612-624` then requires them to be `true`. The verifier proves only that
  the producer wrote `true`. (The real behavioural check does run in-flow at `:353-390`; it is the
  *verifier* that is circular.)
- `apps/canary/src/croissant_simple_groups_evidence_semantics.rs:107,133` — "claim was not derived
  from flow.json" / "…from process evidence" compare two serializations of one in-memory value
  (`croissant_simple_groups.rs:189` vs `:222-238`). Unable to fail for a real run.
- `apps/canary/src/croissant_simple_groups_flow.rs:271` retains the **expected** route list as
  `shared_evidence`, not the observed one, making `public_flow.rs:64`,
  `evidence_semantics.rs:178`, and `evidence.rs:274` tautological.
- `apps/canary/src/croissant_simple_groups_flow.rs:226` — the core fork-disagreement assertion is
  guarded on `selected_hosts == 2`, so a regression to one host **self-disables** the check.
- Producer and verifier are never connected by a test:
  `apps/canary/src/croissant_simple_groups_tests/public_flow.rs:47,57-58` calls the flow directly,
  bypassing `run_croissant_simple_groups_scenario` (`croissant_simple_groups.rs:131`), and stubs the
  evidence pipeline (`public_flow.rs:36,37,39`: `unused-build-attestation.json`,
  `unused-build-source.manifest`, `unused-retained-root`). Conversely
  `verify_croissant_simple_groups_pair` (`croissant_simple_groups_evidence.rs:33`) is only ever run
  against the hand-typed `PairEvidenceFixture` (`croissant_simple_groups_tests.rs:27`).

**Confidence.** confirmed.

---

## Deliverable 1 — bypass inventory

Every row: a test that claims a public promise but does not drive the assembled public path. Grouped
by bypass kind. All paths absolute-relative to the repository root.

### 1a. Calls a provider, planner, or router directly instead of through `Fava`

| file:line | claims | actually exercises | how a broken impl passes |
|---|---|---|---|
| `apps/canary/src/grouping.rs:69-78,248-252,291-300` | SUBSCRIPTION-GROUPING-001: "grouping changes wire shape without changing logical results" over 300 live queries | 300 hand-built `RelayDemand`s → `planner.plan()` directly (`:248`); `WebSocketTransport::default().open_session()` directly (`:249-252`); the canary performs **ingest attribution itself** via `admit_subscription_event(cache, key, &id, &id, …)` (`:291-300`); the 300 "queries" are `.cache_only()` (`:173-175`); a **second throwaway `Fava`** with no planner and no transport reads the pre-filled cache back (`:348-353`) | `Fava::observe` never runs. Any regression in planner selection, `validate_plan` (`crates/fava/src/relay.rs:224`), subscription allocation (`:214`), attribution enforcement (`:265`), or EOSE/CLOSED routing (`:281-296`) is invisible |
| `crates/fava-subscriptions-standard/tests/grouping.rs:75-110` | RELAY-003 exact grouping of 300 tag queries | `(0..300).map(RelayDemand::new(...))` → `plan()`. Asserts `plan.messages.len()==1` and the 300-value set. No event ever flows | An implementation that groups correctly on the wire but mis-attributes events passes |
| `crates/fava-subscriptions-no-grouping/tests/plan.rs:10` | the ungrouped reference plan | 2 hand-built demands, message-shape assertions. Never compared to the grouped plan | — |
| `apps/canary/src/automatic_publication.rs:342-344,113,249,299-300` | ROUTER-PREVIEW-001 / ROUTER-PROFILE-001 | `fava_routing::preview` on a **cloned** router vec; `Fava` has no public write-route preview at all (`crates/fava/src/lib.rs:235` is read-only) | A broken `Fava` write-preview cannot fail a test of a function `Fava` does not own |
| `crates/fava/tests/automatic_publication.rs:75-96` | partial automatic publication routing "through the public facade" | `fava_routing::preview(&routers, …)` directly at `:75`, then asserts `partial.desired_destinations == preview.destinations.keys()` at `:93-96`. Both sides from the same function over the same routers | Any route-derivation defect affecting both sides identically |
| `crates/fava/tests/semantic_write_capabilities.rs:79-107`; `crates/fava/tests/semantic_write_publication.rs:409-465` | preview matches the initial route | builds a `Publication` provider by hand to reach `preview_semantic_routes` (`crates/fava-publication/src/lib.rs:189` — not on `Fava`); both sides share one `Arc<CountingRouter>` whose contribution is a single constant destination (`crates/fava/tests/support/semantic_write.rs:355-366`) | the assertion reduces to `{relay_url()} == {relay_url()}` |
| `crates/fava-delivery-standard/src/lib.rs:81` | "ambiguous handoff is terminal for the standard policy" | hand-constructs `RelayDeliveryOutcome::Unknown{…}` (`:83-85`) and calls the pure `decide()`. The real chain — `HandoffOutcome::Ambiguous` (`crates/fava-transport-websocket/src/lib.rs:127`) → `PublishOutcome::OutcomeUnknown` (`crates/fava-publisher-nip01/src/lib.rs:54`) → `RelayDeliveryOutcome::Unknown` (`crates/fava-publication/src/delivery.rs:206`) — is untested at every hop | — |
| `crates/fava-diagnostics/tests/relay_facts.rs:17` | QUERY-EVIDENCE-001 "EOSE, CLOSED, AUTH, disconnect and withdrawal remain distinct" | calls `diagnostics.session_opened/eose/closed/authentication_required/failed/withdrawn` directly on the recorder and asserts the snapshot has five named fields | Any behavioral collapse in the engine; the recorder is the only thing under test |

### 1b. Files in a "public facade" suite that never construct a `Fava`

Verified by grepping each for `Fava::builder|fava.observe|fava.publish|\.by\(|\.to\(`:
`crates/fava/tests/write_bounds.rs` (372 lines), `semantic_write_store.rs` (560; doc comment claims
"Public contract evidence"), `semantic_write_store/author.rs`, `semantic_write_store/current_guard.rs`,
`semantic_write_publication/interleavings.rs` (359; parent doc claims "Public-facade evidence"),
`semantic_write_contract.rs` (141; claims "Public neutral-contract evidence"), `facade_surface.rs`,
`source_contract.rs`.

### 1c. Fixture supplies the fact under proof

| file:line | claims | supplied by the fixture |
|---|---|---|
| `crates/fava/tests/simple_groups.rs:79` | EVENT-003 "records require actual host evidence" | lines 101-109 hand-write provenance for hosts `a` and `b`; the negative case is proved only by never writing it. `SpyTransport` (`:768`) and `SpyRouter` (`:738`) always refuse, so **no relay is contacted in that 978-line file** |
| `apps/canary/src/local.rs:197-208,103,136,178,111-114` | M1 local-source-merge scenarios | `RelayEvidence::one(RelaySessionKey::new(parse("wss://m1.local")…))` — a relay session that never existed — inserted via `cache.admit(...)` and then asserted back |
| `apps/canary/src/semantic_writes.rs:145-150,177-182,273-278,296-301,411-416` | M7 semantic write scenarios | `cache.admit(CachedEvent::new(source, relay_evidence()))` with `relay_evidence()` = `wss://m7-semantic.example` (`semantic_write_support.rs:209-218`), a relay the `NoopTransport` (`:61-75`) can never contact. At `:411-416` the canary re-admits **Fava's own output** as the next "source" |
| `crates/fava/tests/write_settlement.rs:28-45` + `:261-324` | "receipt counts preserve complete mixed destination evidence" | the whole `Receipt` is a struct literal; the four assertions count the map the fixture wrote |
| `crates/fava/tests/semantic_write_contract.rs:77-90,93-101,104-141` | first-value materialization, addressable authorship, materialization-vs-receipt identity | all three are struct-literal or getter round-trips; the mock asserts inside its own `materialize` (`:68`) |
| `crates/fava/tests/local_source_merge.rs:172,212` | relay echo enrichment; acquisition-vs-provenance authority | the "relay echo" is two `cache.commit(Upsert)` calls (`:179-184,192-197`); the authority distinction is decided by which `RelayEvidence` the fixture wrote (`:216-221` vs `:240-245`) |
| `crates/fava-subscriptions-standard/tests/grouping.rs:184,209` | RELAY-004 exact relay-limit shortfall | the limit is hand-passed to `StandardSubscriptionPlanner::bounded(...)`; NIP-11 does not exist |
| `crates/fava/tests/write_bounds.rs:144-176` | automatic route fan-out bounded before receipt mutation | a `RoutePlan` with 257 destinations is hand-built and handed to `MemoryWriteStore::apply_route`. No router in the workspace can produce 257 destinations |

### 1d. Writes created by hand-writing the write store (§6.2 forbids it)

`crates/fava/tests/local_source_merge.rs:111,148,158,176,261,384,387,390`;
`crates/fava/tests/observation_bounds.rs:70-73` (256 events);
`crates/fava/tests/write_settlement.rs:242-244` (300 events);
`crates/fava/tests/semantic_write_publication/author.rs:70-80` (the persisted-author fact under proof);
`crates/fava/tests/semantic_write_failures/source_isolation.rs:63-65`;
`crates/fava/tests/support/semantic_write_capability_protocol.rs:285-289`;
`crates/fava/tests/semantic_write_capabilities.rs:169-173`;
`apps/canary/src/local.rs:66-68,99,146`.

### 1e. Asserts on a double's private state instead of a public observable

`crates/fava/tests/semantic_write_capabilities.rs:97-98` and
`crates/fava/tests/semantic_write_publication.rs:436-437` (`router.previews()`, `router.opens()`);
`apps/canary/src/routing.rs:187` (`delayed.open_count()` is the sole discriminator for
ROUTING-EXPLICIT-001 because the router points at the *same* relay the explicit query targets, so the
wire proxy cannot distinguish — `routing.rs:52,177`);
`apps/canary/src/semantic_writes.rs:351-370` (`CompletionAck.installed`, an internal `WriteStore` call
outcome, used as the oracle for a public promise);
`crates/fava/tests/observation_bounds.rs:80` and `apps/canary/src/local.rs:82-85`
(`diagnostics().coalesced_query_updates > 0`).

### 1f. Assertion that cannot fail

`crates/fava/tests/observation_bounds.rs:37,47` (thread id under `current_thread` flavour);
`crates/fava/tests/simple_groups/saved.rs:166` (empty `assert_ordinary_write`);
`apps/canary/src/automatic_publication.rs:158` (two sequential `unix_ms()`);
`apps/canary/src/publication.rs:318` (no EVENT possible while the signer is gated shut);
`apps/canary/src/automatic_publication.rs:316-324` (two differently-configured router lists differ);
`apps/canary/src/croissant_simple_groups_evidence.rs:358` / `public_flow.rs:97` (`pid != 75_649`);
`crates/fava-simple-groups/src/tests/group.rs:297,313` (fake custody counter);
`crates/fava-nip02/tests/public_api.rs:71` (`decode as usize != 0`);
`crates/fava-bookmarks/tests/public_api.rs:22` (array-literal length);
`crates/fava-simple-groups/tests/public_api.rs:127,147-148`;
`crates/fava/tests/facade_surface.rs:8`;
self-comparison tautologies at `crates/fava-simple-groups/src/tests/saved.rs:76-78,179-182`,
`snapshot.rs:133-135`, `architecture.rs:226-232`.

### 1g. Source-text grep presented as behavioral proof

`crates/fava/tests/facade_surface.rs:20,66`;
`crates/fava-simple-groups/tests/architecture.rs:161,182,240,288,337`;
`crates/fava-nip02/tests/architecture.rs:51,120`;
`crates/fava-routing/src/chain.rs:446`;
`crates/fava-write-store-redb/tests/semantic_write_store.rs:29`;
`crates/fava-nip02/src/tests/edit.rs:316-340`;
`apps/canary/src/semantic_n_plus_one.rs:81` (`stdout.contains("external-semantic-capability")`);
`apps/canary/src/croissant_simple_groups_wire.rs:80-88` (substring count becomes `facts.handoffs`,
asserted at `public_flow.rs:80` and `evidence.rs:284`).

---

## Deliverable 2 — provider test-double inventory

Every `impl <ProviderTrait> for <T>` in test/testkit code was read. Legend: ✓ can express the mode;
— cannot.

### Transports

| Double (path:line) | pending | fail-mid | cancel-mid | stale/late | unbounded/slow | panic |
|---|---|---|---|---|---|---|
| `crates/fava/tests/explicit_live.rs:57` `PendingTransport` *(uncommitted)* | ✓ | — | — | — | — | — |
| `crates/fava/tests/explicit_live.rs:78` `FirstOpenThenPendingTransport` *(uncommitted)* | ✓ | — | ✓ | — | — | — |
| `crates/fava/tests/explicit_live.rs:107` `ScriptedTransport` | — | ✓ | ✓ | — `generation()` returns constant `7` (`:141`) | capable, never driven | — |
| `crates/fava/tests/multi_relay.rs:50` `ScriptedTransport` | — | ✓ | ✓ | ✓ old-subscription frame on new generation (`:258-271`) | capable, never driven | — |
| `crates/fava/tests/automatic_routes.rs:61` `RecordingTransport` | quiet peer only (`:120` `pending()`) | — | ✓ | — | — | — |
| `crates/fava/tests/simple_groups.rs:773` `SpyTransport` | — | — | — | — | — | — |
| `NoopTransport` ×6 (`automatic_publication.rs:206`, `explicit_publication.rs:271`, `write_settlement.rs:474`, `support/semantic_write.rs:411`, `fava-write-store-redb/tests/process_kill/semantic.rs:428`, `apps/canary/src/semantic_write_support.rs:63`) | — | — | — | — | — | — |
| `falsifiers/external-semantic-capability/tests/support/mod.rs:280` `ScriptedTransport` | — (2 s `DEADLINE` at `:24`) | ✓ | — | ✓ | — | — |

No transport double produces `HandoffOutcome::Ambiguous` or `TransportError::InvalidFrame`; no
duplicate or reordered inbound frames; no slow/backpressured peer; no reconnect during an active
write.

### Signers

| Double | pending | fail-mid | cancel-mid | stale/late | unbounded | panic |
|---|---|---|---|---|---|---|
| `crates/fava/tests/support/semantic_write.rs:284` `BlockingSigner` | ✓ | — | ✓ | — | — | — |
| `crates/fava/tests/simple_groups.rs:690` `BlockingSigner` | ✓ | — | ✓ | — | — | — |
| `crates/fava/tests/explicit_publication.rs:384` `BlockingSigner` | ✓ | — | ✓ | — | — | — |
| `crates/fava/tests/support/semantic_write_capability_signer.rs:28` `GatedSigner` | ✓ | ✓ | — (ignores `_cancel`) | ✓ gen-1 completion after gen-2 exists | — | — |
| `apps/canary/src/publication_child.rs:115` `GatedSigner` | ✓ | — | ✓ | — | — | — |
| `apps/canary/src/semantic_write_support.rs:110` `GateSigner` | ✓ | ✓ | — | ✓ | — | — |
| `crates/fava/tests/support/semantic_write.rs:247` `CountingSigner` | — | — | — | — | — | — |
| `crates/fava/tests/simple_groups.rs:649` `ExactSigner` | — | — | — | — | — | — |
| `apps/canary/src/semantic_write_support.rs:148` `DeterministicSigner` | — | — | — | — | — | — |

`Signer` is the only contract whose trait carries an explicit `cancel: watch::Receiver<bool>`, which
is why it is the only contract where cancellation is observable at all. No signer returns a
spontaneous `SignerError::Refused`/`InvalidOutput` mid-flight through the public path.

### Publishers

| Double | pending | fail-mid | cancel-mid | stale/late | unbounded | panic |
|---|---|---|---|---|---|---|
| `crates/fava/tests/write_settlement.rs:432` `ManualPublisher`/`ManualLane` | ✓ | ✓ | — | ✓ per-lane out-of-order release | ✓ | — |
| `crates/fava/tests/explicit_publication.rs:310` `GatedPublisher` | ✓ | — single fixed outcome | — | — | — | — |
| `crates/fava/tests/explicit_publication.rs:338` `OutcomePublisher` | — | ✓ keyed by relay name | — | — | — | — |
| `apps/canary/src/semantic_delivery_support.rs:43` `GatePublisher` | ✓ | — | — | ✓ | — | — |
| `crates/fava/tests/support/semantic_write.rs:317` `RecordingPublisher` | — always `Acknowledged{"stored"}` | — | — | — | — | — |
| `crates/fava/tests/automatic_publication.rs:189` `RecordingPublisher` | — | — | — | — | — | — |
| `apps/canary/src/semantic_write_support.rs:43` `RecordingPublisher` | — | — | — | — | — | — |
| `crates/fava/tests/simple_groups.rs:723` `SpyPublisher` | — | — | — | — | — | — |
| `crates/fava-write-store-redb/tests/process_kill/semantic.rs:412` `AcknowledgingPublisher` | — | — | — | — | — | — |

The always-Acknowledged publishers back **every** semantic-write test. No retry ladder, no give-up
bound, no ambiguous handoff anywhere in that suite.

### Routers

| Double | pending | fail-mid | cancel-mid | stale/late | unbounded | panic |
|---|---|---|---|---|---|---|
| `crates/fava-router-testkit/src/lib.rs:44` `DelayedRouter` | — (`preview`/`open` are synchronous and always succeed) | — | — | ✓ contribution replaced over a `watch` | — | — |
| `crates/fava/tests/semantic_write_failures/route_revision.rs:122` `QueuedRouter` | — | — | — | ✓ revision across a materialization boundary | broadcast cap 8, never driven | — |
| `crates/fava/tests/support/semantic_write.rs:369` `CountingRouter` | — (`next_change` = `pending()`) | — | — | — | — | — |
| `crates/fava/tests/simple_groups.rs:743` `SpyRouter` | — | always refuses; asserted never called | — | — | — | — |

`DelayedRouter` never delays, never refuses, never fails. No router anywhere returns a `RouterError`
mid-session through the public path, and no router produces a `shortfall` — `route_shortfalls` is
asserted non-empty only at `crates/fava/tests/write_bounds.rs:371` from a store-direct `apply_route`.

### Write stores / caches / evaluators / materializers

| Double | pending | fail-mid | cancel-mid | stale/late | unbounded | panic |
|---|---|---|---|---|---|---|
| `crates/fava/tests/semantic_write_failures/faults.rs:192` `FaultingWriteStore` | ✓ barrier pause inside `apply_route` | ✓ succeed-then-fail after signature and after route | — | ✓ | ✓ dropped change stream | — |
| `crates/fava/tests/semantic_write_failures/faults.rs:81` `ClosingEventCache` | — | ✓ kills live observations mid-stream | — | — | — | — |
| `crates/fava/tests/support/semantic_write_capability_lifecycle.rs:290` `CompletionStore` | — | — pass-through witness | — | observes only | — | — |
| `apps/canary/src/semantic_write_store.rs:51` `CompletionStore` | — | — | — | observes only | — | — |
| `falsifiers/external-null-cache/src/lib.rs:14` `NullEventCache` | ✓ permanently silent source | — | — | — | — | — |
| `crates/fava-observe/src/lib.rs:291` `TrackingSource` / `:323` `RefusingSource` | — | ✓ open refusal | — | — | — | — |
| `crates/fava-observe/src/lib.rs:333` `EmptyEvaluator` / `:345` `FailingEvaluator` | — | ✓ refusal | — | — | — | — |
| `crates/fava-router-outbox/tests/outbox.rs:141` `WatchSource` | — | — | — | — | — | — |
| `crates/fava/tests/semantic_write_failures.rs:71` `ControlledMaterializer` | — | ✓ | — | — | ✓ 8 KiB error, 140 KB content | ✓ **the only panicking double in the workspace** |
| `crates/fava/tests/support/semantic_write.rs:187` `TestMaterializer` | — | — | — | — | — | — |
| `crates/fava/tests/semantic_write_contract.rs:52` `ExactMaterializer` | — | — | — | — | — | — |

`FaultingWriteStore` is the strongest double in the corpus — but it is used **only** inside
`crates/fava/tests/semantic_write_failures/**`. None of `explicit_publication.rs`,
`write_settlement.rs`, `automatic_publication.rs`, or `simple_groups.rs` ever injects a store fault.

### Summary of the double inventory

- **Contracts with no adversarial double at all:** `DeliveryPolicy` (and only one implementation
  workspace-wide), `SubscriptionPlanner`, `QueryEvaluator` (two stubs confined to `fava-observe`),
  `Router` (nothing blocks, refuses mid-session, or panics), `EventCache` (nothing blocks or panics).
- **Cancellation observation exists for exactly one contract** (`Signer`), because it is the only
  trait that passes a cancel channel. There is not a single `impl Drop` on any test double in
  `crates/*/tests`, `apps/canary/src`, or `falsifiers/` — so an abandoned `Transport::open_session`,
  `Publisher::publish`, or `WriteStore` call is unobservable.
- **Panic is expressible for exactly one contract** (`ReplaceableEventMaterializer`).
- **No provider double can fail during shutdown**, because there is no `Fava::shutdown` (the public
  surface at `crates/fava/src/lib.rs:98-259` has no such method).
- **Backpressure evidence** exists in exactly one place — `crates/fava/tests/write_bounds.rs:18`
  asserts `RecvError::Lagged(1)` exactly — and it is generated by the real `MemoryWriteStore`, not by
  a double.

---

## Deliverable 3 — every named deliberate break, judged

`features/` declares 41 breaks via `# fava:falsifier=`. `docs/issues/` records 10 executions, of
which **5 correspond to a named feature break** and 5 do not. Judgement key: **M** = meaningful (a
mistake that would plausibly ship, and whose only detection is the linked evidence); **S** = straw
(trivially detectable, redundantly covered, a compile-shape change, or so drastic that everything
fails); **U** = undetectable (the linked evidence would stay green under the break).

| # | Behavior ID | feature:line | Named break | Executed? | Judgement |
|---|---|---|---|---|---|
| 1 | WRITE-AUTOMATIC-001 | automatic-publication:7 | Wait for every unresolved recipient before starting a lane | ✓ `docs/issues/0008:51` | **M** — the strongest executed break; a real ordering bug, detected by an ordering witness |
| 2 | ROUTER-OUTBOX-001 | automatic-publication:22 | Route the missing relay-list Query automatically; router recursion appears | ✗ | **M** — and urgent: `crates/fava/src/query_source.rs:13` `impl QuerySource for Fava` already starts a recursive `Fava::observe`, so this break may already be shipped |
| 3 | ROUTER-HINT-001 | automatic-publication:35 | Require outbox knowledge for referenced events | ✗ | M |
| 4 | ROUTER-PREVIEW-001 | automatic-publication:47 | Open live router acquisition during preview | ✗ | M — but detection is via `DelayedRouter::open_count()` (`crates/fava-router-testkit/src/lib.rs:38`), a double's private counter |
| 5 | ROUTER-PROFILE-001 | automatic-publication:59 | Move app-relay or fallback choice into core | ✗ | **S** — a refactor, not a defect; detection ("the same assembly selection can no longer produce two plans") is structural, and `fava-router-app-relays`/`fava-router-fallback-relays` have zero tests |
| 6 | ROUTING-ASYNC-001 | automatic-routing:7 | Await a later contribution before using the immediate plan | ✓ `docs/issues/0006:47` | **M, but narrow** — it detects *router* await only. The confirmed crisis bug (`Fava::observe` blocking on relay establishment) passes it, because the 100 ms deadline at `crates/fava/tests/automatic_routes.rs:160` is measured with `RecordingTransport`, whose `open_session` succeeds instantly |
| 7 | ROUTING-EXPLICIT-001 | automatic-routing:23 | Open the configured router chain for an explicit Query | ✗ | M |
| 8 | ROUTING-FALLBACK-001 | automatic-routing:35 | Freeze fallback at its initial contribution | ✗ | M |
| 9 | ROUTING-PREVIEW-001 | automatic-routing:48 | Implement preview by opening live routing | ✗ | M (duplicate of #4) |
| 10 | ROUTING-ATTRIBUTION-001 | automatic-routing:58 | Replace the destination map entry on duplicate relay identity; one router reason disappears | ✗ | **U** — linked evidence `crates/fava/tests/automatic_routes.rs:187` asserts only `planned.reasons.len() == 2`, never *which* reasons. Recording the same reason twice passes |
| 11 | QUERY-LIVE-001 | explicit-live-query:7 | Treat silence or a local timeout as EOSE | ✗ (`docs/issues/0004` ran a different break) | M |
| 12 | QUERY-LIVE-002 | explicit-live-query:19 | Terminate relay demand at EOSE | ✗ | M — canary-only evidence, so it needs a real relay and never runs in CI |
| 13 | QUERY-LIVE-003 | explicit-live-query:30 | Drop the cancellation branch before sending CLOSE | ✗ | M |
| 14 | INGEST-001 | explicit-live-query:43 | Bypass event verification throughout relay admission | ✓ `docs/issues/0004:50` | **S** — three owners disabled at once; already covered three times over (`crates/fava-event-cache-memory/src/lib.rs:203`, `crates/fava-ingest/tests/admission.rs:20`, `crates/fava/tests/explicit_live.rs:314`). This is the break the audit brief named as the archetype |
| 15 | QUERY-EVIDENCE-001 | explicit-live-query:55 | Store EOSE, CLOSED, AUTH, and disconnect in one terminal flag | ✗ | **S** — a struct-shape change that would not compile against `crates/fava-diagnostics/tests/relay_facts.rs:34-40`, which reads five named snapshot fields |
| 16 | TRANSPORT-001 | explicit-live-query:64 | Report an oversized rejected frame as handed off | ✗ | **S** — `crates/fava-transport-websocket/tests/conformance.rs:47` asserts exactly that branch |
| 17 | WRITE-EXPLICIT-001 | explicit-publication:7 | Insert the unsigned event into EventCache at acceptance | ✗ | **M** — `AGENTS.md:71` forbids exactly this; a real bug class |
| 18 | WRITE-EXPLICIT-002 | explicit-publication:20 | Collapse acknowledged, rejected, and definite pre-handoff failure into one success flag | ✗ | **S/U** — an enum collapse would not compile; and the linked evidence `crates/fava/tests/explicit_publication.rs:131,135,139` uses `.any(...)`, so it never binds an outcome to a relay |
| 19 | WRITE-CANCEL-001 | explicit-publication:32 | Allow signing to continue into transport after cancellation | ✗ | M |
| 20 | WRITE-SIGNER-001 | explicit-publication:44 | Discard an accepted unsigned write when its exact author signer is absent | ✗ | M |
| 21 | WRITE-BOUNDS-001 | explicit-publication:56 | Accept 257 explicit relays or 4097 bytes of receipt text | ✗ | **S** — off-by-one on constants the tests hardcode (`crates/fava/tests/write_bounds.rs:38`, `:130`) |
| 22 | QUERY-LOCAL-001 | local-source-merge:6 | Ignore WriteStore source contributions | ✗ | **S** — deleting an entire query source fails most of the suite |
| 23 | QUERY-LOCAL-002 | local-source-merge:17 | Discard relay evidence while merging a cached and local event id | ✗ | M |
| 24 | QUERY-LOCAL-003 | local-source-merge:28 | Retain the older candidate when a local replacement is newer | ✗ | M |
| 25 | QUERY-SOURCE-001 | local-source-merge:40 | Treat `from_relays` as a result-provenance constraint | ✗ | M — but the linked evidence `crates/fava/tests/local_source_merge.rs:212` decides the distinction purely by which `RelayEvidence` the fixture wrote (lines 216-221 vs 240-245) |
| 26 | QUERY-SOURCE-002 | local-source-merge:50 | Ignore `OnlyRelays` result authority | ✗ | M, same fixture caveat |
| 27 | QUERY-OPEN-001 | local-source-merge:61 | Return an open error without explicitly closing provisional source observations | ✗ | **M and now known-violated** — this is the crisis's partial-open leak. The linked evidence (`crates/fava-observe/src/lib.rs:356`, `:377`) covers only the two *local* sources; relay partial-open is proved only by the uncommitted, currently-red `crates/fava/tests/explicit_live.rs:236` |
| 28 | QUERY-SOURCE-003 | local-source-merge:71 | Close the whole query when one local source terminates | ✗ | M |
| 29 | QUERY-DELIVERY-001 | local-source-merge:81 | Deliver queued intermediate snapshots instead of the coalesced latest state | ✗ | **U** — linked evidence `crates/fava/tests/local_source_merge.rs:250` has no slow consumer and no bound; three writes then one snapshot asserting `events.len() == 3`. It passes whether the updates coalesced or not |
| 30 | QUERY-IDENTITY-001 | local-source-merge:92 | Preserve relay insertion order in Query equality or hashing | ✗ | M (narrow, well matched to `crates/fava-query/tests/query_identity.rs`) |
| 31 | EVENT-STATE-001 | local-source-merge:104 | Apply deletion requests without author validation | ✗ | M |
| 32 | PROVIDER-SOURCE-001 | local-source-merge:117 | Stop either memory provider from emitting removals | ✗ | M |
| 33 | EVENT-CACHE-001 | local-source-merge:126 | Accept an upsert without verifying its event ID and signature | ✗ | **S** — same mechanism as #14, third copy |
| 34 | QUERY-MULTI-001 | multi-relay-observation:7 | Credit every relay named by the Query | ✗ | **M** — and the linked evidence (`crates/fava/tests/multi_relay.rs:183-227`) is genuinely capable of detecting it. Should be executed; it is the cheapest win in the table |
| 35 | QUERY-RECONNECT-001 | multi-relay-observation:20 | Accept an EVENT using any known filter instead of the current subscription attribution | ✓ `docs/issues/0005:47` | **M — the best break in the repository.** A single-predicate change, causally detected by an exact-message assertion at `crates/fava/tests/multi_relay.rs:262-271` |
| 36 | OBSERVATION-BOUNDS-001 | multi-relay-observation:34 | Replace the watch boundary with an unbounded update queue | ✗ | M — plausibly detectable by `crates/fava/tests/observation_bounds.rs:75-79`, but nothing asserts a memory bound, only `latest.events.len() == 256` |
| 37 | OBSERVATION-BOUNDS-002 | multi-relay-observation:46 | Assign a dedicated OS thread per Observation | ✗ | **U** — see finding `vacuous-thread-assertion`. The assertion cannot fail |
| 38 | SUBSCRIPTION-GROUPING-001 | subscription-planning:7 | Discard wire-to-logical attribution after grouping | ✗ (`docs/issues/0018:72` ran a case-fold break instead) | **M** — and unprovable today: no differential exists (see `grouping-differential-absent`) |
| 39 | SUBSCRIPTION-SHORTFALL-001 | subscription-planning:18 | Drop demand beyond the relay subscription limit | ✗ | M |
| 40 | WRITE-RECOVERY-001 | write-recovery:7 | Return acceptance without committing its receipt | ✓ `docs/issues/0007:46` | **M** — real SIGKILL witness; the second-best executed break. Weakened only because the child never uses the public path |
| 41 | WRITE-RECOVERY-002 | write-recovery:19 | Recover an in-flight attempt as definitely not handed off | ✗ | **M** — the ambiguity-preservation promise (WRITE-020) with no evidence at all |

### Executed breaks not named by any feature scenario

| Break | Recorded at | Target evidence | Judgement |
|---|---|---|---|
| Emit the merged event twice | `docs/issues/0001:47` | `crates/fava-query-standard/tests/source_merge.rs:120` — **not linked from any feature** | M (weak) — duplicate records are trivially visible, but same-id merge is a real bug class |
| `DELIBERATE_BREAK_M7_STALE_COMPLETION` | `docs/issues/0010` | `crates/fava/tests/semantic_write_publication/interleavings.rs:94` | **M — the most rigorously executed break in the repo.** One predicate removed from `fava-write-store::validate_current_materialization`, SHA-256 recorded before and after, control tests confirmed still green, causal failure located to an exact line. This is the template the other 40 should follow. Caveat: the owning crate `fava-write-store` has zero tests, so detection was two layers away |
| `DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY` | `docs/issues/0010` | `cargo check -p fava-nip02` | **S** — an inserted `use fava_signer as _;` producing E0432. A compile error about an absent crate, not behavior |
| `DELIBERATE_BREAK_M7_EVENT_BUILDER_BOUND` | `docs/issues/0010` | 2-test bound target | **S** — `MAX_TAGS` 2000 → 2001 against a test that hardcodes the boundary |
| Case-folded grouping axis | `docs/issues/0018:72` | `crates/fava-subscriptions-standard/tests/grouping.rs:112` + canary preflight | **M** — a genuine merge-compatibility mistake, causally detected in two independent places, with SHA-256 before/after and controls. Second-best executed break |

### Tally

- 41 named breaks; **5 executed as named** (#1, #6, #14, #35, #40), of which one (#14) is straw.
- **36 named breaks never executed.**
- Judged **meaningful: 29**; **straw: 8** (#5, #14, #15, #16, #18, #21, #22, #33); **undetectable by
  their own linked evidence: 4** (#10, #29, #37, and #18 in its second aspect).
- Five additional breaks were executed against evidence that no feature scenario names, so they do
  not discharge any `fava:status=built` claim.

---

## Deliverable 4 — requirement coverage

The spec has **131** `## <ID>` requirements (GOAL 10, QUERY 19 incl. 007A/013A, EVENT 14, WRITE 30,
RELAY 12, ROUTER 4, ID 8, PROTO 10, OPS 11, PROFILE 8, OPEN 5). None of the IDs appears anywhere in
`crates/`, `apps/`, `falsifiers/`, or `features/` (see `requirement-ids-are-untraceable`), so this
mapping is semantic and was produced by reading test bodies.

Scoring rule applied: only a named Rust `#[test]`/`#[tokio::test]` can score **PROVEN**. Canary-only
evidence caps a requirement at **WEAK**, because `apps/canary/Cargo.toml:8` and both
`falsifiers/*/Cargo.toml:1` declare their own `[workspace]` (so `cargo test --workspace` cannot reach
them), no `BUILD.bazel` exists under `apps/` or `falsifiers/`, and of the 19 `canary:` evidence names
referenced by `features/`, exactly one (`slow-consumer-latest-state`,
`apps/canary/src/lib_tests.rs:19`) has a `#[test]` wrapper — and that one is in-memory with no relay.

| Verdict | Count | Share |
|---|---|---|
| PROVEN | 56 | 43% |
| WEAK / non-distinguishing | 39 | 30% |
| NO EVIDENCE | 36 | 27% |

By family (proven / weak / none):

| Family | proven | weak | none |
|---|---|---|---|
| GOAL (10) | 2 | 7 | 1 |
| QUERY (19) | 9 | 5 | 5 |
| EVENT (14) | 7 | 2 | 5 |
| WRITE (30) | 21 | 7 | 2 |
| RELAY (12) | 6 | 3 | 3 |
| ROUTER (4) | 4 | 0 | 0 |
| ID (8) | 1 | 3 | 4 |
| PROTO (10) | 5 | 0 | 5 |
| **OPS (11)** | **0** | 5 | 6 |
| PROFILE (8) | 1 | 4 | 3 |
| OPEN (5) | 0 | 3 | 2 |

### C — requirements with NO falsifying evidence (36)

Each was confirmed by a grep that actually ran across `crates/ apps/ falsifiers/`.

| ID | Grep | Result |
|---|---|---|
| GOAL-009 | `conformance\|corpus` in `crates/*/src` | 2 hits, both module doc comments. No public conformance kit ships for event cache, write store, evaluator, router, planner, publisher, delivery policy, signer, or service. The only shared corpus is private (`crates/fava/tests/support/`, `crates/fava/tests/source_contract.rs`) |
| QUERY-007 | `nested\|derived\|union\|intersection\|difference` in `fava-query/src` | 0 — no nested-query construct exists |
| QUERY-008 | same | 0 — no combined/union query exists |
| QUERY-013A | `freshness` | `Freshness::{CacheOnly, Live}` exists; no test asserts one query's freshness decision does not perturb another's relay work |
| QUERY-016 | `\bsince\b\|\buntil\b` | 3 hits, all English prose. `FilterSelection` (`crates/fava-query/src/selection.rs`) has no `since`/`until`; app time windows are inexpressible |
| QUERY-017 | `window\|Window` | 0 |
| EVENT-004 | `restart\|reopen` | 62 hits, all write-store. No persistent event cache, no event-cache restart corpus |
| EVENT-005 | `evict\|Evict` | 6 hits, all redb terminal-receipt retention. `MemoryEventCache` *refuses* at capacity (`crates/fava-event-cache-memory/src/lib.rs:84`) rather than evicting; no evicting cache exists |
| EVENT-007 | `coverage_progress\|watermark\|progress` | 3 unrelated hits. No source-scoped coverage concept |
| EVENT-010 | `nip11\|nip05` | 0 |
| EVENT-012 | `destructive_reset\|reset_all` | 0 |
| WRITE-009 | `sign_only\|sign_without\|SignOnly` | 0 — no sign-without-publish operation on the facade |
| WRITE-029 | read of all 5 redb test files | No test issues a command concurrently with `open()` and asserts deferral/refusal; the 256-generation loop at `crates/fava-write-store-redb/tests/semantic_write_store.rs:297` is never followed by a reopen, so bounded-supersession recovery is untested |
| RELAY-007 | `nip42\|Nip42\|NIP-42` | 1 hit: `apps/canary/src/relay.rs:223 nip42_auth = false`. See `nip42-unproven-and-contradicted` |
| RELAY-009 | `nip11` | 0 |
| RELAY-010 | `nip05` | 0 |
| ID-001 | `struct Account\|enum Account\|AccountId\|Session\b` | No account/session type; all `Session` hits are `RelaySession`/`RouterSession` |
| ID-002 | same | No current-account input; `Query` has no account axis |
| ID-005 | `restore_session\|SessionRestore` | 0 |
| ID-007 | `nip44\|nip04\|encrypt\|decrypt` | 8 hits, all string literals in fixtures. `Signer` has `sign_event` only |
| PROTO-005 | `reaction\|repost\|quote\|nip18\|nip22\|nip25` | 2 hits, both inside hostile-string fixtures |
| PROTO-007 | same | No NIP-25 crate |
| PROTO-008 | `nip09\|EventDeletion` | 6 hits, all *ingestion* of kind 5. Nothing publishes a deletion as a write; nothing asserts deletion ≠ cancellation |
| PROTO-009 | `content_parse\|ContentParser` | 0 |
| PROTO-010 | `ls docs/` | No inventory document classifying protocols required/optional/deferred/app-owned |
| OPS-003 | `stalled\|stuck\|Unroutable\|Unsignable\|Undeliverable` | 0 real hits. `Fava::open_receipts()` returns an unclassified `Vec<Receipt>` |
| OPS-006 | `swift\|kotlin\|uniffi` repo-wide | 1 hit, in `MODULE.bazel.lock`. No bindings |
| OPS-007 | `find . -name '*.swift' -o -name '*.kt'` | 0 files |
| OPS-008 | same | No native artifact |
| OPS-010 | same | No device harness |
| OPS-011 | `find . -name benches` | 0 |
| PROFILE-001 | `ls docs/spec docs/internals` | No profile document exists |
| PROFILE-002 | persistent-cache grep | No persistent event cache implementation |
| PROFILE-007 | `pub fn` in `crates/fava/src/lib.rs` | `builder()` requires explicit selection, but no named recommended assembly and no document |
| OPEN-001 | `window\|cursor\|Paginat` | 0 (decision correctly unresolved) |
| OPEN-005 | as PROFILE-002 | No persistent cache to recommend |

### B — evidence that could not distinguish a correct implementation from the current one (39)

The full 39-row table is long; these are the ones where the gap is most consequential, each with the
specific unproven clause:

| ID | Evidence | Unproven clause |
|---|---|---|
| WRITE-020 | `crates/fava/tests/write_settlement.rs:102`; `crates/fava-write-store-redb/tests/process_kill.rs:91` | The `OutcomeUnknown` is **supplied by the test publisher** — the fixture supplies the fact under proof. The redb "attempt" boundary asserts via `receipt.destinations().values().all(...)`, which is **vacuously true on an empty destination map** (no `assert_eq!(destinations().len(), 1)`). `WebSocketTransport` only ever returns `HandedOff`/`NotHandedOff`, so Fava never *derives* ambiguity |
| WRITE-019 | `crates/fava-delivery-standard/src/lib.rs:57` | A pure-function test on `DeliveryPolicy::decide` in isolation. Nothing drives repeated real attempts to a `GivenUp`. "Time offline / awaiting routing / awaiting signing MUST NOT count as a failed attempt" and shared per-relay backoff are unproven |
| WRITE-022 | `crates/fava/tests/semantic_write_publication/interleavings.rs:46` | Destination union is implemented (`crates/fava-write-store-memory/src/semantic.rs:296`) but **no test asserts a corrected successor retains a predecessor destination** |
| WRITE-024 | `crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs:196` | **Paging has no API and no test**; `schema::load` reads every row at open, contradicting "without loading all history" unchecked |
| WRITE-026 | `crates/fava/tests/explicit_publication.rs:111` | That test opens **no observation**, so "the event remains in the local query after every destination rejects it" is asserted nowhere |
| WRITE-030 | `crates/fava/tests/publication_door.rs:120` | Only the unsigned form is submitted. The pre-signed guard (`crates/fava-write/src/lib.rs:117`) is untested and the replaceable-edit path has **no expiry guard at all** |
| RELAY-004 | `crates/fava-subscriptions-standard/tests/grouping.rs:184,209` | The limit is hand-passed to `StandardSubscriptionPlanner::bounded(...)` — the fixture supplies it. NIP-11 does not exist, so advertised limits, message length, subscription-id length, filter limits, event size, and PoW have zero evidence |
| RELAY-012 | `crates/fava/tests/explicit_live.rs:273,342`; `multi_relay.rs:230` | 4 of 12 named hostile behaviours covered. Uncovered: stall/never-EOSE, silent subscription cap, mid-stream auth challenge, EOSE-then-more-events, truncated frames, injected raw bytes, ack-without-serving, disconnect-after-handoff |
| QUERY-012 | `crates/fava/tests/observation_bounds.rs:51` | The surface is a `tokio::watch` (`crates/fava-observe/src/lib.rs:191`), so *second concurrent pull refused without consuming data*, *delivered-once-never-again*, *invalid acknowledge/cancel ordering refused*, and *shutdown ends all pending pulls* are neither implemented nor tested |
| OPS-002 | `crates/fava/tests/observation_bounds.rs:80` | The only assertion is `coalesced_query_updates > 0` — Fava's own counter, and `> 0` is non-distinguishing |
| OPS-004 | 8 sites | No bound at all on active relay sessions, engine-side provider operations, fetched service entries, or platform bridge queues |
| OPS-005 | `crates/fava-transport-testkit`, `crates/fava-router-testkit` | 2 shipped facilities against a 12-item mandate. Deterministic time, scripted relay frames, connection failure/reconnect, signer delay/refusal, cache and write-store restart harnesses, cancellation races, provider substitution, and platform lifecycle all live in private test modules or the excluded canary. The "prove the mechanism by disabling it" clause exists only as prose in `# fava:falsifier=` comments — there is **no executable mutation harness** |
| OPS-009 | `crates/fava/tests/observation_bounds.rs:27` | There is no engine shutdown, so "one exact owner for engine shutdown", "no fact delivered after terminal close", "backgrounding/foregrounding owners", and "repeated cycles return resources to a stable baseline" have no owner and no test |
| GOAL-005 | `crates/fava/tests/source_contract.rs:62,96` | The "shared corpus" runs over the memory *event cache* and the memory *write store* — two **different** contracts, not two implementations of one |
| GOAL-007 | `crates/fava-event-cache-memory/src/lib.rs:203` | Relay-access isolation is untestable in practice: `grep -rn 'RelayAccess::named' crates apps falsifiers` → **0 hits**; every test uses `RelayAccess::public()` |
| EVENT-011 | `crates/fava-write-store-redb/tests/semantic_write_store.rs:379` | `redb_schema_mismatch_refuses_without_fallback` is a bare `is_err()` with no variant and **no assertion that the rows survived** — the "not silently reset" half is unasserted |
| EVENT-014 | `crates/fava-event-cache-memory/src/lib.rs:178,203` | Two injection points only. The acceptance — inject failure at *each* provider-defined mutation boundary with a concurrent reader — is not performed |
| PROFILE-008 | `crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs:196` | Oldest-first is genuinely discriminating, but "active writes are never evicted" is unproven (both receipts in the test are terminal) and no `Superseded` outcome exists in `ReceiptOutcome` |
| ID-006 | `crates/fava-signer-local/src/lib.rs` | The only real implementation holds `Keys`. No external/remote/hardware signer falsifier, so "applications can supply a signer without giving Fava raw private-key bytes" is untested |
| QUERY-009 | — | Satisfied by *absence*: `QuerySnapshot`/`SourceSnapshot` have no `synced`/`complete`/percentage field, but **no test asserts the absence**, so adding one later fails nothing. Same shape for RELAY-011 (negentropy) and OPEN-002/003 |

### Caveat on the PROVEN list

`WRITE-*` scores 21/30 and is the strongest family, but its most safety-critical acceptance —
WRITE-004's *crash after `Write` returns ⇒ same receipt recovered* — is asserted against a hard-coded
`ReceiptId::from_u64(1)` at `crates/fava-write-store-redb/tests/process_kill.rs:130`, never against
the id the killed child actually returned; and every SIGKILL in that corpus lands strictly
post-commit (the child writes its marker *after* `commit()` returns,
`crates/fava-write-store-redb/tests/process_kill.rs:84`), so no torn-commit boundary is exercised.

And because CI runs no Rust tests at all, **none of the 56 proven requirements is protected against
regression by automation.**

---

## Deliverable 5 — missing adversarial classes (workspace-wide)

| Class | Coverage | Evidence |
|---|---|---|
| **Blocked provider** | Only `Signer` (three `BlockingSigner` copies) and, as of the uncommitted crisis fix, `Transport` (`explicit_live.rs:57`). **No test proves a blocked provider leaves unrelated work progressing** — `ARCHITECTURE.md:3371` Falsifier M's central claim. No two-relay test where one relay stalls and the other completes. | `grep -rn "only_from_relays(\[" crates/*/tests` → the only multi-relay facade queries are `multi_relay.rs` (all relays healthy) and the uncommitted `explicit_live.rs:250`. |
| **Partial-open cancellation** | Local sources only: `crates/fava-observe/src/lib.rs:356` `second_source_open_failure_closes_the_first_source` and `:377` `initial_evaluation_failure_closes_both_sources`. Relay partial-open is covered **only** by the uncommitted, currently-red `explicit_live.rs:236`. | — |
| **Stale completion rejection** | Good for signers (`support/semantic_write_capability_lifecycle.rs:102`) and for relay generations (`multi_relay.rs:258-271`). Absent for routers, publishers, delivery, and cache. | — |
| **Shared-work refcount** | Zero. `grep -rIli refcount` over all 90 test files → 0. The only test is the uncommitted, currently-red `explicit_live.rs:214` `equivalent_observations_share_relay_work_until_the_last_handle_closes`. | — |
| **Resource bounds under load** | Constant-comparison tests only (257 relays, 4097 bytes, 2001 tags, 256 destinations), all against hand-built values. One genuine load test: `write_bounds.rs:18` (`Lagged(1)`). No bound is ever reached through the assembled path. | — |
| **Restart / recovery** | Real SIGKILL exists (`crates/fava-write-store-redb/tests/process_kill.rs:118`) but bypasses the public path (see `restart-proof-does-not-use-the-public-path`). No `Fava` is ever built over `RedbWriteStore`. The three facade "recovery" tests open a second engine in-process — explicitly ruled out by §9.2. | — |
| **Shutdown join** | Zero. `Fava` has no `shutdown` method. `grep -rIli shutdown` over all 90 test files → 0. OPS-009 "engine shutdown" and QUERY-012 "shutdown ends all pending pulls without hanging" have neither API nor test. `crates/fava-observe/src/lib.rs:113` spawns a detached `tokio::spawn` with no join handle. | — |
| **Backpressure** | One test (`write_bounds.rs:18`). No provider double can outrun a consumer. The scripted transports have unbounded inbound `VecDeque`s but no test pushes more than four frames. | — |

Additional classes absent for the whole workspace:

- **Provider panic** — one contract only (`ReplaceableEventMaterializer`).
- **Ignore-cancellation provider** (Falsifier M) — no double ignores cancellation deliberately.
- **NIP-42 auth** (RELAY-007) — no implementation, no test.
- **NIP-05 / NIP-11 fetch cache** (RELAY-009/010, Falsifier L) — `grep -rn "FetchCache\|nip05\|nip11"`
  over `crates apps` → 0 hits. Nothing exists.
- **Evaluator substitution** (Falsifier I) — one evaluator, no differential.
- **Planner substitution** (Falsifier J) — two planners, no differential.
- **External-provider proof** (Falsifier A) — 2 of the 6 required external implementations exist
  (`falsifiers/external-null-cache` = memory/null event cache;
  `falsifiers/external-semantic-capability` = materializer capability). Missing: external
  static-table router, external no-grouping planner, external scripted transport, external no-retry
  delivery policy, external persistent write store.

---

## Top 10 — where a green test is actively lying about a public promise

Ranked by (how load-bearing the promise is) × (how confidently the test's greenness is meaningless).

1. **`apps/canary/src/grouping.rs` — `subscription-grouping-equivalence`.** Claims 300 live queries
   prove grouping preserves logical results; drives zero live queries, calls the planner and the
   transport directly, and performs ingest attribution itself. Compounded by
   `grouping-break-is-inert-production-already-does-it`: `crates/fava/src/relay.rs:211` discards
   `plan.demand` and `:269-273` admits with `&id, &id`, so the named break describes what production
   already does. **RELAY-003 has no evidence of any kind.**

2. **`crates/fava/tests/observation_bounds.rs:27` —
   `one_thousand_idle_observations_share_the_current_runtime_thread`.** Asserts
   `std::thread::current().id()` inside a `current_thread` runtime; the assertion cannot fail
   whatever Fava does. It is the only evidence for OPS-009's "no OS thread per query", and its named
   break (`features/multi-relay-observation.feature:46`) is undetectable.

3. **`crates/fava-diagnostics/tests/relay_facts.rs:17` + `crates/fava/tests/explicit_live.rs:342` —
   QUERY-EVIDENCE-001, status `built`.** RELAY-007 requires Fava to *answer* NIP-42 challenges and
   re-authenticate after reconnect. `crates/fava/src/relay.rs:300` only records a counter; no
   `ClientMessage::Auth` is ever constructed anywhere. The scripted transport's `sent()` log is
   available at `crates/fava/tests/explicit_live.rs:39` and is not consulted. **The test is green
   precisely because Fava does nothing.**

4. **`crates/fava/tests/write_settlement.rs:28` —
   `receipt_counts_preserve_complete_mixed_destination_evidence`.** Every asserted number counts a
   map the fixture wrote at `:261-324`. A publication pipeline that never records `Pending` or
   `Unknown` passes unchanged. This is the headline WRITE-018 evidence.

5. **`crates/fava/tests/simple_groups.rs:79` —
   `simple_group_records_require_actual_host_evidence`.** EVENT-003's exact promise, proved by
   asserting back the provenance the fixture wrote, in a 978-line file where `SpyTransport`
   (`:768`) guarantees no relay is ever contacted. `crates/fava/tests/multi_relay.rs:183` shows the
   correct construction 200 lines away.

6. **`apps/canary/src/publication.rs:311-320` — `cancel-pre-handoff` / WRITE-CANCEL-001.** The wire
   assertion `wire_count(EVENT) != 0` is structurally unable to fire because the `GatedSigner` is
   never released. The named break ("allow signing to continue into transport after cancellation")
   cannot be detected.

7. **`crates/fava/tests/automatic_publication.rs:75-96` and the two `preview_semantic_routes`
   variants.** The route plan is validated against the same planner call, over the same routers, and
   in the semantic variants over a router that returns one constant destination. WRITE-016/017
   preview parity is a tautology. Compounded by `apps/canary/src/automatic_publication.rs:342` —
   `Fava` has **no public write-route preview**, so the canary asserts against a function the engine
   does not own.

8. **`crates/fava/tests/local_source_merge.rs:250` —
   `slow_consumer_receives_exact_latest_state_with_bounded_delivery`,** and its canary twin
   `apps/canary/src/local.rs:46-87`. There is no slow consumer and no bound is measured; the bounded
   half of QUERY-011 rests on `diagnostics().coalesced_query_updates > 0` — Fava's own counter, `> 0`.
   The named break (`features/multi-relay-observation.feature:34`, "replace the watch boundary with
   an unbounded update queue") would not be detected.

9. **`crates/fava/tests/simple_groups/saved.rs:166` — `fn assert_ordinary_write(_write: &Write) {}`.**
   An empty function whose name is the assertion, called at `crates/fava/tests/simple_groups.rs:321`
   and `saved.rs:135,136`. Three sites read as protected and are not.

10. **`crates/fava-write-store-redb/tests/process_kill.rs:91` as WRITE-RECOVERY-001 evidence.** The
    kill is real and the crate is the best-tested in the repo, but the feature claims the *facade*
    promise ("the same receipt and event identity are **queryable** without resubmission") and the
    test never builds a `Fava` — `grep -rIl redb crates/fava/tests` returns nothing. Recovery is
    asserted against a hard-coded `ReceiptId::from_u64(1)` (`:130`), not the id the killed child
    returned, and every kill lands strictly post-commit (`:84`). *(Honourable mention:
    `crates/fava-write-store-redb/tests/process_kill/semantic.rs:394-408` does reassemble a real
    `Fava` after SIGKILL and prove exactly-once resumption — that one is genuinely excellent and
    should be the template.)*

**Meta-item, above all ten:** none of these tests is run by CI (`.github/workflows/architecture.yml`
runs only the Python vocabulary check), and none of the canary scenarios has a tracked evidence
bundle (`.gitignore:3`, `git ls-files apps/canary/runs` → 0). The 306-green number is a manual,
unreproducible measurement.

---

## Conforming (verified, not merely unexamined)

Each item below came from a search or a read that actually ran.

- **Every named evidence link resolves.** I extracted all 57 distinct `fava:evidence=rust:…` and
  `fava:rust=…` targets from `features/` and grepped for `fn <name>` across `crates apps falsifiers`.
  All resolve except the token `conformance`, which names a file
  (`crates/fava-transport-websocket/tests/conformance.rs`), not a function. No feature scenario names
  a nonexistent test.
- **Every `fava:evidence=canary:` id exists and is `enabled`.** All 18 distinct canary ids referenced
  by `features/` appear in `apps/canary/scenarios.json` with `"status": "enabled"`. No scenario is
  marked built against a disabled or absent canary.
- **`crates/fava-write-store-redb/tests/process_kill.rs` is a genuine process kill, not a fake.** It
  spawns `env::current_exe()` with `--exact boundary_child` (`:107-116`), waits on a filesystem
  marker (`:117`), `child.kill()` (`:118`), reaps and asserts `!status.success()` (`:119-120`), then
  reopens the database in the parent (`:123`). Six commit boundaries are covered
  (`before-accept`, `acceptance`, `signature`, `attempt`, `outcome`, `cancel`) with
  distinct per-boundary assertions (`:129-166`). The public-path objection in
  `restart-proof-does-not-use-the-public-path` stands, but the kill mechanism itself is real.
- **`crates/fava/tests/multi_relay.rs:229-278`** `reconnect_uses_fresh_identity_and_rejects_old_subscription_frames`
  is a correct stale-generation proof: real disconnect injection (`:247`), asserts the new generation
  strictly exceeds the old (`:251`), the subscription id differs (`:252`), an EVENT bearing the *old*
  subscription id on the *new* generation is rejected with an exact message (`:262-271`), cache stays
  empty (`:272`), and the same event on the current id is admitted (`:274-277`).
- **`crates/fava/tests/multi_relay.rs:183-227`** `duplicate_event_merges_only_actual_serving_relays`
  is a correct EVENT-003 proof: three real scripted sessions, relays 0 and 1 *send* the event, relay
  2 sends only EOSE, and relay 2's absence is asserted explicitly (`:225`).
- **`crates/fava/tests/support/semantic_write_capability_lifecycle.rs:102-206`**
  `prove_processed_stale_success` is a correct stale-completion proof: generation 1 held by a gated
  signer, generation 2 installed, generation-1 completion delivered and asserted rejected
  (`:160-166`), receipt unchanged (`:167-173`), publisher untouched (`:174`), only generation 2
  reaching the publisher with the exact event id (`:193-199`).
- **`crates/fava/tests/semantic_write_failures/**` is the strongest suite in the workspace.**
  `FaultingWriteStore` (`faults.rs:96`) provides eight independent, externally driven fault knobs
  including succeed-then-fail after signature (`:307-310`) and after route (`:335-338`) and a
  `Barrier` pause inside `apply_route` (`:339-343`). `materializer_panic_is_scoped_and_attributed`
  (`semantic_write_failures.rs:165`) proves a provider panic is attributed and an unrelated kind still
  progresses (`:196-205`). `source_isolation.rs:44` and `:85` are a true two-way isolation contrast
  with the failure reason naming the specific closed source.
- **`crates/fava/tests/semantic_write_store/current_guard.rs:29-54`** proves an *identical event body*
  cannot bypass either a stale generation or a stale source identity, and after every refusal
  re-reads the receipt and asserts byte-equality plus *no receipt-change notification*
  (`:55-62`, `:75-78`, `:95-98`). That is the "tempting wrong interpretation" exclusion §6 asks for.
- **`crates/fava-simple-groups`'s pure protocol core is among the best evidence in the repository.**
  Bounded consumption is *instrumented* rather than named (`src/tests/group.rs:84-97` counts 257
  pulls from an infinite iterator; `src/tests/saved.rs:99-139` uses a `PanicAfter` iterator).
  Byte-exactness is checked against an `as_json()` snapshot captured before the call
  (`group.rs:154-159`, `:204-206`), and `group.rs:261-271` mutates content *after* signing so it
  actually exercises verification. Row attribution is index-exact
  (`src/tests/records.rs:399-471`, `:559-634`). Encoding is checked independently rather than by
  round-trip (`records.rs:426-445`: uppercase hex rejected, lowercase form of the same key accepted).
  Absent-vs-present-empty is distinguished (`records.rs:70-88`). No sleeps are used to prove ordering
  anywhere in that crate.
- **`crates/fava-observe/src/lib.rs:356` and `:377`** are correct partial-open proofs for the two
  *local* sources: a refusing second source is asserted to close the first (`closes == 1`), and an
  evaluator failure closes both (`closes == 2`).
- **`crates/fava-ingest/tests/admission.rs:20`** is a correct pure-function admission test: typed
  `WrongSubscription`, `OffFilter`, and `InvalidEvent` refusals, cache asserted empty between each,
  then the valid event admitted.
- **`crates/fava/tests/write_bounds.rs:18`** asserts `RecvError::Lagged(1)` *exactly* and that the
  next receipt id is `2` — the one place in the workspace where boundedness is proved as an explicit
  typed shortfall rather than a constant comparison.
- **No test under `crates/` uses a sleep to prove ordering.** `grep -rn "sleep"` over all 90 test
  files returns hits only in `apps/canary/src/croissant*_tests.rs` and
  `apps/canary/src/croissant_simple_groups_supervision_tests.rs`, all of it process plumbing. The
  `crates/` suites use `tokio::task::yield_now()` loops behind a `timeout`
  (`crates/fava/tests/explicit_live.rs:395-403`) — better than a sleep. The one remaining offender is
  `crates/fava/tests/support/semantic_write.rs:490-498`, which proves a *negative* with a 25 ms
  timeout (4 call sites); that is a race in both directions and should use a barrier, which the same
  suite already demonstrates at `semantic_write_failures/faults.rs:339-343`.

---

## Open questions

1. **Is `Fava` intended to have a `shutdown`?** OPS-009 and QUERY-012 both require engine shutdown
   semantics; `crates/fava/src/lib.rs:98-259` has no such method and `crates/fava-observe/src/lib.rs:113`
   spawns a detached task with no join handle. This is a missing API, not only missing evidence, and
   overlaps `missing-owners.md:101` `runtime-no-shutdown-join`. Whose finding is it?
2. **Per-relay replaceable winners.** `crates/fava/tests/multi_relay.rs:280-378`
   `multi_relay_replaceable_authority_survives_public_facade` asserts that two events at one
   addressable coordinate from two relays **both** remain visible, matching
   `crates/fava-state/src/lib.rs:284` `relay_replaceable_winners`. The spec at
   `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:687` says a query MUST NOT observe "a new
   replaceable winner while the predecessor is simultaneously still current", and EVENT-002 requires
   deterministic coordinate resolution with timestamp/id tie-breaking. A per-relay winner rule
   appears nowhere in the authority. §4 does list "per-relay truth rather than global completeness"
   as a legitimate subject, so the design may be intended — but it is undocumented and the test name
   promises the opposite of what it asserts. **Needs an authority ruling before the test is changed.**
3. **Is `croissant-simple-groups-public-flow` in scope for the behavior corpus?** It is the largest
   canary scenario (~5,000 lines), contains the repository's best mutation suites
   (`apps/canary/src/croissant_nip02_tests.rs:48-345`, `croissant_simple_groups_tests/review_iteration_*.rs`),
   and is referenced by **no** feature file (`GROUP-04`..`GROUP-12` appear nowhere in `features/`).
   Either it should be linked, or it should be recognised as a build-provenance harness rather than
   behavioral evidence.
4. **Do `apps/canary` and `falsifiers/*` belong in the workspace?** They declare their own
   `[workspace]` and have no Bazel targets, which is why no automated run can reach them. Bringing
   them in is a one-line change with large consequences for what "green" means.
5. **`RelayAccess::named` has zero uses** (`grep -rn "RelayAccess::named" crates apps falsifiers` →
   0). Every test uses `RelayAccess::public()`. Is relay-access isolation (GOAL-007, RELAY-007)
   deferred, or is the axis dead?
> Historical audit record. Superseded by STATE-ARCH-1; not current implementation guidance.
