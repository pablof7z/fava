# Technology Stack

**Analysis Date:** 2026-08-21

## Milestone Implementation Boundary

**Implemented and milestone-complete:**
- M0 through M6 have complete issue records in `docs/issues/0002-m0-evidence-foundation.md`, `docs/issues/0001-local-source-merge.md`, and `docs/issues/0004-explicit-live-query.md` through `docs/issues/0008-automatic-write-routing.md`.
- The executable surface contains the M0 evidence lab; M1 local source merge; M2 explicit live queries; M3 multi-relay reactivity; M4 ordered read routing and subscription planning; M5 durable explicit publication; and M6 automatic write routing with partial delivery. The public facade is `crates/fava/src/lib.rs`, and all enabled application scenarios are registered in `apps/canary/scenarios.json`.

**Specification-only:**
- M7 through M11 are defined in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`; they have no completed issue records, canary scenarios, or implementation crates in `Cargo.toml`.
- The root workspace contains no `fava-nip02`, second replaceable-edit protocol crate, `fava-auth`, `fava-fetch-cache`, `fava-nip05`, `fava-nip11`, persistent event-cache provider, external provider matrix, FFI, Swift, or Kotlin artifact. Do not plan against those specified boundaries as if they were implemented.

## Languages

**Primary:**
- Rust 1.90.0 / edition 2024 - all 34 engine, contract, provider, protocol, testkit, and facade crates declared in `Cargo.toml`; the downstream acceptance binary in `apps/canary/Cargo.toml`; and the external provider proof in `falsifiers/external-null-cache/Cargo.toml`.

**Secondary:**
- Starlark - Bazel module, root targets, and one first-party BUILD file per crate in `MODULE.bazel`, `BUILD.bazel`, and `crates/*/BUILD.bazel`.
- Python 3.13 in CI - the architectural vocabulary gate and unit corpus are `tools/check_vocabulary.py` and `tools/tests/test_vocabulary_check.py`; `.github/workflows/architecture.yml` installs Python 3.13.
- Gherkin - durable product behavior is recorded in `features/*.feature`; feature files are evidence catalogs, not a runtime Cucumber suite.
- TOML and JSON - Rust/build configuration lives in `Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, and `docs/internals/vocabulary.toml`; canary scenario registration lives in `apps/canary/scenarios.json`.

## Runtime

**Environment:**
- Rust 1.90.0 with the minimal toolchain profile, Clippy, and rustfmt is pinned in `rust-toolchain.toml`.
- Tokio 1.53.1 supplies task execution, timers, synchronization, current-state watch channels, causal broadcast channels, networking, process supervision, and the canary multithreaded runtime. Runtime features are selected per crate in `Cargo.toml`, `crates/*/Cargo.toml`, and `apps/canary/Cargo.toml`.
- Fava is an embeddable library. `crates/fava/src/lib.rs` exposes `Fava` and `FavaBuilder`; `apps/canary/src/main.rs` is the only product-level executable in the repository.

**Package Manager:**
- Cargo 1.90.0 is the dependency-metadata source of truth. The root resolver-v3 workspace has 34 members in `Cargo.toml` and a format-4 `Cargo.lock`.
- The ordinary downstream canary and the external-provider falsifier are deliberately separate Cargo workspaces with their own manifests and lockfiles at `apps/canary/Cargo.toml`, `apps/canary/Cargo.lock`, `falsifiers/external-null-cache/Cargo.toml`, and `falsifiers/external-null-cache/Cargo.lock`.
- Bazel 9.2.0 is the authoritative root-workspace build/test frontend, pinned by `.bazeliskrc`; `MODULE.bazel` imports the Cargo graph through `rules_rust` crate_universe.

## Frameworks

**Core:**
- No application framework - provider composition is explicit through `FavaBuilder` in `crates/fava/src/lib.rs`; no cache, store, evaluator, transport, router, signer, publisher, or delivery policy is silently selected.
- Tokio 1.53.1 - asynchronous lifecycle owners include `crates/fava-observe/src/lib.rs`, `crates/fava-routing/src/chain.rs`, `crates/fava-publication/src/lib.rs`, and `crates/fava/src/live.rs`.
- Nostr SDK 0.45.3 - supplies events, keys, filters, timestamps, relay URLs, signatures, and NIP-01 message values across `crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`, and `crates/fava-wire/src/lib.rs`.
- Tokio Tungstenite 0.30.0 - the standard relay transport in `crates/fava-transport-websocket/src/lib.rs` supports `ws://` and `wss://`; the independent canary witness uses the same library separately in `apps/canary/src/wire.rs` and `apps/canary/src/proxy.rs`.
- Redb 4.2.0 - the standard durable write-store implementation in `crates/fava-write-store-redb/src/lib.rs` commits accepted receipts with immediate durability and recovers ambiguous in-flight outcomes.

