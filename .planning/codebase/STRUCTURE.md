# Codebase Structure

**Analysis Date:** 2026-08-20

## Directory Layout

```text
nnn/
├── AGENTS.md                         # Repository authority, delivery, and Rust constraints
├── Cargo.toml                        # Main Rust workspace and shared dependency/lint policy
├── Cargo.lock                        # Main workspace dependency lock
├── MODULE.bazel                      # Bazel module, Rust toolchain, and Cargo graph import
├── MODULE.bazel.lock                 # Resolved Bazel module dependency lock
├── BUILD.bazel                       # Root formatting target and exported Cargo metadata
├── .bazelrc                          # Authoritative build/test, cache, lint, and format settings
├── .bazeliskrc                       # Pinned Bazel release
├── .bazelignore                      # Nested Cargo/output/worktree package exclusions
├── rust-toolchain.toml               # Pinned Rust toolchain/components
├── rustfmt.toml                      # Workspace formatting policy
├── README.md                         # Product/status orientation
├── crates/                           # Main Fava library workspace
│   ├── fava-state/                   # Relay evidence and event-coordinate semantics
│   ├── fava-write/                   # Event/write/receipt values
│   ├── fava-query/                   # Query language, source/evaluator contracts, result values
│   ├── fava-query-standard/          # Reference full-reevaluation oracle
│   ├── fava-event-cache/             # Event-cache contract
│   ├── fava-event-cache-memory/      # Bounded memory event-cache provider
│   ├── fava-write-store/             # Write-store contract
│   ├── fava-write-store-memory/      # Bounded volatile write-store provider
│   ├── fava-observe/                 # Local live-query lifecycle owner
│   └── fava/                         # Thin facade and public integration tests
├── apps/
│   └── canary/                       # Separate downstream app/evidence workspace
├── falsifiers/
│   └── external-null-cache/          # Separate outside-workspace provider proof
├── features/                         # Durable app-visible BDD behavior
├── docs/
│   ├── spec/                         # Authoritative behavior/architecture/testing/plan inputs
│   └── issues/                       # Focused local slice status and evidence records
└── .planning/
    └── codebase/                     # Generated GSD codebase maps
```

Product structure is rooted by the main workspace members in `Cargo.toml`, with matching first-party Bazel packages such as `crates/fava-state/BUILD.bazel`, `crates/fava-query-standard/BUILD.bazel`, and `crates/fava/BUILD.bazel`. Bazel owns the main build/test entry through `.bazelrc`, while `MODULE.bazel` imports third-party dependency metadata from `Cargo.toml` and `Cargo.lock`. `apps/canary/Cargo.toml` and `falsifiers/external-null-cache/Cargo.toml` each declare `[workspace]`, so they are deliberately separate Cargo compilation boundaries and have no `BUILD.bazel` targets below `apps/` or `falsifiers/`.

## Directory Purposes

**`crates/`:**
- Purpose: Holds the implemented reusable Fava library slice and its neutral contracts/providers.
- Contains: One crate per value owner, provider contract, implementation, lifecycle owner, or facade listed in `Cargo.toml`.
- Key files: `crates/fava-state/src/lib.rs`, `crates/fava-write/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava/src/lib.rs`.

**`crates/fava-state/`:**
- Purpose: Own implemented event-state vocabulary and deterministic coordinate/winner semantics.
- Contains: One library source at `crates/fava-state/src/lib.rs` and package declaration at `crates/fava-state/Cargo.toml`.
- Key files: `crates/fava-state/src/lib.rs`.

**`crates/fava-write/`:**
- Purpose: Own implemented event values, stable write/receipt identifiers, and local publication evidence.
- Contains: One library source at `crates/fava-write/src/lib.rs` and package declaration at `crates/fava-write/Cargo.toml`.
- Key files: `crates/fava-write/src/lib.rs`.

**`crates/fava-query/`:**
- Purpose: Own the current declarative query API, source policy, provider-neutral source/evaluator traits, and application snapshots.
- Contains: Query values and contracts in `crates/fava-query/src/lib.rs` with dependencies in `crates/fava-query/Cargo.toml`.
- Key files: `crates/fava-query/src/lib.rs`, `docs/spec/partial-spec-api-semantics.md`.

