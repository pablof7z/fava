# Missing owners audit (`fava-runtime`, `fava-session`, `fava-auth`, and the crate-set diff)

**Date:** 2026-08-23 · **Mode:** read-only · **Area slug:** `missing-owners`

## Scope checked

Specs read (whole or by section):

- `docs/spec/ARCHITECTURE.md` — top-level map (l.211-250), crate families (l.272-282),
  `fava-session` (l.2204-2283), `fava-auth` (l.2286-2303), `fava-diagnostics` (l.2305-2337),
  `fava-runtime` (l.2339-2364), `fava` facade + public surface (l.2366-2420),
  restart/shutdown lifecycles (l.2903-2955), ownership ledger (l.2958-3010),
  dependency direction (l.3040-3095), Falsifier H (l.3277-3300),
  crate responsibility tables (l.3595-3675).
- `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` — GOAL-008 (l.226-232),
  QUERY-003 (l.300-309), WRITE-007/008 (l.799-822), ID-001..ID-006 (l.1181-1220),
  OPS-004 (l.1420-1437), OPS-009 (l.1486-1492).
- `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` — workstreams (l.115-175), M8 (l.897-984),
  M9 (l.986-1062).
- `AGENTS.md` (vocabulary policy), `docs/internals/vocabulary.toml` (whole file).
- `.planning/ROADMAP.md`, `.planning/REQUIREMENTS.md`,
  `.planning/phases/07.2-runtime-signer-lifecycle-and-parked-write-wakeup/` (context only, not authority).

Code read: `crates/fava/src/{lib,live,relay,routes,query_source,publication}.rs`,
`crates/fava-observe/src/lib.rs`, `crates/fava-routing/src/chain.rs`,
`crates/fava-publication/src/{lib,run,delivery,revision}.rs`,
`crates/fava-publisher/src/lib.rs`, `crates/fava-publisher-nip01/src/lib.rs`,
`crates/fava-signer/src/lib.rs`, `crates/fava-diagnostics/src/lib.rs`, `Cargo.toml`.

Searches actually run (workspace-wide, `crates/` + `apps/`):
`tokio::spawn|task::spawn|spawn_blocking|spawn_local|JoinSet|JoinHandle`,
`tokio::time::sleep|time::timeout|Instant::now|Duration::from|sleep(`,
`catch_unwind|is_panic|JoinError|panic::`, `struct Session|enum SessionError|struct Runtime|struct Auth`,
`fava\.close\(\)|\.shutdown\(\)|\.reset\(\)`, `owner = "fava-runtime"` in vocabulary.

---

## Crate-set diff (both directions)

Extraction: every `fava-[a-z0-9-]+` token in `ARCHITECTURE.md` (61 distinct) diffed against `ls crates` (37).

### Named by the architecture, absent from `crates/`