**Testing:**
- Rust built-in test harness - unit and integration targets are co-located under `crates/*/src/` and `crates/*/tests/`; Bazel mirrors them in `crates/*/BUILD.bazel`.
- Tokio test macros 1.53.1 - async owner and facade tests use `#[tokio::test]`, including `crates/fava-transport-websocket/tests/conformance.rs`, `crates/fava-write-store-redb/tests/process_kill.rs`, and `crates/fava/tests/*.rs`.
- Rust canary acceptance - `apps/canary/scenarios.json` registers enabled M0-M6 scenarios executed by `apps/canary/src/main.rs` through public Fava/provider APIs and real relay processes.
- Python `unittest` - vocabulary-gate behavior is tested in `tools/tests/test_vocabulary_check.py`.
- External-provider compile/runtime proof - `falsifiers/external-null-cache/src/lib.rs` implements an event-cache provider outside the root workspace against public contracts.

**Build/Dev:**
- Bazel 9.2.0 with `rules_rust` 0.73.0 - `MODULE.bazel` pins the Rust toolchain, imports `Cargo.toml` plus `Cargo.lock`, and renders the third-party graph for `aarch64-apple-darwin`.
- Cargo - retains dependency metadata and independently builds/tests the canary and falsifier; milestone issue records under `docs/issues/` list the complete Cargo validation surfaces.
- rustfmt - edition 2024, 100-column width, field-init shorthand, and try shorthand are configured in `rustfmt.toml`; Bazel exposes `bazel run //:fmt` and `--config=fmt-check` through `BUILD.bazel` and `.bazelrc`.
- Clippy - workspace lints in `Cargo.toml` enable `all` and `pedantic`; unsafe code is forbidden and missing docs warn. `.bazelrc` exposes a `--config=clippy` aspect with warnings denied.
- Architectural vocabulary gate - `docs/internals/vocabulary.toml` registers concepts, public symbols, current crate names, and specified future names; `tools/check_vocabulary.py` rejects unregistered public nouns and crates.

## Key Dependencies

**Critical:**
- `nostr` = 0.45.3 - Nostr protocol values and cryptographic verification across the domain, wire, ingest, signer, and routing crates declared in `Cargo.toml`.
- `tokio` = 1.53.1 - bounded async execution and observation/publication coordination across `crates/fava-observe/`, `crates/fava-routing/`, `crates/fava-publication/`, `crates/fava-signer/`, and `crates/fava/`.
- `tokio-tungstenite` = 0.30.0 and `futures-util` = 0.3.34 - WebSocket connection, split sink/stream, and frame flow in `crates/fava-transport-websocket/src/lib.rs` and the canary networking modules.
- `redb` = 4.2.0 - durable receipt and publication-obligation storage in `crates/fava-write-store-redb/src/lib.rs` and `crates/fava-write-store-redb/src/ops.rs`.
- `thiserror` = 2.0.20 - typed public refusal/error surfaces throughout contract and facade crates, including `crates/fava-query/`, `crates/fava-routing/`, `crates/fava-write-store/`, and `crates/fava/`.
- `serde` = 1.0.229 and `serde_json` = 1.0.151 - persisted write values, NIP-01 frames, plan inputs, canary scenarios, and evidence artifacts in `crates/fava-state/`, `crates/fava-write/`, `crates/fava-write-store-redb/`, `crates/fava-wire/`, and `apps/canary/`.