**`crates/fava-query-standard/`:**
- Purpose: Supply the current simple semantic oracle and component evidence for source merging.
- Contains: Implementation in `crates/fava-query-standard/src/lib.rs`; integration corpus in `crates/fava-query-standard/tests/source_merge.rs`.
- Key files: `crates/fava-query-standard/src/lib.rs`, `crates/fava-query-standard/tests/source_merge.rs`.

**`crates/fava-event-cache/` and `crates/fava-write-store/`:**
- Purpose: Define separate neutral contracts for relay-observed cache state and accepted local materializations.
- Contains: Traits/errors in `crates/fava-event-cache/src/lib.rs` and `crates/fava-write-store/src/lib.rs`.
- Key files: `crates/fava-event-cache/Cargo.toml`, `crates/fava-write-store/Cargo.toml`.

**`crates/fava-event-cache-memory/` and `crates/fava-write-store-memory/`:**
- Purpose: Provide bounded current-process implementations of the two storage roles.
- Contains: Provider implementations and local unit tests in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
- Key files: `crates/fava-event-cache-memory/Cargo.toml`, `crates/fava-write-store-memory/Cargo.toml`.

**`crates/fava-observe/`:**
- Purpose: Own local live-query opening, source observation, reevaluation, coalesced delivery, and close.
- Contains: Owner and owner-level failure tests in `crates/fava-observe/src/lib.rs`.
- Key files: `crates/fava-observe/src/lib.rs`, `crates/fava-observe/Cargo.toml`.

**`crates/fava/`:**
- Purpose: Expose the thin public facade and validate explicit provider assembly.
- Contains: Facade in `crates/fava/src/lib.rs`; public integration evidence in `crates/fava/tests/local_source_merge.rs`.
- Key files: `crates/fava/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`, `crates/fava/Cargo.toml`.

**`apps/canary/`:**
- Purpose: Act as an ordinary downstream application and independent evidence lab, not as a Fava-internal test harness.
- Contains: Separate package/lockfile, CLI, scenario registry, relay supervision, transparent proxy, independent wire witness, reconnaissance, artifact assembly, and ignored run bundles under `apps/canary/runs/`.
- Key files: `apps/canary/Cargo.toml`, `apps/canary/src/main.rs`, `apps/canary/src/lib.rs`, `apps/canary/scenarios.json`, `apps/canary/README.md`.

**`falsifiers/`:**
- Purpose: Challenge replaceable public boundaries from outside the main Cargo workspace.
- Contains: The current null-cache provider and its assembly test in `falsifiers/external-null-cache/src/lib.rs`.
- Key files: `falsifiers/external-null-cache/Cargo.toml`, `falsifiers/external-null-cache/Cargo.lock`, `falsifiers/external-null-cache/src/lib.rs`.

**`features/`:**
- Purpose: Preserve app-visible behavior and named deliberate breaks independently from crate layout.
- Contains: M0 relay-lab behavior in `features/relay-lab.feature` and current M1 tracer behavior in `features/local-source-merge.feature`.
- Key files: `features/relay-lab.feature`, `features/local-source-merge.feature`.

