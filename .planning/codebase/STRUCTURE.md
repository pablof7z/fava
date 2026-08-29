# Codebase Structure

**Analysis Date:** 2026-08-21

## Directory Layout

```text
nnn/
├── AGENTS.md                         # Delivery and architecture rules
├── README.md                         # Current product orientation
├── Cargo.toml                        # 34-crate main workspace and shared deps/lints
├── Cargo.lock                        # Locked main-workspace dependency graph
├── rust-toolchain.toml               # Rust 1.90.0 + Clippy + rustfmt
├── rustfmt.toml                      # Edition-2024 formatting policy
├── BUILD.bazel                       # Root Cargo exports and format alias
├── MODULE.bazel                      # Bazel/rules_rust/crate_universe configuration
├── crates/
│   ├── fava-state/                   # Relay evidence and event-state rules
│   ├── fava-write/                   # Event, intent, publication, receipt values
│   ├── fava-query/                   # Query/source/result contracts
│   ├── fava-query-standard/          # Reference query evaluator
│   ├── fava-event-cache/             # Event-cache contract
│   ├── fava-event-cache-memory/      # Bounded memory event cache
│   ├── fava-write-store/             # Write-store contract
│   ├── fava-write-store-memory/      # Volatile write-store provider
│   ├── fava-write-store-redb/        # Durable Redb write-store provider
│   ├── fava-observe/                 # Query observation owner
│   ├── fava-wire/                    # NIP-01 wire values and codec
│   ├── fava-subscriptions/           # Subscription-planner contract
│   ├── fava-subscriptions-no-grouping/ # One REQ per demand
│   ├── fava-subscriptions-standard/  # Bounded compatible grouping
│   ├── fava-transport/               # Relay transport/session contract
│   ├── fava-transport-websocket/     # WebSocket implementation
│   ├── fava-transport-testkit/       # Shared transport conformance fixture
│   ├── fava-ingest/                  # Relay EVENT admission owner
│   ├── fava-diagnostics/             # Bounded public diagnostic facts
│   ├── fava-routing/                 # Ordered read/write router chain
│   ├── fava-router-app-relays/       # App-selected relay policy
│   ├── fava-router-fallback-relays/  # Reactive fallback policy
│   ├── fava-router-outbox/           # NIP-65 outbox/inbox policy
│   ├── fava-router-hints/            # Nostr hint/evidence policy
│   ├── fava-router-testkit/          # Delayed routing fixture
│   ├── fava-nip65/                   # Pure NIP-65 relay-list semantics
│   ├── fava-signer/                  # Signer contract
│   ├── fava-signer-local/            # Local-key signer
│   ├── fava-publisher/               # One-attempt publisher contract
│   ├── fava-publisher-nip01/         # NIP-01 publisher
│   ├── fava-delivery/                # Delivery-decision contract
│   ├── fava-delivery-standard/       # Bounded standard delivery policy
│   ├── fava-publication/             # Durable publication lifecycle owner
│   └── fava/                         # Thin facade and public integration tests
├── apps/
│   └── canary/                       # Separate downstream acceptance/evidence workspace
├── falsifiers/
│   └── external-null-cache/          # Separate public-boundary substitution proof
├── features/                         # M0-M6 app-visible BDD behavior
├── docs/
│   ├── spec/                         # Authoritative clean-room specifications
│   ├── internals/                    # Closed vocabulary registry
│   └── issues/                       # Focused implementation/evidence ledger
├── tools/                            # Vocabulary enforcement and tests
└── .planning/
    ├── codebase/                     # GSD current-state maps
    ├── research/                     # Generated planning research
    ├── PROJECT.md                    # Project context
    ├── REQUIREMENTS.md               # Requirement ledger
    ├── ROADMAP.md                    # Phase roadmap
    └── STATE.md                      # Current GSD state
```