**Infrastructure:**
- Contract crates are separate from implementations: `crates/fava-event-cache/` / `crates/fava-event-cache-memory/`, `crates/fava-write-store/` / memory and Redb providers, `crates/fava-transport/` / WebSocket provider, `crates/fava-subscriptions/` / standard and no-grouping planners, `crates/fava-signer/` / local signer, `crates/fava-publisher/` / NIP-01 publisher, and `crates/fava-delivery/` / standard policy.
- Routing is split between the neutral contracts/composer in `crates/fava-routing/` and policy implementations in `crates/fava-router-app-relays/`, `crates/fava-router-fallback-relays/`, `crates/fava-router-outbox/`, and `crates/fava-router-hints/`.
- `sha2` = 0.11.0 and `hex` = 0.4.3 derive deterministic canary run identities and artifact hashes in `apps/canary/src/artifacts.rs` and disposable identities in `apps/canary/src/lib.rs`.
- `tempfile` = 3.27.0 supplies isolated acceptance/test storage in `apps/canary/Cargo.toml`.
- `nostr-rs-relay` = 0.8.12 is an installed external acceptance prerequisite, version-checked and process-supervised by `apps/canary/src/relay.rs`; it is not linked into the engine workspace.

## Configuration

**Environment:**
- No environment file, secrets loader, or required environment variable is used. Canary relay binary, seed, evidence directory, and public reconnaissance URL are explicit CLI inputs parsed by `apps/canary/src/main.rs`.
- `RUST_LOG=info` is injected only into the supervised relay child by `apps/canary/src/relay.rs`.
- Provider selection is compile-time/application assembly through `FavaBuilder` in `crates/fava/src/lib.rs`; hidden runtime feature flags and implicit provider defaults are absent.
- Scenario ownership and enabled state are checked in at `apps/canary/scenarios.json`; executable dispatch is explicit in `apps/canary/src/main.rs`.

**Build:**
- Cargo owns package metadata, exact third-party versions, workspace lints, and the lock graph in `Cargo.toml` and `Cargo.lock`.
- Bazel owns the root build surface through `MODULE.bazel`, `.bazeliskrc`, `.bazelrc`, `BUILD.bazel`, and `crates/*/BUILD.bazel`; first-party crate dependencies are explicit while crate_universe supplies third-party dependencies.
- `.bazelrc` places Bazel outputs outside Cargo `target/`, shares a bounded 50 GB action cache across worktrees, and defines strict Clippy and formatting configurations.
- The canary generates loopback relay TOML per run with persistent SQLite, NIP-42 disabled, and explicit event/frame/buffer limits in `apps/canary/src/relay.rs`.

## Platform Requirements

**Development:**
- Install Rust 1.90.0 with Clippy and rustfmt as declared in `rust-toolchain.toml`; use Bazelisk/Bazel 9.2.0 for the authoritative root build defined by `.bazeliskrc` and `MODULE.bazel`.
- Bazel's rendered dependency graph currently targets `aarch64-apple-darwin` only in `MODULE.bazel`. Cargo metadata is not restricted to that triple.
- Deterministic real-relay acceptance requires `nostr-rs-relay` 0.8.12 and the macOS/process commands used by `apps/canary/src/relay.rs`, `apps/canary/src/artifacts.rs`, and `apps/canary/src/lib.rs`, as documented in `apps/canary/README.md`.
- Python 3 with `tomllib` runs `tools/check_vocabulary.py`; CI pins Python 3.13 in `.github/workflows/architecture.yml`.

**Production:**
- The implemented artifact is the downstream-assembled Rust library in `crates/fava/src/lib.rs`, with a standard WebSocket transport in `crates/fava-transport-websocket/`, NIP-01 publisher in `crates/fava-publisher-nip01/`, memory event cache in `crates/fava-event-cache-memory/`, and memory or durable Redb write store in `crates/fava-write-store-memory/` and `crates/fava-write-store-redb/`.
- No server, container, deployment manifest, persistent event-cache provider, HTTP service cache, NIP-42 auth owner, FFI layer, Swift package, Kotlin/JVM package, Android AAR, or iOS XCFramework exists in the tracked implementation. Those capabilities belong to specified M8-M11 work in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

---

*Stack analysis: 2026-08-21*