| Crate | Milestone that introduces it | Status |
|---|---|---|
| `fava-runtime` | **none — never appears in any M0-M11 "Crates/slices" list**; only in workstream D (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:154`) | **unscheduled** — see `runtime-crate-absent` |
| `fava-session` | none in M0-M11; workstream D (l.151); scheduled by inserted Phase 07.2 | **in flight** — see `session-owner-misplaced` |
| `fava-auth` | M8 (l.905) | not yet due (Phase 8 open) |
| `fava-fetch-cache`, `-memory`, `-redb` | M9 (l.997) | not yet due |
| `fava-nip05`, `fava-nip05-http`, `fava-nip11`, `fava-nip11-http` | M9 (l.999-1000) | not yet due |
| `fava-standard` | M9 "standard persistent profile" / M10 | not yet due |
| `fava-event-cache-redb` | M9 | not yet due |
| `fava-signer-nip46` | M10 (provider substitution) | not yet due |
| `fava-content`, `fava-nip18`, `fava-nip22`, `fava-nip25` | PROTO-010 inventory / M11 scope | not yet due |
| `fava-ffi` | M11 | not yet due |
| `*-testkit` (`fava-event-cache-`, `fava-write-store-`, `fava-subscriptions-`, `fava-publisher-`, `fava-signer-`) | M10 GOAL-009 conformance kits | not yet due (`fava-router-testkit` and `fava-transport-testkit` already exist) |
| `fava-relay-lab` | — | **conforming by design**: `ARCHITECTURE.md:3674` explicitly says "no `fava-relay-lab` crate is created"; the role is `apps/canary`, which exists |

Only `fava-runtime` and `fava-session` are *past due*. The rest are correctly-sequenced future work and are **not** reported as findings.

### Exist in `crates/`, unnamed by the architecture

| Crate | Verdict |
|---|---|
| `fava` | named (`ARCHITECTURE.md:2366`, `:3632`) — the regex simply does not match the bare token |
| `fava-subscriptions-no-grouping` | **approved vocabulary addition** — `docs/internals/vocabulary.toml:684` and `:690` list it under the `SubscriptionPlanner` term's `crates` and `spec_crates`. `ARCHITECTURE.md:3602-3603` covers it generically ("subscription planners"). No finding. |

No unapproved crate exists. That direction of the diff is clean.

---

## Findings

### `runtime-crate-absent` — critical — ownership

**authority** — `docs/spec/ARCHITECTURE.md:3629`: "| `fava-runtime` | Execution resources, provider isolation, cancellation, timers, and shutdown joins. |"
and `:2990`: "| Execution resources and joins | `fava-runtime` | all state owners |"
and `:2345-2355` (owned resources): "task execution; timers and clocks; bounded command/completion channels; router sessions and their asynchronous input queries; source-observation polling; transport sessions; publisher futures; signer and auth provider operations; provider panic/failure isolation; cancellation propagation; resource joining and shutdown deadlines."
`:281` places it in "Universal owners … Fava-instance lifecycles and cross-subsystem ordering."

**implementation** — the crate does not exist: it is absent from `ls crates` and from `Cargo.toml` `[workspace] members` (`/Users/pablo/Work/fava/Cargo.toml:3-40`). It has **no** vocabulary term (`grep 'owner = "fava-runtime"' docs/internals/vocabulary.toml` → 0 hits; the only mentions are as `spec_crates` on the `Fava` term, `docs/internals/vocabulary.toml:270`). It is named in **no** milestone's "Crates/slices" list in `FAVA_REWRITE_IMPLEMENTATION_PLAN.md` and in **no** `.planning/ROADMAP.md` phase. Its eleven owned resources are instead spread across five crates as ten detached `tokio::spawn` calls (table below).

**classification** — `homeless` for joins/shutdown/panic-isolation/provider deadlines; `misplaced` for task execution and cancellation (each state owner privately re-implements a `watch::channel(bool)` cancel of its own).

**observable distinction** — an application cannot bound teardown: there is no operation on `Fava` that returns after outstanding work has stopped. Dropping every `Fava` clone leaves all ten task families running (each holds an `Arc` clone of the providers). See `runtime-no-shutdown-join` for the falsifier.

**proposed falsifier** — `fava_names_one_execution_owner`
```rust
// crates/fava/tests/runtime_owner.rs
#[test]
fn no_owner_spawns_its_own_detached_task() {
    // architecture gate: every async resource is created through fava-runtime
    let hits = grep_workspace_src("tokio::spawn");
    assert_eq!(hits, vec!["crates/fava-runtime/src/lib.rs"]); // fails today: 10 sites, 0 in fava-runtime
}
```
**confidence** — confirmed.

---

### `runtime-no-shutdown-join` — critical — failure isolation

**authority** — `docs/spec/ARCHITECTURE.md:2932-2954` (Shutdown): "facade enters Closing → new application work is refused → pending facade calls receive terminal lifecycle facts → … → transport sessions close and join → stores flush/close according to provider contract → **runtime joins owned resources** → facade enters Closed", followed by ":2956 Each resource is closed by its owner. The facade owns shutdown ordering."
`:2401` lists "deterministic close and destructive reset" as part of the facade's public surface.
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1488` (OPS-009): "Opening, observing, cancelling, dropping, closing, backgrounding, foregrounding, and **engine shutdown** MUST each have one exact owner." `:1490`: "No event, receipt fact, callback, or provider completion may be delivered after terminal close." `:1492`: "Repeated close is harmless."
`:307` (QUERY-003): "Engine shutdown refusal and inability to read the initial local sources MUST remain distinguishable."

**implementation** — `Fava` has no close, shutdown, reset, lifecycle state, or `Drop`. Its complete public surface is 12 methods at `crates/fava/src/lib.rs:98,108,121,137,145,159,171,184,193,202,217,226,235` — none is a lifecycle terminator. `grep -rn 'fava\.close()\|\.shutdown()\|\.reset()' crates/ apps/` returns only `apps/canary`'s own test proxy (`apps/canary/src/proxy.rs`), never Fava. There is no `Closing`/`Closed`/`Running` state anywhere in `crates/fava/src/`. Because `Fava` is `#[derive(Clone)]` (`crates/fava/src/lib.rs:80`) with every field an `Arc`, dropping the last handle drops nothing that the ten spawned tasks depend on — each task captured its own clone.

**observable distinction** — an application that opens a live query, publishes a write, and then drops every `Fava` clone still has relay sockets open, still writes to its `WriteStore`, and still records diagnostics. In a test harness the process cannot deterministically quiesce; `#[tokio::test]` masks this because the runtime is torn down under the tasks. QUERY-003's required distinction between "engine shutting down" and "sources unreadable" is unrepresentable: `ObserveError` has no shutdown variant.

**proposed falsifier** — `close_joins_outstanding_work`
```rust
#[tokio::test]
async fn close_joins_outstanding_work() {
    let fava = build_with_counting_store();          // store counts writes after close
    let _obs = fava.observe(live_query()).await.unwrap();
    let _w  = fava.publish(unsigned()).unwrap();
    fava.close().await.unwrap();                     // does not compile today
    assert_eq!(store.writes_after_close(), 0);
    fava.close().await.unwrap();                     // OPS-009: repeated close is harmless
}
```
**confidence** — confirmed.

---

### `runtime-detached-tasks` — major — failure isolation / boundedness

**authority** — `docs/spec/ARCHITECTURE.md:2345-2355` (runtime owns "task execution", "cancellation propagation", "provider panic/failure isolation", "resource joining and shutdown deadlines"); `:2359`: "Universal owners decide what work is authorized. The runtime performs the work and returns typed completions."
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:228` (GOAL-008): "Application-supplied providers may block, fail, **panic** … MUST NOT indefinitely block unrelated relays, queries, writes, signers, or shutdown." `:230`: "Late completions MUST carry enough identity to be dropped when stale."
`:1420-1435` (OPS-004) requires explicit bounds on "active relay sessions", "provider operations", "observation delivery".

**implementation** — complete workspace inventory of spawned tasks in shipping crates (`apps/canary` and `*/tests/*` excluded; both are test-only harnesses and are listed separately below). **Every `JoinHandle` returned is dropped immediately; there is no `JoinSet`, no `JoinHandle`, and no `JoinError` anywhere under `crates/*/src/`.**

| # | file:line | spawned by | joinable | cancellable | bounded | joined at shutdown |
|---|---|---|---|---|---|---|
| 1 | `crates/fava-observe/src/lib.rs:113` | `fava-observe` (`Observation::start` merge loop) | no | yes — `watch` cancel, `Observation::close`/`Drop` | 1 per observation; **no cap on observations** | no |
| 2 | `crates/fava-routing/src/chain.rs:86` | `fava-routing` (`monitor_router`, one per router) | no | yes — `cancel_rx` | ≤ `MAX_ROUTERS` = 32 per chain; **no cap on chains** | no |
| 3 | `crates/fava-routing/src/chain.rs:95` | `fava-routing` (`compose_updates`) | no | yes — `cancel_rx` | 1 per chain; no cap on chains | no |
| 4 | `crates/fava-publication/src/run.rs:40` | `fava-publication` (custody loop, one per receipt) | no | yes — `cancellations` map | 1 per receipt; **no cap on receipts** | no |
| 5 | `crates/fava-publication/src/run.rs:437` | `fava-publication` (`start_signing`) | no | **only if the provider honours `cancel`** — no Fava-side deadline | 1 per revision generation | no |
| 6 | `crates/fava-publication/src/delivery.rs:67` | `fava-publication` (one delivery lane per destination) | no | **only if the provider honours `cancel`** | ≤ `destination_evidence_capacity()` per receipt | no |
| 7 | `crates/fava/src/live.rs:59` | **`fava` facade** (`OpenedRelay::run`) — architecture assigns this to observe/transport | no | yes — `observation.attach_cancellation` | 1 per explicit relay per observation | no |
| 8 | `crates/fava/src/routes.rs:53` | **`fava` facade** (automatic route-plan loop) | no | yes | 1 per automatic observation | no |
| 9 | `crates/fava/src/routes.rs:158` | **`fava` facade** (`OpenedRelay::run` per route destination) | no | yes | ≤ `MAX_DESTINATIONS` = 256 per revision | no |
| 10 | `crates/fava/src/query_source.rs:25` | **`fava` facade** (`impl QuerySource for Fava`) | no | **not promptly** — the `select!` loop is only entered *after* `fava.observe(query).await` returns, and that await has no deadline (see `runtime-no-provider-deadline`) | 1 per nested source open; **recursion depth unbounded** | no |

Test-only harnesses (out of shipping scope, listed for completeness): `crates/fava-transport-websocket/tests/conformance.rs:28,50,68,85`; `crates/fava/tests/write_settlement.rs:67,110,127,177,222`; `crates/fava/tests/semantic_write_failures/faults.rs:119`; `apps/canary/src/{proxy.rs:40, semantic_process.rs:55, croissant_simple_groups.rs:352,376, semantic_write_support.rs:420,427, croissant_simple_groups_supervision_tests.rs:84, hostile.rs:32, croissant.rs:697}`. Notably `apps/canary` is the *only* place in the repository that uses `JoinSet`/`JoinHandle` and actually drains tasks (`apps/canary/src/proxy.rs:105 drain_connections`) — the test harness has the join discipline the library lacks.

Panic isolation: exactly one `catch_unwind` exists in the whole workspace, `crates/fava-publication/src/revision.rs:218`, guarding the `EditApplier` provider. No other provider boundary is guarded. Because handle #1's `JoinHandle` is dropped, a panic inside `evaluator.evaluate(...)` (`crates/fava-observe/src/lib.rs:100` / `:156`) unwinds the task, drops `latest_tx`, and surfaces to the application as `Err(ObservationClosed)` from `Observation::changed()` (`crates/fava-observe/src/lib.rs:195`) — byte-identical to an ordinary close. `DiagnosticsSnapshot` (`crates/fava-diagnostics/src/lib.rs:16-39`) has no provider field at all, so nothing records the panic; `ARCHITECTURE.md:2330` specifies `pub providers: Vec<ProviderDiagnostic>`.

**classification** — `homeless` (joins, panic isolation, task bounds); `misplaced` (rows 7-10: task execution owned by the facade).

**observable distinction** — install a `QueryEvaluator` that panics on the second `evaluate`. The application observes `ObservationClosed` and `Fava::diagnostics()` reports nothing — indistinguishable from having called `close()` itself. GOAL-008's acceptance ("deliberately … panicking one provider leaves unrelated work and shutdown within declared bounds") cannot be evaluated because "shutdown" does not exist.

**proposed falsifier** — `panicking_evaluator_is_attributed_not_silent`
```rust
#[tokio::test]
async fn panicking_evaluator_is_attributed_not_silent() {
    let fava = build_with(PanicOnSecondEvaluate::new());
    let mut obs = fava.observe(local_query()).await.unwrap();
    commit_one_cache_event(&fava);
    assert!(matches!(obs.changed().await, Err(ObservationClosed)));
    let providers = fava.diagnostics().providers;      // field absent today
    assert!(providers.iter().any(|p| p.panicked()));   // fails today
}
```
**confidence** — confirmed.

---

### `runtime-no-provider-deadline` — major — failure isolation / replaceability

**authority** — `docs/spec/ARCHITECTURE.md:2363`: "Potentially blocking or application-supplied provider calls run outside owner locks and store transactions. **A stalled provider has bounded influence and cannot block unrelated owner progress or Fava shutdown indefinitely.**"
`:2346` (runtime owns "timers and clocks"), `:2355` ("resource joining and **shutdown deadlines**").
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:806-812` (WRITE-007): "Unavailable, rejected, invalid-output, cancelled, **timed-out**, and stale signer results remain distinct."
`:1428` (OPS-004) requires a bound on "provider operations".

**implementation** — complete inventory of every time value in shipping crates (`grep -rn 'Duration|Instant' crates/*/src/`; `Instant` appears **zero** times workspace-wide, so no deadline exists anywhere):

| file:line | value | classification |
|---|---|---|
| `crates/fava-publication/src/delivery.rs:14` | `const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5)` | **ad-hoc** — a hard-coded literal inside the publication owner. `DeliveryPolicy` is the architecture's delivery policy owner (`ARCHITECTURE.md:3613`) and never sees it. |
| `crates/fava-publication/src/delivery.rs:180` | `timeout: ATTEMPT_TIMEOUT` placed into `PublishAttempt` | **ad-hoc / advisory only** — the value is *handed to the provider as data*; Fava does not enforce it. `self.publisher.publish(attempt, …).await` at `delivery.rs:182-185` is a bare await with no `tokio::time::timeout` wrapper. |
| `crates/fava-publisher-nip01/src/lib.rs:58` | `tokio::time::timeout(attempt.timeout, …)` | **provider-owned** — the *default* publisher voluntarily honours it. A competing `Publisher` that ignores `attempt.timeout` hangs the lane forever, and nothing detects it. |
| `crates/fava-publication/src/run.rs:14,215` | `STORE_READ_RETRY_DELAY = 10ms` + `tokio::time::sleep` | **ad-hoc** — hard-coded backoff in the custody loop; no policy owner, no ceiling, no attempt count. |
| `crates/fava/src/relay.rs:135` | `tokio::time::sleep(Duration::from_millis(50))` in `OpenedRelay::reconnect` | **ad-hoc** — hard-coded fixed reconnect backoff, in the facade, with no cap and no policy owner. |
| `crates/fava-publisher/src/lib.rs:27` | `pub timeout: Duration` field on `PublishAttempt` | contract-level carrier for the advisory value above |

There is **no Fava-owned timeout on any other provider boundary**: `signer.sign_event(unsigned, cancel).await` (`crates/fava-publication/src/run.rs:438`) has no deadline and `SignerError` (`crates/fava-signer/src/lib.rs:37-50`) has **no `TimedOut` variant** — only `Unavailable`, `Rejected`, `Cancelled`, `InvalidOutput` — so WRITE-007's required "timed-out" outcome is not representable. `transport.open_session(...)`, `RelaySession::next_message()`, `EventCache` and `WriteStore` calls are likewise unbounded.

**classification** — `homeless`.

**observable distinction** — supply a `Signer` whose `sign_event` never resolves and never observes `cancel`. The accepted write stays in a non-terminal state forever, `Write::settled(all())` never returns, the receipt records no signer fact, and there is no engine-level operation that can end it. The same applies to a `Publisher` that ignores `attempt.timeout`. Because the default `fava-publisher-nip01` privately honours the deadline while the contract does not require or enforce it, the default provider has a guarantee a substituted provider cannot be held to — a replaceability bypass.

**proposed falsifier** — `stalled_signer_is_bounded_and_typed`
```rust
#[tokio::test]
async fn stalled_signer_is_bounded_and_typed() {
    let fava = build_with(NeverResolvingIgnoringCancel::for_key(alice()));
    let w = fava.publish(unsigned_by(alice())).unwrap();
    let receipt = w.settled(all()).await.unwrap();      // hangs forever today
    assert!(receipt.signer_outcome_is(SignerError::TimedOut)); // variant absent today
}
```
**confidence** — confirmed.

---

### `session-owner-misplaced` — critical — ownership / vocabulary

**authority** — `docs/spec/ARCHITECTURE.md:2982`: "| Signer registration and availability | **`fava-session`** plus signer provider | publication/auth owners |"
`:3626`: "| `fava-session` | Accounts, current-account input, signer registrations, and session restore. |"
`:2204-2206`: "## `fava-session` — **Responsibility:** own the application-visible account set and current-account input. The first delivered slice owns the bounded runtime signer attachment for each exact account public key."
`:2211-2222` specifies the exact nominal types `Session` and `SessionError { DuplicateSigner, MissingSigner, SignerCapacityExceeded, GenerationExhausted }`.
`:2261-2263`: "The public `Fava` facade delegates runtime `add_signer`, explicit `replace_signer`, and `remove_signer` operations to this owner. Builder-supplied signers seed the same `Session`; they are not copied into publication-owned state."
`docs/internals/vocabulary.toml:779-792` already registers the term `Session` with `owner = "fava-session"`, `spec_symbols = ["Session","SessionError"]`, `spec_crates = ["fava-session"]`, and `symbols = [] / crates = []` (i.e. approved but unimplemented).
`FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1181-1187` (ID-001): "A session contains accounts, current-account selection, and attached signer/crypto provider configuration."

**implementation** — the crate does not exist. Signer registration is a private immutable map owned by the publication crate:
`crates/fava-publication/src/lib.rs:33` — `signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>`, populated once in `Publication::new` (`:53-72`), read at `crates/fava-publication/src/run.rs:426`. The duplicate-signer error is spelled `PublicationError::DuplicateSigner(PublicKey)` (`crates/fava-publication/src/lib.rs:306`) rather than `SessionError::DuplicateSigner`. The facade only accepts signers at build time (`crates/fava/src/lib.rs:259,335,345,423`); `grep 'struct Session|enum SessionError' crates/*/src/` → 0 hits. There is no account set, no current account, and no session import/export anywhere.

**classification** — `misplaced` for signer registration (publication owns what session must own); `homeless` for accounts, current account, and session restore.

**observable distinction** — an application cannot attach a signer to a running `Fava`. An unsigned write accepted before the signer exists is permanently parked, because the only way to introduce a signer is `FavaBuilder::signer` before `build()`, and rebuilding creates a new `Publication` with a new `signers` map — the durable receipt survives but nothing in the new engine is the same lifecycle. This is exactly the falsifier already recorded in `docs/internals/vocabulary.toml:788`.

**proposed falsifier** — `runtime_signer_attachment_wakes_the_exact_parked_write`
```rust
#[tokio::test]
async fn runtime_signer_attachment_wakes_the_exact_parked_write() {
    let fava = Fava::builder()./* no signers */.build().unwrap();
    let w = fava.publish(unsigned_by(alice())).unwrap();
    fava.add_signer(local_signer(bob())).unwrap();        // no such method today
    assert!(w.receipt().unwrap().is_awaiting_signer());
    fava.add_signer(local_signer(alice())).unwrap();
    assert!(w.settled(all()).await.unwrap().is_signed()); // fails today
}
```
**confidence** — confirmed. **Note:** Phase 07.2 (`.planning/phases/07.2-.../07.2-01-PLAN.md:11-14`) already schedules `crates/fava-session/`. This is reported because it is a *currently true* ownership contradiction, not because the remediation is unknown.

---

### `runtime-unscheduled` — major — ownership (process gate)

**authority** — `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:154` lists `fava-runtime` in workstream D "Universal lifecycle owners", alongside `fava-ingest`, `fava-observe`, `fava-publication`, `fava-session`, `fava-auth`, `fava-diagnostics`, `fava`. `AGENTS.md` vocabulary policy makes a **lifecycle owner** a vocabulary change.

**implementation** — every other workstream-D crate is either built (`fava-ingest`, `fava-observe`, `fava-publication`, `fava-diagnostics`, `fava`), scheduled (`fava-session` → Phase 07.2), or milestone-assigned (`fava-auth` → M8). `fava-runtime` alone appears in **no** milestone "Crates/slices" list (verified by grep over the whole plan: hits at l.154 only, plus the two "protocol crates must not depend on" exclusion lists at `ARCHITECTURE.md:3081` and `:3285`), in **no** `.planning/ROADMAP.md` phase, and has **no** vocabulary term. Its responsibilities are therefore not merely unbuilt — they are unowned by any plan, and M8's "bounded provider execution" / "provider-failure-isolation" exit gates (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:913,969`) currently have no crate to land in.

**observable distinction** — Falsifier H (`ARCHITECTURE.md:3277-3300`) requires a new external protocol crate to need "zero edits to … `fava`, **`fava-runtime`**, `fava-observe`, …". That clause is vacuously satisfiable today and will silently change meaning when the crate appears, so the M10 qualification cannot distinguish a conforming from a non-conforming outcome for that line.

**proposed falsifier** — `every_workstream_d_owner_has_a_scheduled_home`
```rust
#[test]
fn every_named_lifecycle_owner_exists_or_is_scheduled() {
    for crate_name in workstream_d_crates() {           // parsed from the plan
        assert!(crate_dir_exists(crate_name) || milestone_for(crate_name).is_some(),
                "{crate_name} is named as a lifecycle owner but has no crate and no milestone");
    } // fails today on fava-runtime
}
```
**confidence** — confirmed.

---

### `auth-owner-absent` — minor — ownership (informational, not yet due)

**authority** — `docs/spec/ARCHITECTURE.md:2981`: "| NIP-42 challenge lifecycle | `fava-auth` | query/publication owners |"; `:3627`; `:2286-2301` (owned state: relay-access identity, current relay challenge, application authentication-policy operation, signer operation for the AUTH event, current session generation, accepted/refused/failed authentication facts, re-authentication after reconnect, exact attribution to query and publication work).

**implementation** — crate absent. What exists today is *acknowledgement without a lifecycle*: `crates/fava-diagnostics/src/lib.rs:174 authentication_required(session, generation)` records the challenge as a bare fact, and `crates/fava-publication/src/delivery.rs:203-205` converts `PublishOutcome::AuthenticationRequired` straight into `RelayDeliveryOutcome::GivenUp { reason: "relay authentication required" }` — no challenge state, no policy hook, no AUTH signing, no re-authentication after reconnect. `crates/fava/src/relay.rs` never handles an `AUTH` frame. `grep 'struct Auth' crates/*/src/` → 0 hits.

**classification** — `homeless`, correctly deferred: M8 (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:905`), Phase 8 open in `.planning/ROADMAP.md`, HARD-* requirements all `[ ]` in `.planning/REQUIREMENTS.md:139-145`.

**observable distinction** — a relay that requires NIP-42 for writes terminates the lane as `GivenUp` rather than authenticating. This is the expected pre-M8 state; recorded so the audit's absence claim is grounded rather than assumed.

**proposed falsifier** — deferred to Phase 8; the M8 canary scenario `nip42-write-and-reconnect` (`FAVA_REWRITE_IMPLEMENTATION_PLAN.md:936-942`) is the intended one.

**confidence** — confirmed (as a deferral, not a deviation).

---

## Conforming (verified, not merely unexamined)

- **Reverse crate diff is clean.** The only crate in `crates/` unnamed by `ARCHITECTURE.md` is `fava-subscriptions-no-grouping`, and it is an **approved** vocabulary addition — `docs/internals/vocabulary.toml:684` and `:690`. No unapproved crate exists.
- **`fava-relay-lab` is correctly absent.** `ARCHITECTURE.md:3674` explicitly forbids the crate and assigns the role to `apps/canary`, which exists and is the only component in the repository with real task-join discipline (`apps/canary/src/proxy.rs:28,40,105` — `JoinHandle` + `JoinSet` + `drain_connections`).
- **All other missing crates are correctly sequenced.** `fava-auth` → M8; `fava-fetch-cache*`, `fava-nip05*`, `fava-nip11*`, `fava-event-cache-redb`, `fava-standard` → M9; `fava-signer-nip46`, the five outstanding `*-testkit` crates → M10; `fava-ffi`, `fava-content`, `fava-nip18/22/25` → M11. Phases 8-11 are all `[ ]` in `.planning/ROADMAP.md`. None reported.
- **Cancellation, where present, is genuinely wired.** Rows 1-4 and 7-9 of the spawned-task table all observe a `watch::Receiver<bool>` under `biased` `select!` and terminate. `Observation` cancels on `close()` and on `Drop`; `FavaChanges` likewise (`crates/fava/src/query_source.rs:81-85`). The gap is joins and provider deadlines, not the absence of cancellation signalling.
- **The applier provider boundary *is* panic-isolated** — `crates/fava-publication/src/revision.rs:218` `std::panic::catch_unwind(AssertUnwindSafe(...))`. It is the sole conforming instance of `ARCHITECTURE.md:2353` and shows the intended shape.
- **Routing fan-out is bounded** — `crates/fava-routing/src/chain.rs:13-19` (`MAX_ROUTERS` 32, `MAX_DESTINATIONS`/`MAX_TARGETS`/`MAX_COVERAGE`/`MAX_COVERED_SESSIONS`/`MAX_SHORTFALLS` 256, `MAX_TEXT_BYTES` 4096). Delivery lanes are bounded per receipt by `destination_evidence_capacity()` (`crates/fava-publication/src/run.rs:61`). Appliers are capped at 64 (`revision.rs:15`). The unbounded dimensions are *counts of owners* (observations, chains, receipts), which is the runtime resource bound.
- **`fava-diagnostics` exists and is bounded** (`crates/fava-diagnostics/src/lib.rs:72 bounded(capacity)`), so its absence from this report is a positive finding, not an omission.

## Open questions

1. **Does `fava-runtime` need a crate, or is it a cross-cutting discipline?** Every one of its eleven owned resources could be satisfied by a rule ("no `tokio::spawn` outside an injected executor handle") plus a `Runtime` value threaded through `FavaBuilder`. The architecture names it as a crate in three places, but no milestone schedules it — this may be an architecture/plan inconsistency rather than an implementation gap, and should be settled at the architecture level before Phase 8 lands M8's "bounded provider execution" and "provider-failure-isolation" gates with nowhere to put them.
2. **Should the provider deadline be a `fava-runtime` policy or a per-contract field?** `PublishAttempt.timeout` is already a contract field (`crates/fava-publisher/src/lib.rs:27`) that the owner does not enforce. Deciding between "runtime wraps every provider await in `tokio::time::timeout`" and "each contract carries an advisory deadline the owner enforces" changes whether `SignerError` gains a `TimedOut` variant.
3. **Does Phase 07.2 also deliver the account set and current account,** or only the signer-attachment slice? `ARCHITECTURE.md:2206` says the first slice is signer attachment only, but `:2265-2272` lists account identities, current account, and all-or-nothing import/export as `fava-session` owned state, and ID-002/ID-005 depend on them. If 07.2 ships only signers, `fava-session` remains a `partial` owner and ID-002/ID-005 need an explicit later home.
4. **Is `apps/canary`'s reconnect/quiesce behavior currently masking the missing shutdown join?** Every canary scenario ends with `proxy.shutdown()` (its own proxy), never with a Fava close, and every Rust test runs under `#[tokio::test]`, which tears the runtime down under the detached tasks. No existing evidence would fail if `Fava::close()` were added and did nothing.