The main workspace contains 34 crates listed in `Cargo.toml`. Every main-workspace crate has a
co-located `Cargo.toml` and `BUILD.bazel`. `apps/canary/Cargo.toml` and
`falsifiers/external-null-cache/Cargo.toml` each declare their own `[workspace]`, so they compile
as separate downstream boundaries and do not appear in root Bazel packages.

The tracked implementation covers M0-M6. Current crate membership contains no M7-M11 owners such as
`crates/fava-nip02/`, `crates/fava-auth/`, `crates/fava-fetch-cache/`, `crates/fava-nip05/`,
`crates/fava-nip11/`, `crates/fava-session/`, `crates/fava-runtime/`, or `crates/fava-ffi/`.

## Directory Purposes

**`crates/`:**
- Purpose: Holds reusable product values, contracts, implementations, lifecycle owners, and facade.
- Contains: 34 Rust library crates declared explicitly in root `Cargo.toml`.
- Key files: `Cargo.toml`, `crates/fava/src/lib.rs`, `crates/fava-publication/src/run.rs`,
  `crates/fava-routing/src/chain.rs`.

**Domain/value crates:**
- Purpose: Own stable meanings and deterministic rules before provider mechanisms.
- Contains: `crates/fava-state/`, `crates/fava-write/`, `crates/fava-query/`,
  `crates/fava-wire/`, and `crates/fava-nip65/`.
- Key files: `crates/fava-state/src/lib.rs`, `crates/fava-write/src/lib.rs`,
  `crates/fava-query/src/lib.rs`, `crates/fava-wire/src/lib.rs`,
  `crates/fava-nip65/src/lib.rs`.

**Storage contracts and providers:**
- Purpose: Keep signed relay-event retention separate from accepted local write custody.
- Contains: `crates/fava-event-cache/`, `crates/fava-event-cache-memory/`,
  `crates/fava-write-store/`, `crates/fava-write-store-memory/`, and
  `crates/fava-write-store-redb/`.
- Key files: `crates/fava-event-cache/src/lib.rs`,
  `crates/fava-event-cache-memory/src/lib.rs`, `crates/fava-write-store/src/lib.rs`,
  `crates/fava-write-store-memory/src/lib.rs`, `crates/fava-write-store-redb/src/ops.rs`.

**Query and observation:**
- Purpose: Evaluate independent complete sources and deliver bounded latest state.
- Contains: `crates/fava-query-standard/`, `crates/fava-observe/`, and source/result contracts
  in `crates/fava-query/`.
- Key files: `crates/fava-query-standard/src/lib.rs`,
  `crates/fava-query-standard/tests/source_merge.rs`, `crates/fava-observe/src/lib.rs`.

**Relay planning and execution:**
- Purpose: Separate NIP-01 wire values, subscription planning, session resources, and verified ingest.
- Contains: `crates/fava-wire/`, `crates/fava-subscriptions/`, both planner providers,
  `crates/fava-transport/`, `crates/fava-transport-websocket/`,
  `crates/fava-transport-testkit/`, and `crates/fava-ingest/`.
- Key files: `crates/fava-subscriptions/src/lib.rs`,
  `crates/fava-subscriptions-standard/src/lib.rs`, `crates/fava-transport/src/lib.rs`,
  `crates/fava-transport-websocket/src/lib.rs`, `crates/fava-ingest/src/lib.rs`.

**Routing contracts and providers:**
- Purpose: Derive live relay destinations for reads and writes without owning wire subscriptions.
- Contains: `crates/fava-routing/`, app-relay, fallback, outbox, hint, and testkit router crates,
  plus pure `crates/fava-nip65/` parsing.
- Key files: `crates/fava-routing/src/lib.rs`, `crates/fava-routing/src/chain.rs`,
  `crates/fava-router-outbox/src/lib.rs`, `crates/fava-router-hints/src/lib.rs`.

