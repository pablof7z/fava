# Testing Patterns

**Analysis Date:** 2026-08-21

## Test Framework

**Runner:**
- Rust's built-in Cargo test harness under the pinned Rust 1.90.0 toolchain runs workspace, standalone canary, and external-provider tests (`rust-toolchain.toml`, `Cargo.toml`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`).
- Tokio 1.53.1 supplies asynchronous tests; public-facade and observation tests use `#[tokio::test(flavor = "current_thread")]` for deterministic single-thread scheduling (`Cargo.toml`, `crates/fava/tests/local_source_merge.rs`, `crates/fava-observe/src/lib.rs`).
- No separate unit-test configuration file is present; test targets are discovered from `#[cfg(test)]` modules and `tests/*.rs` according to Cargo conventions (`Cargo.toml`, `crates/fava-query-standard/tests/source_merge.rs`).
- Bazel is the authoritative repository build/test surface. `rust_test` targets mirror the Rust evidence corpus in per-crate `BUILD.bazel` files; lint and format checks are aspects configured in `.bazelrc` (`MODULE.bazel`, `crates/fava/BUILD.bazel`).

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

cargo test -p fava --test automatic_publication known_destinations_deliver_now_and_later_route_uses_same_receipt -- --exact
# Run one focused M6 public-facade acceptance test.

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
bazel test //...
bazel build //... --config=clippy
bazel build //... --config=fmt-check
python3 tools/check_vocabulary.py
python3 -m unittest tools/tests/test_vocabulary_check.py
# Repository-wide build, lint, format, and architecture-vocabulary gates.
```

- No watch-mode command is configured in `Cargo.toml`; rerun the focused `cargo test` command while developing the owning behavior (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Deterministic live scenarios run separately with `cargo run --manifest-path apps/canary/Cargo.toml -- run <scenario-id> --seed <unique-seed>` after installing the pinned `nostr-rs-relay` 0.8.12 prerequisite. Enabled scenarios fail when prerequisites are absent; public-relay reconnaissance is explicit and non-gating (`apps/canary/README.md`, `apps/canary/scenarios.json`, `apps/canary/src/main.rs`).

## Test File Organization

**Location:**
- Co-locate narrow unit tests at the bottom of the owning source file in `#[cfg(test)] mod tests`, as in `crates/fava-state/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- Put crate-level integration tests under the owner's `tests/` directory: evaluator evidence is in `crates/fava-query-standard/tests/source_merge.rs`, transport conformance is in `crates/fava-transport-websocket/tests/conformance.rs`, and crash evidence is in `crates/fava-write-store-redb/tests/process_kill.rs`.
- Keep architectural substitution proof outside the main workspace in `falsifiers/external-null-cache/src/lib.rs`; this proves public contracts are sufficient without private access (`falsifiers/external-null-cache/Cargo.toml`, `docs/spec/ARCHITECTURE.md`).
- Keep the ordinary downstream process/wire evidence application in its own workspace at `apps/canary/`. It explicitly assembles public Fava contracts/providers, owns its scenario runners, and uses independent proxy/process/wire witnesses for external effects (`apps/canary/Cargo.toml`, `apps/canary/src/lib.rs`, `apps/canary/src/proxy.rs`).
- Keep readable application behavior under `features/`; Gherkin is product memory and does not require a Cucumber runner (`features/local-source-merge.feature`, `features/relay-lab.feature`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

**Naming:**
- Name unit and integration tests as snake_case behavioral claims: `failed_capacity_batch_is_atomic`, `reconnect_uses_fresh_identity_and_rejects_old_subscription_frames`, and `known_destinations_deliver_now_and_later_route_uses_same_receipt` (`crates/fava-event-cache-memory/src/lib.rs`, `crates/fava/tests/multi_relay.rs`, `crates/fava/tests/automatic_publication.rs`).
- Name integration-test files and feature files after the coherent behavior slice, not the implementation crate: `crates/fava/tests/automatic_publication.rs` and `features/automatic-publication.feature` cover the same M6 product distinction at different evidence layers.

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

Active examples span M1-M6: `crates/fava/tests/local_source_merge.rs`, `crates/fava/tests/explicit_live.rs`, `crates/fava/tests/multi_relay.rs`, `crates/fava/tests/automatic_routes.rs`, `crates/fava/tests/explicit_publication.rs`, `crates/fava/tests/automatic_publication.rs`, `crates/fava-write-store-redb/tests/process_kill.rs`, and `apps/canary/scenarios.json`.

## Test Structure

**Suite Organization:**

Use small fixture helpers, arrange real causal inputs, invoke the owning public operation, and assert both partial progress and terminal identity. This abbreviated pattern is from `crates/fava/tests/automatic_publication.rs`:

```rust
#[tokio::test(flavor = "current_thread")]
async fn known_destinations_deliver_now_and_later_route_uses_same_receipt() {
    let preview = fava.preview_write_routes(&intent).expect("preview");
    assert!(!preview.settled);
    assert_eq!(delayed.open_count(), 0);
    assert_eq!(publisher.count(), 0);

    let accepted = fava.publish(intent).expect("accepted");
    wait_until(|| publisher.count() == 3).await;
    let partial = fava
        .receipt(accepted.receipt_id)
        .expect("receipt read")
        .expect("receipt exists");
    assert_eq!(partial.receipt_id, accepted.receipt_id);
    assert!(!partial.route_settled);
}
```

**Patterns:**
- Setup provides causes through supported constructors and operations, never by inserting the answer under proof; use event builders, provider commits, scripted relay frames, delayed router replacement contributions, and process launches as inputs (`crates/fava/tests/local_source_merge.rs`, `crates/fava/tests/automatic_routes.rs`, `apps/canary/src/lib.rs`).
- Keep the assertion at the smallest stable owner, then add a public-facade or canary capstone only when it proves an additional cross-boundary fact (`crates/fava-routing/src/chain.rs`, `crates/fava-router-outbox/tests/outbox.rs`, `crates/fava/tests/automatic_publication.rs`).
- Close or drop lifecycle handles and verify scoped closure when cleanup is part of the claim; observation tests count source closes and the canary owns process/proxy shutdown (`crates/fava-observe/src/lib.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/relay.rs`).
- Prefer exact IDs, evidence, revisions, and statuses over loose success flags; acceptance assertions inspect event IDs, subscription IDs, generations, receipt IDs, route revisions, relay outcomes, and source status (`crates/fava/tests/explicit_live.rs`, `crates/fava/tests/multi_relay.rs`, `crates/fava/tests/explicit_publication.rs`, `crates/fava/tests/automatic_publication.rs`).

## Mocking

**Framework:** No mocking framework is used; tests define small handwritten implementations of public contracts (`Cargo.toml`, `crates/fava-observe/src/lib.rs`, `falsifiers/external-null-cache/src/lib.rs`).

**Patterns:**

Use a narrow fake only to control the boundary under test. From `crates/fava-observe/src/lib.rs`:

```rust
struct RefusingSource;

impl QuerySource for RefusingSource {
    fn open(&self, _query: &Query) -> Result<OpenedQuerySource, QuerySourceError> {
        Err(QuerySourceError::Refused(
            "injected open failure".to_owned(),
        ))
    }
}
```

- `TrackingSource`, `TrackingChanges`, `RefusingSource`, `EmptyEvaluator`, and `FailingEvaluator` isolate open/close and evaluation behavior without a general-purpose mock layer (`crates/fava-observe/src/lib.rs`).
- `ScriptedTransport` and `ScriptedSession` inject exact NIP-01 frames and disconnects; `DelayedRouter` supplies controlled complete replacement contributions; `RecordingPublisher` captures exact publication attempts (`crates/fava/tests/explicit_live.rs`, `crates/fava-router-testkit/src/lib.rs`, `crates/fava/tests/automatic_publication.rs`).
- `NullEventCache` is a materially different provider compiled outside the root workspace and assembled through public contracts (`falsifiers/external-null-cache/src/lib.rs`, `falsifiers/external-null-cache/Cargo.toml`).
- The public-facade tests use real in-memory providers and the standard evaluator rather than mocking merge semantics (`crates/fava/tests/local_source_merge.rs`).
- The M0 canary uses a real pinned relay process plus a transparent WebSocket proxy rather than mocking persistence or external frame handoff (`apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/wire.rs`).

**What to Mock:**
- Fake a neutral provider contract when the claim is provider refusal, close, late completion, or failure isolation at that boundary (`crates/fava-observe/src/lib.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Inject deterministic clocks, barriers, relay frames, signer outcomes, router contributions, publisher outcomes, or failures when those are causes needed to control a distributed schedule (`crates/fava/tests/automatic_routes.rs`, `crates/fava/tests/explicit_publication.rs`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

**What NOT to Mock:**
- Do not mock the semantic owner whose decision is being proved or let the fixture calculate/insert the expected route, result, coverage, or receipt (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Do not use diagnostics to self-certify external effects; compare public results with wire, relay, process, filesystem, or platform witnesses (`apps/canary/src/proxy.rs`, `apps/canary/src/artifacts.rs`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`).
- Do not use uncontrolled public relays as a deterministic pass/fail oracle; public mode is read-only reconnaissance in `apps/canary/src/recon.rs` and `apps/canary/scenarios.json`.

## Fixtures and Factories

**Test Data:**

Use local helper functions that produce valid domain inputs with explicit times and identities. From `crates/fava-query-standard/tests/source_merge.rs`:

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
- Use fixed keys and explicit boundary names only when crash recovery needs stable cross-process identity; isolate each database under a unique temporary root (`crates/fava-write-store-redb/tests/process_kill.rs`).
- Use `tempfile::tempdir` for isolated filesystem tests and caller-selected unique seeds for preserved canary runs (`apps/canary/src/recon.rs`, `apps/canary/src/artifacts.rs`, `apps/canary/README.md`).
- Derive disposable identities deterministically from the scenario seed and isolate relay port/data/process state per run (`apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`).
- Preserve live evidence under the ignored `apps/canary/runs/` tree with a manifest, JSONL, reports, logs, resources, and hashes (`apps/canary/README.md`, `apps/canary/src/artifacts.rs`).

## Coverage

**Requirements:** No line or branch coverage percentage is enforced; `Cargo.toml`, `apps/canary/Cargo.toml`, and `falsifiers/external-null-cache/Cargo.toml` contain no coverage tool or threshold configuration.

**Current executable inventory:**
- `cargo test --workspace --all-targets -- --list` enumerates 66 root-workspace tests across owner units, provider conformance, public-facade acceptance, bounds, and process-kill durability (`Cargo.toml`, `crates/fava/BUILD.bazel`).
- `cargo test --manifest-path apps/canary/Cargo.toml -- --list` enumerates seven repeatable canary-harness tests across `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/recon.rs`.
- The external-provider workspace has one assembly test in `falsifiers/external-null-cache/src/lib.rs`.
- Per-crate Bazel files declare 24 `rust_test` targets, including the nine public-facade targets in `crates/fava/BUILD.bazel` and process-kill evidence in `crates/fava-write-store-redb/BUILD.bazel`.
- Completion records state each M0-M6 milestone's applicable Cargo/Bazel, strict Clippy, format, canary, falsifier, live-relay, durability, and vocabulary gates (`docs/issues/0002-m0-evidence-foundation.md`, `docs/issues/0001-local-source-merge.md`, `docs/issues/0004-explicit-live-query.md`, `docs/issues/0005-multi-relay-observation.md`, `docs/issues/0006-ordered-automatic-routing.md`, `docs/issues/0007-durable-explicit-publication.md`, `docs/issues/0008-automatic-write-routing.md`).

**View Coverage:**
```bash
# Not configured: no cargo-llvm-cov or tarpaulin setup is present in Cargo.toml.
cargo test --workspace --all-targets
```

- Treat behavioral and architectural coverage as built feature scenarios, owner evidence, public capstones, and causal falsifiers, not a substitute line percentage (`features/`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

## Test Types

**Unit Tests:**
- Use co-located unit tests for one value, parser, atomic mutation, builder invariant, lifecycle refusal, or policy decision (`crates/fava-state/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-nip65/src/lib.rs`, `crates/fava-delivery-standard/src/lib.rs`).

**Integration Tests:**
- Use owner integration tests for merge/source semantics, transport conformance, subscription equivalence, router policy, diagnostics, and crash recovery (`crates/fava-query-standard/tests/source_merge.rs`, `crates/fava-transport-websocket/tests/conformance.rs`, `crates/fava-subscriptions-standard/tests/grouping.rs`, `crates/fava-router-outbox/tests/outbox.rs`, `crates/fava-write-store-redb/tests/process_kill.rs`).
- Use public Rust acceptance tests through `Fava::builder`, `Fava::observe`, `Fava::publish`, and route preview in `crates/fava/tests/`; these map to built scenarios in `features/` for M1-M6.
- Use `falsifiers/external-null-cache/src/lib.rs` for outside-workspace provider assembly. Shared testkits now exist for transport and routing (`crates/fava-transport-testkit/`, `crates/fava-router-testkit/`), while complete cross-provider qualification remains M10 work (`docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`).

**E2E Tests:**
- The enabled `lab-real-relay-smoke` scenario launches `nostr-rs-relay` 0.8.12 as a child process, publishes and queries through real WebSockets, hard-kills/restarts it with the same data directory, and preserves independent evidence (`apps/canary/src/lib.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/wire.rs`, `features/relay-lab.feature`).
- M1-M6 register 21 additional enabled application scenarios for local merge, real relay query, reconnect, routing, subscription grouping, durable publication, crash recovery, and automatic delivery (`apps/canary/scenarios.json`). Their live evidence and deliberate breaks are recorded in `docs/issues/0001-local-source-merge.md` and `docs/issues/0004-explicit-live-query.md` through `docs/issues/0008-automatic-write-routing.md`.
- Public relay access is reconnaissance only and requires an explicit URL; it is not an E2E correctness gate (`apps/canary/src/recon.rs`, `apps/canary/scenarios.json`).

**Property / Model / Differential Tests:**
- These are required for algebra, broad input spaces, operation orders, safety, and planner/evaluator equivalence by `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md` and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- No property-test dependency such as `proptest` or `quickcheck` is declared in `Cargo.toml`. Deterministic differential/shared-corpus evidence exists for memory query sources and subscription planners (`crates/fava/tests/source_contract.rs`, `crates/fava-subscriptions-standard/tests/grouping.rs`, `crates/fava-subscriptions-no-grouping/tests/plan.rs`); controlled schedules and exact bounds are example-driven (`crates/fava/tests/observation_bounds.rs`, `crates/fava/tests/write_bounds.rs`).

**Native / Parity Tests:**
- Rust, Swift, and Kotlin parity plus real platform-process proof are specified for M11 by `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` and `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- No Swift, Kotlin, iOS, Android, parity-corpus, or native-capstone test tree is present. M7-M11 are specified only; `apps/canary/scenarios.json` currently ends at M6.

## Common Patterns

**Async Testing:**

Bound every wait and fail with a causal message. From `crates/fava/tests/local_source_merge.rs`:

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

Match the typed refusal and separately assert cleanup. From `crates/fava-observe/src/lib.rs`:

```rust
let result = observer.open(Query::events().cache_only());

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
| Behavior text names application meaning and owner | Every current `features/*.feature` file is built M0-M6 behavior; owner and capstone evidence are linked from `features/local-source-merge.feature` through `features/automatic-publication.feature`. |
| Smallest causal red, then green owner evidence | The M0-M6 issue records describe their red/falsifier and green evidence; owner tests live in semantic/provider crates such as `crates/fava-state/src/lib.rs`, `crates/fava-ingest/tests/admission.rs`, `crates/fava-routing/src/chain.rs`, and `crates/fava-write-store-redb/tests/process_kill.rs`. |
| Public capstone proves composition | Public `Fava` acceptance targets under `crates/fava/tests/` and 22 enabled canary scenarios cover implemented M0-M6 product slices (`crates/fava/BUILD.bazel`, `apps/canary/scenarios.json`). |
| Every milestone claim has a mechanism-disable check | Built feature metadata names one falsifier per behavior, and `docs/issues/0002-m0-evidence-foundation.md` plus `docs/issues/0001-local-source-merge.md` and `docs/issues/0004-explicit-live-query.md` through `docs/issues/0008-automatic-write-routing.md` record the milestone mutations. |
| External effects use independent witnesses | `apps/canary/src/proxy.rs`, `apps/canary/src/wire.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/artifacts.rs` witness frames, processes, restart, filesystem evidence, and hashes independently of internal Fava state. |
| Replaceable boundaries have reusable proof | `crates/fava-transport-testkit/` supplies transport conformance, `crates/fava-router-testkit/` supplies controlled routing, `crates/fava/tests/source_contract.rs` shares source behavior, and `falsifiers/external-null-cache/` proves outside-workspace cache assembly. Full seam-by-seam substitution remains specified M10 work. |
| Distributed schedules and bounds are controlled | `crates/fava/tests/observation_bounds.rs`, `crates/fava/tests/automatic_routes.rs`, `crates/fava/tests/write_bounds.rs`, and `crates/fava-write-store-redb/tests/process_kill.rs` control coalescing, delayed routing, atomic refusal, and process-death boundaries. |
| Required checks are repeatable | Cargo and Bazel commands are encoded in manifests and `.bazelrc` and recorded in milestone issues. `.github/workflows/architecture.yml` runs the vocabulary checker and its unit tests on pushes to `main` and pull requests; Cargo/Bazel test, lint, and format gates are not currently encoded in GitHub Actions. |
| Future scope is not claimed | M7-M11 appear only in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`; there are no M7+ registered canary scenarios, native tests, or later-profile qualification artifacts. |

## Deliberate-Break Expectations

- Before claiming new or changed evidence, disable, bypass, reverse, or remove the mechanism named by the behavior's falsifier and confirm the linked test fails for that reason (`AGENTS.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Restore the protection and rerun the focused owner test, changed-crate tests, affected public capstone, Cargo/Bazel gates, and vocabulary checks when architecture or public API changed (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `AGENTS.md`).
- A deliberate break may be a local patch, an owner-controlled test seam, or a proxy/lab mutation, but it must never become an application-facing production flag (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `AGENTS.md`).
- M0-M6 falsifiers cover fresh-storage restart, source/evidence merge, signature bypass, stale subscription attribution, delayed routing, fallback freeze, durable acceptance omission, outcome collapse, and waiting for route settlement (`features/relay-lab.feature`, `features/local-source-merge.feature`, `features/explicit-live-query.feature`, `features/multi-relay-observation.feature`, `features/automatic-routing.feature`, `features/explicit-publication.feature`, `features/automatic-publication.feature`).
- Do not treat a green test as evidence if it stays green under the named break, if setup inserted the conclusion, or if the failure is an unrelated panic/setup error (`docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).

---

*Testing analysis: 2026-08-21*