**`docs/spec/`:**
- Purpose: Hold the authoritative clean-room behavior, architecture, testing, delivery, and supplemental API semantics.
- Contains: The five documents indexed in authority order by `docs/spec/README.md`.
- Key files: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `docs/spec/ARCHITECTURE.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`, `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `docs/spec/partial-spec-api-semantics.md`.

**`docs/issues/`:**
- Purpose: Record focused local slice outcomes, exclusions, proof, and incomplete gates without contaminating normative specs with status.
- Contains: M1 tracer record at `docs/issues/0001-local-source-merge.md` and completed M0 evidence record at `docs/issues/0002-m0-evidence-foundation.md`.
- Key files: `docs/issues/0001-local-source-merge.md`, `docs/issues/0002-m0-evidence-foundation.md`.

## Key File Locations

**Entry Points:**
- `crates/fava/src/lib.rs`: Public Rust library builder and `observe` facade.
- `apps/canary/src/main.rs`: Executable canary command parser and process exit boundary.
- `apps/canary/src/lib.rs`: Canary scenario registry and orchestration API.
- `falsifiers/external-null-cache/src/lib.rs`: External provider composition proof entry through its test.

**Configuration:**
- `Cargo.toml`: Main workspace membership, shared versions, dependency aliases, and lint policy.
- `MODULE.bazel`: Bazel 9 module graph, `rules_rust` 0.73.0, Rust 1.90.0 toolchain, supported `aarch64-apple-darwin` target, and Cargo/crate-universe import.
- `MODULE.bazel.lock`: Resolved Bazel module and extension dependency graph.
- `BUILD.bazel`: Root exports for `Cargo.toml` and `Cargo.lock` plus the public `//:fmt` alias.
- `.bazelrc`: Authoritative `bazel test //...` workflow, shared bounded disk cache, output symlink prefix, and Clippy/rustfmt aspect configurations.
- `.bazeliskrc`: Bazel 9.2.0 pin used by Bazelisk.
- `.bazelignore`: Excludes Cargo `target/` trees and nested `.claude/` worktrees from Bazel package discovery.
- `crates/fava/BUILD.bazel`: Public facade library target and `local_source_merge` acceptance-test target.
- `crates/fava-query-standard/BUILD.bazel`: Standard evaluator library and `source_merge` test target.
- `crates/fava-state/BUILD.bazel`: Representative leaf library target; corresponding higher-layer targets include `crates/fava-query/BUILD.bazel`, `crates/fava-observe/BUILD.bazel`, and `crates/fava/BUILD.bazel`.
- `rust-toolchain.toml`: Rust 1.90.0 toolchain with `clippy` and `rustfmt`.
- `rustfmt.toml`: Edition 2024, 100-column width, and shorthand formatting settings.
- `.gitignore`: Excludes root/nested `target/` directories and `apps/canary/runs/` evidence bundles.
- `apps/canary/Cargo.toml`: Separate canary package/runtime dependencies.
- `apps/canary/scenarios.json`: Canonical current scenario status registry.
- `falsifiers/external-null-cache/Cargo.toml`: Separate provider-falsifier workspace and public path dependencies.

**Core Logic:**
- `crates/fava-state/src/lib.rs`: Relay evidence and event-coordinate semantics.
- `crates/fava-write/src/lib.rs`: Local event/write/publication values.
- `crates/fava-query/src/lib.rs`: Query language, source contracts, result/evidence types, evaluator contract.
- `crates/fava-query-standard/src/lib.rs`: Standard source merge/evaluation oracle.
- `crates/fava-observe/src/lib.rs`: Live local observation lifecycle.
- `crates/fava/src/lib.rs`: Public assembly facade.

**Provider Contracts and Implementations:**
- `crates/fava-event-cache/src/lib.rs`: Event-cache contract.
- `crates/fava-event-cache-memory/src/lib.rs`: Memory event-cache provider.
- `crates/fava-write-store/src/lib.rs`: Write-store contract.
- `crates/fava-write-store-memory/src/lib.rs`: Memory write-store provider.

**Testing and Evidence:**
- `crates/fava-query-standard/tests/source_merge.rs`: Component source-merge corpus.
- `crates/fava/tests/local_source_merge.rs`: Public facade acceptance evidence for the local tracer.
- `crates/fava-observe/src/lib.rs`: Owner-level failure/closure tests co-located under `#[cfg(test)]`.
- `features/local-source-merge.feature`: App-readable M1 tracer behavior and falsifiers.
- `features/relay-lab.feature`: App-readable M0 process/wire behavior and falsifier.
- `apps/canary/src/`: Independent relay-lab implementation.
- `falsifiers/external-null-cache/src/lib.rs`: Public provider-substitution test.
- `docs/issues/0001-local-source-merge.md`: M1 tracer evidence and remaining gates.
- `docs/issues/0002-m0-evidence-foundation.md`: M0 evidence record.

## Naming Conventions