**Publication contracts, providers, and owner:**
- Purpose: Separate signing, one-attempt publishing, delivery policy, and durable lifecycle ordering.
- Contains: `crates/fava-signer/`, `crates/fava-signer-local/`, `crates/fava-publisher/`,
  `crates/fava-publisher-nip01/`, `crates/fava-delivery/`,
  `crates/fava-delivery-standard/`, and `crates/fava-publication/`.
- Key files: `crates/fava-signer/src/lib.rs`, `crates/fava-publisher/src/lib.rs`,
  `crates/fava-delivery/src/lib.rs`, `crates/fava-publication/src/lib.rs`,
  `crates/fava-publication/src/run.rs`.

**`crates/fava/`:**
- Purpose: Expose public assembly and operations while delegating mutable lifecycle to owners.
- Contains: Facade/builder in `crates/fava/src/lib.rs`; query coordination in
  `crates/fava/src/live.rs`, `crates/fava/src/relay.rs`, and `crates/fava/src/routes.rs`;
  nested source adapter in `crates/fava/src/query_source.rs`; public tests in `crates/fava/tests/`.
- Key files: `crates/fava/src/lib.rs`, `crates/fava/Cargo.toml`, `crates/fava/BUILD.bazel`.

**`apps/canary/`:**
- Purpose: Act as an ordinary downstream Rust product plus independent process/wire evidence lab.
- Contains: Separate manifest/lockfile, registry, CLI, M0-M6 scenario modules, relay supervisor,
  proxy, independent wire witness, hostile fixtures, crash child, and artifact writer.
- Key files: `apps/canary/Cargo.toml`, `apps/canary/scenarios.json`,
  `apps/canary/src/main.rs`, `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`.

**`falsifiers/`:**
- Purpose: Challenge public replaceability from outside the main Cargo workspace.
- Contains: One independent null event-cache proof.
- Key files: `falsifiers/external-null-cache/Cargo.toml`,
  `falsifiers/external-null-cache/src/lib.rs`.

**`features/`:**
- Purpose: Preserve durable app-visible behavior and named falsifiers independently from crate layout.
- Contains: Nine `.feature` files covering relay lab, local source merge, explicit live query,
  multi-relay observation, routing/planning, publication, and write recovery.
- Key files: `features/relay-lab.feature`, `features/local-source-merge.feature`,
  `features/explicit-live-query.feature`, `features/automatic-publication.feature`.

