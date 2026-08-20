# Testing Patterns

**Analysis Date:** 2026-08-20

## Test Framework

**Runner:**
- Rust's built-in Cargo test harness under the pinned Rust 1.90.0 toolchain runs workspace, standalone canary, and external-provider tests (`rust-toolchain.toml`, `Cargo.toml`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`).
- Tokio 1.53.1 supplies asynchronous tests; public-facade and observation tests use `#[tokio::test(flavor = "current_thread")]` for deterministic single-thread scheduling (`Cargo.toml`, `crates/fava/tests/local_source_merge.rs`, `crates/fava-observe/src/lib.rs`).
- No separate unit-test configuration file is present; test targets are discovered from `#[cfg(test)]` modules and `tests/*.rs` according to Cargo conventions (`Cargo.toml`, `crates/fava-query-standard/tests/source_merge.rs`).

**Assertion Library:**
- Use standard Rust `assert!`, `assert_eq!`, `assert_ne!`, and `matches!`; no third-party assertion or snapshot-testing dependency is declared (`Cargo.toml`, `apps/canary/Cargo.toml`, `crates/fava-observe/src/lib.rs`).
- Use `expect` only to establish a test precondition with a causal message; assertions must inspect the result under proof (`crates/fava/tests/local_source_merge.rs`, `crates/fava-query-standard/tests/source_merge.rs`).

**Run Commands:**
```bash
cargo test --workspace --all-targets
# Run all tests in the main workspace declared by Cargo.toml.

cargo test --manifest-path apps/canary/Cargo.toml
# Run the standalone canary's ordinary test harness.

cargo test --manifest-path falsifiers/external-null-cache/Cargo.toml
# Run the outside-workspace public-provider falsifier.

cargo test -p fava --test local_source_merge accepted_local_event_is_visible_without_cache_pollution -- --exact
# Run one focused public-facade acceptance test from crates/fava/tests/local_source_merge.rs.

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
# Required formatting and lint gates recorded in docs/issues/0001-local-source-merge.md.
```