**Files:**
- Use `lib.rs` for each library crate root and `main.rs` only for an executable entry point, as in `crates/fava/src/lib.rs` and `apps/canary/src/main.rs`.
- Use snake_case Rust module filenames for cohesive canary responsibilities, as in `apps/canary/src/artifacts.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/wire.rs`, and `apps/canary/src/recon.rs`.
- Use snake_case integration-test filenames, as in `crates/fava-query-standard/tests/source_merge.rs` and `crates/fava/tests/local_source_merge.rs`.
- Use kebab-case behavior filenames ending in `.feature`, as in `features/local-source-merge.feature` and `features/relay-lab.feature`.
- Use zero-padded issue numbers plus a kebab-case slug, as in `docs/issues/0001-local-source-merge.md` and `docs/issues/0002-m0-evidence-foundation.md`.
- Keep authoritative spec filenames explicit and upper-case where established in `docs/spec/`; do not duplicate their rules into status documents, per `docs/spec/README.md`.

**Directories:**
- Use kebab-case `fava-*` crate directories that match Cargo package names, as in `crates/fava-query-standard/` and `crates/fava-event-cache-memory/`.
- Pair a neutral provider contract with separately named implementation directories, as in `crates/fava-event-cache/` plus `crates/fava-event-cache-memory/` and `crates/fava-write-store/` plus `crates/fava-write-store-memory/`.
- Place outside-workspace boundary challenges under a descriptive kebab-case child of `falsifiers/`, as in `falsifiers/external-null-cache/`.
- Place ordinary downstream acceptance products under `apps/`, as in `apps/canary/`; do not place them inside `crates/fava/`.

## Where to Add New Code

**New Vertical Feature:**
- Primary code: Add behavior at its single owner under the existing `crates/fava-*/src/` path, or add the target owner crate named by `docs/spec/ARCHITECTURE.md` only when its vertical slice begins in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- Tests: Put the first executable proof beside the owner (`src/lib.rs` under `#[cfg(test)]` or `tests/*.rs`), add public composition evidence under `crates/fava/tests/` only when it proves another boundary, and update app-visible behavior under `features/` according to `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.
- Status/evidence: Record focused implementation status and deliberate-break results in a new numbered file under `docs/issues/`; keep normative meaning in the owning file under `docs/spec/`.

**New Domain Value or Rule:**
- Implementation: Extend the owner, not a generic common crate: event-state rules in `crates/fava-state/src/`, event/write/receipt values in `crates/fava-write/src/`, and query/source/result meaning in `crates/fava-query/src/`.
- Tests: Add pure/property/model evidence under the same crate or its `tests/` directory, following placement guidance in `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.

**New Provider Boundary:**
- Contract: Create or extend one neutral `crates/fava-<role>/` contract using domain values from their owners, following existing patterns in `crates/fava-event-cache/` and `crates/fava-write-store/`.
- Implementation: Put each concrete algorithm/backend in its own `crates/fava-<role>-<implementation>/`, following `crates/fava-event-cache-memory/` and `crates/fava-write-store-memory/`.
- Falsifier: Add a meaningfully different provider outside the main workspace under `falsifiers/<provider-proof>/`, following `falsifiers/external-null-cache/`; do not stabilize a contract from one implementation alone, per `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.
- Assembly: Add path/version aliases and a workspace member in root `Cargo.toml`; add concrete providers to `crates/fava/Cargo.toml` only when intentionally required by facade code or test-only dev dependencies.
- Build target: Add `crates/fava-<role>/BUILD.bazel` beside the new crate manifest, declare first-party dependencies as explicit `//crates/<package>:lib` labels, and source third-party dependencies through `@crates` as demonstrated by `crates/fava-event-cache-memory/BUILD.bazel`.

**New Query Evaluator:**
- Contract: Reuse `QueryEvaluator` in `crates/fava-query/src/lib.rs` unless product meaning changes.
- Standard implementation: Extend the oracle in `crates/fava-query-standard/src/lib.rs` and its corpus in `crates/fava-query-standard/tests/`.
- Alternative implementation: Add a separate provider crate under `crates/fava-query-<implementation>/` and prove differential equivalence against `crates/fava-query-standard/`, as required by `docs/spec/ARCHITECTURE.md`.