**`docs/spec/`:**
- Purpose: Hold authoritative clean-room behavior, architecture, testing, delivery, and query semantics.
- Contains: The five authorities indexed by `docs/spec/README.md`.
- Key files: `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`,
  `docs/spec/ARCHITECTURE.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`,
  `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, `docs/spec/partial-spec-api-semantics.md`.

**`docs/internals/`:**
- Purpose: Define the closed architecture vocabulary used by code, specs, and planning.
- Contains: Registry documentation and TOML definitions.
- Key files: `docs/internals/README.md`, `docs/internals/vocabulary.toml`.

**`docs/issues/`:**
- Purpose: Record focused implementation status, ownership, proof, and deliberate breaks without
  adding status narration to normative specifications.
- Contains: Completed M0-M6 records in `docs/issues/0001-local-source-merge.md` through
  `docs/issues/0008-automatic-write-routing.md`; planning reconciliation is a separate local slice.
- Key files: `docs/issues/0004-explicit-live-query.md`,
  `docs/issues/0007-durable-explicit-publication.md`,
  `docs/issues/0008-automatic-write-routing.md`.

**`tools/`:**
- Purpose: Enforce architectural vocabulary against Rust and documentation surfaces.
- Contains: Python checker and fixture-based unit tests.
- Key files: `tools/check_vocabulary.py`, `tools/tests/test_vocabulary_check.py`.

**`.planning/`:**
- Purpose: Hold GSD project, requirements, roadmap, state, research, and codebase maps.
- Contains: `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`,
  `.planning/STATE.md`, `.planning/codebase/`, and `.planning/research/`.
- Key files: `.planning/codebase/ARCHITECTURE.md`, `.planning/codebase/STRUCTURE.md`.

## Key File Locations

**Entry Points:**
- `crates/fava/src/lib.rs`: Public Rust builder and application operations.
- `apps/canary/src/main.rs`: Canary command/exit boundary.
- `apps/canary/src/lib.rs`: Scenario registry and M0 relay-lab orchestration.
- `falsifiers/external-null-cache/src/lib.rs`: External provider substitution test.
- `tools/check_vocabulary.py`: Architectural vocabulary gate.

**Configuration:**
- `Cargo.toml`: Workspace membership, shared metadata, dependency aliases, and lints.
- `Cargo.lock`: Locked dependency graph consumed by Cargo and Bazel crate_universe.
- `crates/*/Cargo.toml`: Per-crate first-party dependency boundaries.
- `MODULE.bazel`: Bazel 9 module, `rules_rust` 0.73.0, Rust 1.90.0, and Cargo import.
- `MODULE.bazel.lock`: Resolved Bazel graph.
- `BUILD.bazel`: Root exports and `//:fmt` alias.
- `crates/*/BUILD.bazel`: Explicit first-party Bazel graph and tests.
- `.bazelrc`: Test, disk cache, output prefix, Clippy, and format aspects.
- `.bazeliskrc`: Bazelisk version pin.
- `.bazelignore`: Bazel package-discovery exclusions.
- `rust-toolchain.toml`: Rust 1.90.0 with Clippy and rustfmt.
- `rustfmt.toml`: Edition 2024 and 100-column formatting.
- `apps/canary/Cargo.toml`: Separate downstream application dependencies.
- `apps/canary/scenarios.json`: Current 22-enabled-scenario M0-M6 registry.
- `falsifiers/external-null-cache/Cargo.toml`: Separate public-contract proof dependencies.

**Core Logic:**
- `crates/fava-state/src/lib.rs`: Event state, deletion, expiration, and relay evidence.
- `crates/fava-write/src/lib.rs`: Intent, revision, receipt, and outcome values.
- `crates/fava-query/src/lib.rs`: Query, source, event-record, snapshot, and evaluator contracts.
- `crates/fava-observe/src/lib.rs`: Local merged observation lifecycle.
- `crates/fava-routing/src/lib.rs`: Routing values and provider contract.
- `crates/fava-routing/src/chain.rs`: Ordered asynchronous composition and bounds.
- `crates/fava-publication/src/run.rs`: Signing, routing, and delivery loop.
- `crates/fava/src/relay.rs`: Live session, attribution, reconnect, and withdrawal.

**Provider Contracts:**
- `crates/fava-event-cache/src/lib.rs`: Event-cache contract.
- `crates/fava-write-store/src/lib.rs`: Write-store contract.
- `crates/fava-subscriptions/src/lib.rs`: Subscription-planner contract.
- `crates/fava-transport/src/lib.rs`: Transport/session contract.
- `crates/fava-routing/src/lib.rs`: Router/session contract.
- `crates/fava-signer/src/lib.rs`: Signer contract.
- `crates/fava-publisher/src/lib.rs`: Publisher contract.
- `crates/fava-delivery/src/lib.rs`: Delivery-policy contract.

**Provider Implementations:**
- `crates/fava-event-cache-memory/src/lib.rs`: Memory event cache.
- `crates/fava-write-store-memory/src/lib.rs`: Memory write store.
- `crates/fava-write-store-redb/src/lib.rs`: Durable Redb write store.
- `crates/fava-query-standard/src/lib.rs`: Standard evaluator.
- `crates/fava-subscriptions-standard/src/lib.rs`: Grouping planner.
- `crates/fava-subscriptions-no-grouping/src/lib.rs`: One-per-demand planner.
- `crates/fava-transport-websocket/src/lib.rs`: WebSocket transport.
- `crates/fava-signer-local/src/lib.rs`: Local signer.
- `crates/fava-publisher-nip01/src/lib.rs`: NIP-01 publisher.
- `crates/fava-delivery-standard/src/lib.rs`: Standard delivery policy.
- `crates/fava-router-*/src/lib.rs`: Independently selected router policies.

**Testing:**
- `crates/fava/tests/`: Public facade tests for M1-M6.
- `crates/fava-query-standard/tests/source_merge.rs`: Shared source semantic corpus.
- `crates/fava-write-store-redb/tests/process_kill.rs`: Durable crash-boundary corpus.
- `crates/fava-transport-websocket/tests/conformance.rs`: Transport conformance.
- `crates/fava-subscriptions-standard/tests/grouping.rs`: Planner grouping equivalence.
- `crates/fava-router-outbox/tests/outbox.rs`: NIP-65 route acquisition.
- `apps/canary/src/`: Public real-process acceptance scenarios.
- `features/`: Product-readable BDD and falsifier statements.
- `docs/issues/`: Exact completion and mutation evidence.

## Naming Conventions

**Files:**
- Use `lib.rs` for library roots and `main.rs` only for executable roots, as in
  `crates/fava/src/lib.rs` and `apps/canary/src/main.rs`.
- Split cohesive submodules into snake_case files before crossing the 500-line soft limit, following
  `crates/fava/src/query_source.rs`, `crates/fava-publication/src/run.rs`, and
  `crates/fava-write-store-redb/src/ops.rs`.
- Use snake_case integration-test filenames under `tests/`, such as
  `crates/fava/tests/automatic_publication.rs`.
- Use kebab-case `.feature` names, such as `features/explicit-live-query.feature`.
- Use zero-padded issue numbers plus a kebab-case scope, such as
  `docs/issues/0008-automatic-write-routing.md`.
- Keep established uppercase authoritative spec filenames under `docs/spec/`.

**Directories:**
- Use kebab-case `fava-*` matching the Cargo package name.
- Keep contract and implementation directories separate as `fava-<role>/` and
  `fava-<role>-<implementation>/`, as in `crates/fava-transport/` and
  `crates/fava-transport-websocket/`.
- Put protocol meaning in a vocabulary-approved protocol crate, following `crates/fava-nip65/`.
- Put outside-workspace challenges below `falsifiers/<proof>/`; downstream products below
  `apps/<product>/`.

## Where to Add New Code

**New Feature:**
- Primary code: Change the single existing owner under `crates/fava-*/src/`; create an owner only
  when the active milestone requires it and the vocabulary approval gate permits the name.
- Tests: Add the first causal proof at the owner under `src/` or `tests/*.rs`; add
  `crates/fava/tests/*.rs` only for public cross-boundary composition.
- Product evidence: Update `features/<behavior>.feature`, register the capstone in
  `apps/canary/scenarios.json`, and use a cohesive `apps/canary/src/*.rs` module.
- Status: Add one focused numbered record under `docs/issues/`; never add implementation status to
  `docs/spec/`.

**New Component/Module:**
- Domain values: event/provenance in `crates/fava-state/src/`; write/receipt meaning in
  `crates/fava-write/src/`; query/source/result meaning in `crates/fava-query/src/`.
- Protocol meaning: use the vocabulary-approved protocol crate, following
  `crates/fava-nip65/src/lib.rs`; do not add NIP switches to universal crates.
- Provider contract: add or extend `crates/fava-<role>/` using domain values.
- Provider implementation: add `crates/fava-<role>-<implementation>/` in the same vertical slice;
  do not place the contract in its implementation crate or defer the split.
- Workspace/build: add both crates to root `Cargo.toml` and matching `BUILD.bazel` files with
  explicit first-party labels.
- Assembly: expose the neutral contract through `FavaBuilder`; keep concrete providers out of
  normal `crates/fava/Cargo.toml` dependencies.

**New Router:**
- Implementation: add `crates/fava-router-<policy>/` implementing `Router` from
  `crates/fava-routing/src/lib.rs`.
- Dependencies: depend on routing plus only needed domain/query sources; never add the router to
  `crates/fava-routing/Cargo.toml`.
- Tests: add owner tests and public ordered-chain evidence under `crates/fava/tests/` or canary.

**New Planner, Transport, Signer, Publisher, or Delivery Policy:**
- Reuse the corresponding neutral trait in `crates/fava-subscriptions/`, `crates/fava-transport/`,
  `crates/fava-signer/`, `crates/fava-publisher/`, or `crates/fava-delivery/`.
- Add a separately named implementation crate; keep provider-private resources there.
- Reuse/extend owner conformance, following `crates/fava-transport-testkit/` and
  `crates/fava-transport-websocket/tests/conformance.rs`.

**M7 Protocol Composition:**
- Add specified `crates/fava-nip02/` and a second unrelated protocol crate only when M7 begins;
  each depends on ordinary query/write values, not runtime, transport, stores, routers, or publisher.
- Put generic replaceable-event edit values with their approved semantic owner, durable edit storage
  behind `crates/fava-write-store/`, and reapplication in `crates/fava-publication/`.
- Prove first value, reapplication, inverse, stale generation, and protocol N+1 without changing
  NIP-specific behavior in `crates/fava/src/lib.rs`.

**Later Specified Owners:**
- M8: add vocabulary-approved auth/provider hardening contracts only with owning scenarios.
- M9: add persistent event-cache and fetch-cache contract/provider crates; keep NIP-05/NIP-11
  semantics in their protocol-service crates.
- M10: add alternatives outside default-provider crates for the provider matrix.
- M11: add FFI, Swift, and Kotlin products outside universal Rust core.

**Utilities:**
- Shared product helpers: keep with the semantic owner; do not create a generic common crate.
- Canary-only helpers: keep under `apps/canary/src/`.
- Architecture enforcement: keep repository checks under `tools/` with tests under `tools/tests/`.

## Special Directories

**`apps/canary/runs/`:**
- Purpose: Generated manifests, reports, JSONL, relay config/data/logs, process facts, resources,
  child runs, and proxy frames.
- Generated: Yes, by `apps/canary/src/artifacts.rs` and scenario modules.
- Committed: No; ignored by `.gitignore`.

**`target/` and nested `*/target/`:**
- Purpose: Cargo output for main, canary, and falsifier workspaces.
- Generated: Yes.
- Committed: No; ignored by `.gitignore`.

**`bazel-bin/`, `bazel-out/`, `bazel-testlogs/`, and `bazel-nnn/`:**
- Purpose: Bazel output symlinks created with the `bazel-` prefix from `.bazelrc`.
- Generated: Yes.
- Committed: No; ignored through `bazel-*`.

**`falsifiers/external-null-cache/`:**
- Purpose: Maintained external-workspace provider proof against public contracts.
- Generated: No.
- Committed: Yes, excluding `target/`.

**`docs/internals/`:**
- Purpose: Maintained architectural name/public symbol authority.
- Generated: No.
- Committed: Yes; changes require separate vocabulary approval.

**`.planning/codebase/`:**
- Purpose: GSD maps consumed by planning and execution.
- Generated: Yes.
- Committed: Orchestrator-controlled; mapper agents write but do not commit.

**`.planning/research/`:**
- Purpose: Generated planning research.
- Generated: Yes.
- Committed: No under current ignore policy.

**`.codex/`, `.pi/`, and `.claude/`:**
- Purpose: Local agent/GSD runtime material, not Fava product architecture.
- Generated: Tool-managed.
- Committed: No; ignored. Do not place product code here.

---

*Structure analysis: 2026-08-21*