- No watch-mode command is configured in `Cargo.toml`; rerun the focused `cargo test` command while developing the owning behavior (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- The deterministic live M0 scenario is run separately with `cargo run --manifest-path apps/canary/Cargo.toml -- run lab-real-relay-smoke --seed <unique-seed>` after installing the pinned relay prerequisite (`apps/canary/README.md`, `apps/canary/src/main.rs`).

## Test File Organization

**Location:**
- Co-locate narrow unit tests at the bottom of the owning source file in `#[cfg(test)] mod tests`, as in `crates/fava-state/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- Put crate-level integration tests under the owner's `tests/` directory: evaluator/component evidence is in `crates/fava-query-standard/tests/source_merge.rs`, while public-facade acceptance evidence is in `crates/fava/tests/local_source_merge.rs`.
- Keep architectural substitution proof outside the main workspace in `falsifiers/external-null-cache/src/lib.rs`; this proves public contracts are sufficient without private access (`falsifiers/external-null-cache/Cargo.toml`, `docs/spec/ARCHITECTURE.md`).
- Keep the ordinary downstream process/wire evidence application in its own workspace at `apps/canary/`; it has no dependency on Fava crates and owns its local tests, real-relay runner, and evidence artifacts (`apps/canary/Cargo.toml`, `apps/canary/README.md`).
- Keep readable application behavior under `features/`; Gherkin is product memory and does not require a Cucumber runner (`features/local-source-merge.feature`, `features/relay-lab.feature`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

**Naming:**
- Name unit and integration tests as snake_case behavioral claims: `failed_capacity_batch_is_atomic`, `same_signed_event_merges_relay_and_publication_evidence`, and `relay_echo_enriches_one_record_without_erasing_receipt` are representative (`crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs`).
- Name integration-test files and feature files after the coherent behavior slice, not the crate implementation: `crates/fava/tests/local_source_merge.rs` and `features/local-source-merge.feature` cover the same public distinction at different evidence layers.

**Structure:**
```text
crates/<owner>/src/lib.rs                  # Co-located unit tests
crates/<owner>/tests/<behavior>.rs          # Owner integration tests
crates/fava/tests/<behavior>.rs             # Public Rust facade acceptance
features/<behavior>.feature                 # Readable behavior and falsifier
falsifiers/<boundary>/src/lib.rs            # External architectural proof
apps/canary/src/                            # Downstream process/wire lab
apps/canary/scenarios.json                  # Enabled/reconnaissance registry
```

The active examples for this layout are `crates/fava-observe/src/lib.rs`, `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs`, `features/local-source-merge.feature`, `falsifiers/external-null-cache/src/lib.rs`, and `apps/canary/scenarios.json`.

## Test Structure

**Suite Organization:**

Use small fixture helpers, arrange real causal inputs, invoke the owning public operation, and assert the observable result. This current public-facade pattern comes from `crates/fava/tests/local_source_merge.rs`:

```rust
#[tokio::test(flavor = "current_thread")]
async fn accepted_local_event_is_visible_without_cache_pollution() {
    let (fava, cache, writes) = assembly();
    let unsigned = unsigned_event(&Keys::generate(), Kind::TextNote, 10, "local");
    let mut feed = fava
        .observe(EventQuery::events().cache_only())
        .await
        .expect("query opens from local sources");

    let accepted = writes
        .accept_materialized(EventValue::Unsigned(unsigned))
        .expect("write store accepts finalized local event");
    let visible = next_snapshot(&mut feed).await;

    assert_eq!(visible.events.len(), 1);
    assert_eq!(
        visible.events[0].publication.as_ref().map(|value| value.receipt_id),
        Some(accepted.receipt_id)
    );
    assert!(cache.is_empty().expect("cache remains readable"));
}
```

**Patterns:**
- Setup provides causes through supported constructors and operations, never by inserting the answer under proof; use event builders, cache commits, write acceptance, relay frames, and process launches as inputs (`crates/fava/tests/local_source_merge.rs`, `apps/canary/src/lib.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Keep the assertion at the smallest stable owner, then add a public-facade or canary capstone only when it proves an additional cross-boundary fact (`crates/fava-query-standard/tests/source_merge.rs`, `crates/fava/tests/local_source_merge.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Close or drop lifecycle handles and verify scoped closure when cleanup is part of the claim; observation tests count source closes and the canary owns process/proxy shutdown (`crates/fava-observe/src/lib.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/relay.rs`).
- Prefer exact IDs, evidence, revisions, and statuses over loose counts when identity matters; current acceptance assertions inspect event IDs, receipt IDs, relay-evidence cardinality, and source status (`crates/fava/tests/local_source_merge.rs`, `crates/fava-observe/src/lib.rs`).

## Mocking

**Framework:** No mocking framework is used; tests define small handwritten implementations of public contracts (`Cargo.toml`, `crates/fava-observe/src/lib.rs`, `falsifiers/external-null-cache/src/lib.rs`).

**Patterns:**

Use a narrow fake only to control the boundary under test. This source-failure pattern comes from `crates/fava-observe/src/lib.rs`:

```rust
struct RefusingSource;

impl QuerySource for RefusingSource {
    fn open(&self, _query: &CanonicalQuery) -> Result<OpenedQuerySource, QuerySourceError> {
        Err(QuerySourceError::Refused(
            "injected open failure".to_owned(),
        ))
    }
}
```

- `TrackingSource`, `TrackingChanges`, `RefusingSource`, `EmptyEvaluator`, and `FailingEvaluator` isolate open/close and evaluation behavior without a general-purpose mock layer (`crates/fava-observe/src/lib.rs`).
- `NullEventCache` is a materially different provider compiled outside the root workspace and assembled through public contracts (`falsifiers/external-null-cache/src/lib.rs`, `falsifiers/external-null-cache/Cargo.toml`).
- The public-facade tests use real in-memory providers and the standard evaluator rather than mocking merge semantics (`crates/fava/tests/local_source_merge.rs`).
- The M0 canary uses a real pinned relay process plus a transparent WebSocket proxy rather than mocking persistence or external frame handoff (`apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/wire.rs`).

**What to Mock:**
- Fake a neutral provider contract when the claim is provider refusal, close, late completion, or failure isolation at that boundary (`crates/fava-observe/src/lib.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Inject deterministic clocks, barriers, relay frames, signer outcomes, or failures when those are causes needed to control a distributed schedule (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`).

**What NOT to Mock:**
- Do not mock the semantic owner whose decision is being proved or let the fixture calculate/insert the expected route, result, coverage, or receipt (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Do not use diagnostics to self-certify external effects; compare public results with wire, relay, process, filesystem, or platform witnesses (`apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`).
- Do not use uncontrolled public relays as a deterministic pass/fail oracle; public mode is read-only reconnaissance in `apps/canary/src/recon.rs` and `apps/canary/scenarios.json`.

## Fixtures and Factories

**Test Data:**

Use local helper functions that produce valid domain inputs with explicit times and identities. This pattern comes from `crates/fava-query-standard/tests/source_merge.rs`:

```rust
fn signed_event(keys: &Keys, kind: Kind, created_at: u64, content: &str) -> Event {
    EventBuilder::new(kind, content)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn snapshot(kind: SourceKind, events: Vec<SourceEvent>) -> SourceSnapshot {
    SourceSnapshot {
        kind,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        events,
    }
}
```

**Location:**
- Keep behavior-specific factories in the test target that consumes them: `assembly`, `signed_event`, `unsigned_event`, and `evidence` live in `crates/fava/tests/local_source_merge.rs`; evaluator-only helpers live in `crates/fava-query-standard/tests/source_merge.rs`.
- Use `tempfile::tempdir` for isolated filesystem tests and caller-selected unique seeds for preserved canary runs (`apps/canary/src/recon.rs`, `apps/canary/src/artifacts.rs`, `apps/canary/README.md`).
- Derive disposable identities deterministically from the scenario seed and isolate relay port/data/process state per run (`apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`).
- Preserve live evidence under the ignored `apps/canary/runs/` tree with a manifest, JSONL, reports, logs, resources, and hashes (`apps/canary/README.md`, `apps/canary/src/artifacts.rs`).

## Coverage

**Requirements:** No line or branch coverage percentage is enforced; `Cargo.toml`, `apps/canary/Cargo.toml`, and `falsifiers/external-null-cache/Cargo.toml` contain no coverage tool or threshold configuration.

**Current executable inventory:**
- The root workspace currently has 13 enabled tests: five public-facade tests in `crates/fava/tests/local_source_merge.rs`, three evaluator tests in `crates/fava-query-standard/tests/source_merge.rs`, three observation tests in `crates/fava-observe/src/lib.rs`, one cache atomicity test in `crates/fava-event-cache-memory/src/lib.rs`, and one coordinate test in `crates/fava-state/src/lib.rs`.
- The canary workspace currently has five tests across `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/recon.rs`.
- The external-provider workspace currently has one assembly test in `falsifiers/external-null-cache/src/lib.rs`.
- All 19 enumerated tests, root formatting, root Clippy, canary Clippy, and falsifier Clippy passed during this 2026-08-20 analysis using the commands above (`Cargo.toml`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`).

**View Coverage:**
```bash
# Not configured: no cargo-llvm-cov or tarpaulin setup is present in Cargo.toml.
cargo test --workspace --all-targets
```

- Treat behavioral and architectural coverage as traceable promises plus causal falsifiers, not a substitute line percentage (`features/local-source-merge.feature`, `features/relay-lab.feature`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

## Test Types

**Unit Tests:**
- Use co-located unit tests for one value, atomic mutation, builder invariant, lifecycle refusal, or deterministic helper (`crates/fava-state/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/artifacts.rs`).

**Integration Tests:**
- Use owner integration tests for merge semantics and source authority in `crates/fava-query-standard/tests/source_merge.rs`.
- Use public Rust acceptance tests through `Fava::builder` and `Fava::observe` in `crates/fava/tests/local_source_merge.rs`; these are the executable targets referenced by `features/local-source-merge.feature`.
- Use `falsifiers/external-null-cache/src/lib.rs` for outside-workspace provider assembly, which proves one replaceability boundary but is not yet a shared provider-conformance corpus (`docs/spec/ARCHITECTURE.md`).

**E2E Tests:**
- The enabled `lab-real-relay-smoke` scenario launches `nostr-rs-relay` 0.8.12 as a child process, publishes and queries through real WebSockets, hard-kills/restarts it with the same data directory, and preserves independent evidence (`apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/wire.rs`, `features/relay-lab.feature`).
- The live M0 green run and its fresh-data-directory mutation are recorded in `docs/issues/0002-m0-evidence-foundation.md`; this mapping ran the canary's five repeatable tests but did not rerun the external relay scenario (`apps/canary/README.md`).
- Public relay access is reconnaissance only and requires an explicit URL; it is not an E2E correctness gate (`apps/canary/src/recon.rs`, `apps/canary/scenarios.json`).

**Property / Model / Differential Tests:**
- These are required for algebra, broad input spaces, operation orders, safety, and planner/evaluator equivalence by `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- No property-test framework, model-test target, or differential-test target is currently declared in `Cargo.toml`; current executable evidence is example-based in `crates/fava-query-standard/tests/source_merge.rs` and `crates/fava/tests/local_source_merge.rs`.

**Native / Parity Tests:**
- Rust, Swift, and Kotlin parity plus real platform-process proof are required by `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`.
- No Swift, Kotlin, iOS, Android, parity-corpus, or native-capstone test tree is currently present under `crates/` or `apps/`; these are later milestone requirements in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Common Patterns

**Async Testing:**

Bound every wait and fail with a causal message. This helper comes from `crates/fava/tests/local_source_merge.rs`:

```rust
async fn next_snapshot(feed: &mut fava_observe::Observation) -> Arc<fava::QuerySnapshot> {
    timeout(Duration::from_secs(1), feed.changed())
        .await
        .expect("observation update arrives within bound")
        .expect("observation remains open")
}
```

- Use Tokio watch channels for bounded latest-state delivery and test the slow-consumer result, not every intermediate mutation (`crates/fava-observe/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`, `docs/spec/partial-spec-api-semantics.md`).
- Use controlled deadlines and readiness polling with explicit process checks; the 25 ms sleep in `apps/canary/src/relay.rs` is polling inside a ten-second deadline, while the proof is the successful TCP connection or child exit, not elapsed sleep (`apps/canary/src/relay.rs`).

**Error Testing:**

Match the typed refusal and separately assert cleanup. This current pattern comes from `crates/fava-observe/src/lib.rs`:

```rust
let result = observer.open(EventQuery::events().cache_only());

assert!(matches!(
    result,
    Err(ObserveError::SourceOpen {
        role: SourceKind::WriteStore,
        ..
    })
));
assert_eq!(closes.load(Ordering::SeqCst), 1);
```

- Use `expect_err` plus message inspection only for the canary's intentionally string-backed orchestration error (`apps/canary/src/recon.rs`).
- For atomicity, provoke a bounded refusal and then assert the prior state remains intact (`crates/fava-event-cache-memory/src/lib.rs`).

## Required Evidence Discipline

The normative workflow is owned by `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, with milestone gates in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md` and architecture falsifiers in `docs/spec/ARCHITECTURE.md`.

| Required proof | Current repository evidence |
|---|---|
| Behavior text precedes implementation and names the owner | `features/local-source-merge.feature`, `features/relay-lab.feature`, `docs/issues/0001-local-source-merge.md`, and `docs/issues/0002-m0-evidence-foundation.md` name current behaviors and owners. |
| Smallest causal red test, then green owner test | Red/green commands are recorded in `docs/issues/0001-local-source-merge.md` and `docs/issues/0002-m0-evidence-foundation.md`; executable owner tests live in `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava-observe/src/lib.rs`, and `crates/fava-event-cache-memory/src/lib.rs`. |
| Public capstone only when it proves composition | Five `@acceptance` scenarios in `features/local-source-merge.feature` map to public `fava` tests in `crates/fava/tests/local_source_merge.rs`. |
| Every claimed invariant has a mechanism-disable check | Eight local-source scenarios name falsifiers in `features/local-source-merge.feature`; `docs/issues/0001-local-source-merge.md` records a deliberate-break pass for the implemented subset. |
| External effects use an independent witness | `apps/canary/src/proxy.rs`, `apps/canary/src/wire.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/artifacts.rs` witness M0 wire, process, restart, and artifact facts. |
| Every replaceable provider boundary has a public conformance kit and different implementation | `falsifiers/external-null-cache/src/lib.rs` supplies one external event-cache implementation, but no `fava-*-testkit` crate from the required inventory in `docs/spec/ARCHITECTURE.md` exists in the current `crates/` tree. |
| Broad algebra and schedules use property/model/differential proof | `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` requires these layers; current `Cargo.toml` and `crates/*/tests/` contain only example-based tests. |
| Required checks are automated consistently | Commands are recorded in `docs/issues/0001-local-source-merge.md` and `docs/issues/0002-m0-evidence-foundation.md`, but no `.github/workflows/` or other repository CI pipeline is present. |

## Deliberate-Break Expectations

- Before claiming new or changed evidence, disable, bypass, reverse, or remove the mechanism named by the behavior's falsifier and confirm the linked test fails for that reason (`AGENTS.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Restore the protection and rerun the focused owner test, changed-crate tests, affected public capstone, and required formatting/lint gates (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `docs/issues/0001-local-source-merge.md`).
- A deliberate break may be a local patch, an owner-controlled test seam, or a proxy/lab mutation, but it must never become an application-facing production flag (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `AGENTS.md`).
- The current local-source falsifiers cover source omission, evidence merging, replacement selection, source authority, open cleanup, scoped closure, and bounded latest-state delivery (`features/local-source-merge.feature`).
- The current M0 falsifier restarts against fresh storage and requires the post-restart exact query to reach EOSE without the event, making persistence evidence fail causally (`features/relay-lab.feature`, `docs/issues/0002-m0-evidence-foundation.md`).
- Do not treat a green test as evidence if it stays green under the named break, if setup inserted the conclusion, or if the failure is an unrelated panic/setup error (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

---

*Testing analysis: 2026-08-20*