**New Lifecycle Owner:**
- Implementation: Add the focused owner crate named by the relevant target slice in `docs/spec/ARCHITECTURE.md`, rather than adding policy to `crates/fava/src/lib.rs` or execution policy to `crates/fava-observe/src/lib.rs`.
- Facade: Expose only the thin application operation/handle in `crates/fava/src/lib.rs`; keep state and lifecycle in its owner crate.
- Tests: Put schedule/failure tests at the owner and a public capstone in `crates/fava/tests/` or `apps/canary/` only when cross-boundary behavior requires it, following `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.

**New Canary Scenario:**
- Registry: Add status/requirements to `apps/canary/scenarios.json`.
- CLI dispatch: Add the user command boundary to `apps/canary/src/main.rs` only if the scenario needs a new command shape.
- Orchestration: Add scenario control to `apps/canary/src/lib.rs` and reuse focused modules under `apps/canary/src/`; add a new module only for a cohesive responsibility.
- Evidence: Reuse `apps/canary/src/artifacts.rs`, proxy/witness facilities in `apps/canary/src/proxy.rs` and `apps/canary/src/wire.rs`, and ignored output under `apps/canary/runs/`.
- Behavior: Add/update the owning scenario under `features/` and link its exact executable evidence as prescribed by `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`.

**Utilities:**
- Shared domain helpers: Keep them with the owner in `crates/fava-state/src/`, `crates/fava-write/src/`, or `crates/fava-query/src/`; do not create a generic `common` bucket, per `docs/spec/ARCHITECTURE.md`.
- Canary-only helpers: Keep them within focused modules under `apps/canary/src/`, as demonstrated by `apps/canary/src/artifacts.rs` and `apps/canary/src/wire.rs`.

## Special Directories

**`apps/canary/runs/`:**
- Purpose: Stores per-run manifests, reports, JSONL evidence, logs, resources, relay data/configuration, and proxy frames created by `apps/canary/src/artifacts.rs`.
- Generated: Yes; `apps/canary/src/lib.rs` creates it during canary/reconnaissance runs.
- Committed: No; ignored by `.gitignore`.

**`target/` and nested `*/target/`:**
- Purpose: Cargo build output for the main and separate workspaces.
- Generated: Yes; manifests are `Cargo.toml`, `apps/canary/Cargo.toml`, and `falsifiers/external-null-cache/Cargo.toml`.
- Committed: No; ignored by `.gitignore`.

**`bazel-bin/`, `bazel-out/`, `bazel-testlogs/`, and `bazel-nnn/`:**
- Purpose: Bazel convenience symlinks into generated build, test, and execution-root output owned by `.bazelrc` and the Bazel workspace named in `MODULE.bazel`.
- Generated: Yes; Bazel creates these paths when root targets such as `//crates/fava:lib` or `//crates/fava:local_source_merge` are built or tested.
- Committed: No; `bazel-bin/`, `bazel-out/`, `bazel-testlogs/`, and `bazel-nnn/` are currently untracked build symlinks, and `.gitignore` does not currently exclude them.

**`falsifiers/external-null-cache/`:**
- Purpose: Compile a provider outside the main workspace boundary against public Fava contracts.
- Generated: No; source is maintained at `falsifiers/external-null-cache/src/lib.rs`.
- Committed: Yes; package, lockfile, and source are tracked under `falsifiers/external-null-cache/`.

**`apps/canary/`:**
- Purpose: Preserve a real downstream/evidence boundary independent of Fava internals.
- Generated: No; sources are maintained under `apps/canary/src/`.
- Committed: Yes, except generated `apps/canary/runs/` and build output excluded by `.gitignore`.

**`.planning/codebase/`:**
- Purpose: Holds generated GSD maps consumed by later planning/execution commands.
- Generated: Yes; this map writes `.planning/codebase/ARCHITECTURE.md` and `.planning/codebase/STRUCTURE.md`.
- Committed: Orchestrator-controlled; mapper agents do not stage or commit `.planning/codebase/`.

**`.codex/`, `.pi/`, and `.claude/`:**
- Purpose: Local agent/GSD runtime material and worktrees, not Fava product architecture.
- Generated: Locally installed or tool-managed; product code remains under `crates/`, `apps/`, `falsifiers/`, `features/`, and `docs/`.
- Committed: Not part of tracked product source in the current checkout; do not place Fava implementation code in `.codex/`, `.pi/`, or `.claude/`.
