# Technology Stack

**Analysis Date:** 2026-08-20

## Languages

**Primary:**
- Rust 1.90 / edition 2024 - all implemented engine crates, the facade, the external-provider falsifier, and the canary application are Rust packages declared in `Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`, and `apps/canary/Cargo.toml`.

**Secondary:**
- Gherkin feature text - behavioral specifications live in `features/local-source-merge.feature` and `features/relay-lab.feature`; no Cucumber dependency is declared in `Cargo.toml`.
- JSON and TOML - manifests use TOML in `Cargo.toml` and `apps/canary/Cargo.toml`; the canary embeds `apps/canary/scenarios.json` and generates relay TOML from `apps/canary/src/relay.rs`.

## Runtime

**Environment:**
- Rust toolchain 1.90.0 with the minimal profile - pinned in `rust-toolchain.toml`; root packages require Rust 1.90 and edition 2024 through `Cargo.toml`.
- Tokio 1.53.1 - supplies tasks, timers, and `watch` channels in `crates/fava-observe/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`, and `crates/fava-write-store-memory/src/lib.rs`; the canary enables filesystem, network, process, and multithreaded-runtime features in `apps/canary/Cargo.toml`.
- The product is an embeddable library - `crates/fava/Cargo.toml` declares the public library target; the only binary is the independent acceptance app in `apps/canary/Cargo.toml`.

**Package Manager:**
- Cargo 1.90.0 - selected by `rust-toolchain.toml` and used by `Cargo.toml`, `apps/canary/Cargo.toml`, and `falsifiers/external-null-cache/Cargo.toml`.
- Lockfiles: Cargo format 4 lockfiles are present at `Cargo.lock`, `apps/canary/Cargo.lock`, and `falsifiers/external-null-cache/Cargo.lock`.
- Workspace layout: ten libraries belong to the resolver-v3 root workspace in `Cargo.toml`; the canary and falsifier are independent workspaces in `apps/canary/Cargo.toml` and `falsifiers/external-null-cache/Cargo.toml`.

## Frameworks

**Core:**
- No application framework - focused libraries are assembled through `FavaBuilder` in `crates/fava/src/lib.rs`, following static composition in `docs/spec/ARCHITECTURE.md`.
- Tokio 1.53.1 - bounded latest-state delivery uses `tokio::spawn`, `tokio::select!`, and `tokio::sync::watch` in `crates/fava-observe/src/lib.rs`.
- Nostr SDK 0.45.3 - supplies event, key, relay URL, timestamp, filter, and client-message types in `crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`, and `apps/canary/src/wire.rs`.

**Testing:**
- Rust built-in test harness - unit and integration tests use `#[test]` in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-query-standard/tests/source_merge.rs`.
- Tokio test macros 1.53.1 - async tests use `#[tokio::test]` in `crates/fava-observe/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`, and `falsifiers/external-null-cache/src/lib.rs`.
- Gherkin evidence catalog - requirement IDs, built status, evidence, and falsifiers are mapped in `features/local-source-merge.feature` and `features/relay-lab.feature`; these files are not a separate runtime framework.

**Build/Dev:**
- Cargo - builds/tests the root through `Cargo.toml`; the independent canary command is documented in `apps/canary/README.md`.
- rustfmt - installed through `rust-toolchain.toml`; edition 2024, 100-column width, field-init shorthand, and try shorthand are configured in `rustfmt.toml`.
- Clippy - installed through `rust-toolchain.toml`; `all` and `pedantic` are warnings and unsafe code is forbidden through `Cargo.toml` and `apps/canary/Cargo.toml`.
- `nostr-rs-relay` 0.8.12 - separately installed acceptance dependency documented in `apps/canary/README.md` and version-enforced in `apps/canary/src/relay.rs`.

## Key Dependencies

**Critical:**
- `nostr` = 0.45.3 - owns Nostr event creation/verification, keys, timestamps, relay URLs, filters, and messages across `Cargo.toml`, `crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`, and `apps/canary/src/wire.rs`.
- `tokio` = 1.53.1 - owns async observation in `crates/fava-observe/Cargo.toml` and canary filesystem/network/process orchestration in `apps/canary/Cargo.toml`.
- `thiserror` = 2.0.20 - derives typed errors in `crates/fava/Cargo.toml`, `crates/fava-query/Cargo.toml`, `crates/fava-event-cache/Cargo.toml`, and `crates/fava-write-store/Cargo.toml`.
- Workspace crates = 0.1.0 - semantic values live in `crates/fava-state/` and `crates/fava-write/`; neutral contracts in `crates/fava-query/`, `crates/fava-event-cache/`, and `crates/fava-write-store/`; implementations in `crates/fava-query-standard/`, `crates/fava-event-cache-memory/`, and `crates/fava-write-store-memory/`; ownership/assembly in `crates/fava-observe/` and `crates/fava/`, all registered by `Cargo.toml`.

**Infrastructure:**
- `tokio-tungstenite` = 0.30.0 and `futures-util` = 0.3.34 - provide WebSocket streams/frame forwarding only to the canary in `apps/canary/Cargo.toml`, `apps/canary/src/wire.rs`, and `apps/canary/src/proxy.rs`.
- `serde` = 1.0.229 and `serde_json` = 1.0.151 - parse scenarios and serialize manifests, evidence, frames, and reports in `apps/canary/Cargo.toml`, `apps/canary/src/lib.rs`, `apps/canary/src/recon.rs`, and `apps/canary/src/artifacts.rs`.
- `sha2` = 0.11.0 and `hex` = 0.4.3 - derive disposable identities/run IDs and hash artifacts in `apps/canary/Cargo.toml`, `apps/canary/src/lib.rs`, and `apps/canary/src/artifacts.rs`.
- `tempfile` = 3.27.0 - test-only temporary storage in `apps/canary/Cargo.toml` and `apps/canary/src/recon.rs`.
- `nostr-rs-relay` = 0.8.12 - real third-party process used by `apps/canary/src/relay.rs`; it is not linked into the engine workspace in `Cargo.toml`.

## Configuration

**Environment:**
- No environment file is required; relay URL, relay binary, seed, and runs directory are CLI inputs in `apps/canary/src/main.rs`.
- `RUST_LOG=info` is injected into the child relay by `apps/canary/src/relay.rs` rather than supplied by callers.
- No runtime feature-flag system exists in `Cargo.toml`; `AGENTS.md` prohibits hidden runtime flags and dependency features are explicit in `Cargo.toml` and `apps/canary/Cargo.toml`.
- Scenario status is checked in at `apps/canary/scenarios.json`: `lab-real-relay-smoke` is enabled and `public-relay-recon` is reconnaissance.

**Build:**
- Workspace graph, shared versions, pins, resolver, and lints are centralized in `Cargo.toml`.
- Compiler/components are pinned in `rust-toolchain.toml`; formatting is configured in `rustfmt.toml`.
- The canary resolves separately through `apps/canary/Cargo.toml` and `apps/canary/Cargo.lock`; the external-provider proof does the same through `falsifiers/external-null-cache/Cargo.toml` and `falsifiers/external-null-cache/Cargo.lock`.
- Relay configuration is generated per run with loopback binding, SQLite persistence, explicit event/frame/buffer limits, and NIP-42 disabled in `apps/canary/src/relay.rs`.

## Platform Requirements

**Development:**
- Install Rust 1.90.0, Cargo, rustfmt, and Clippy as declared in `rust-toolchain.toml`; registry dependencies are pinned by `Cargo.lock`.
- Core development is Rust-only through `Cargo.toml`; no Swift package, Gradle project, Node/Python manifest, Dockerfile, or CI workflow accompanies the implemented artifacts under `crates/`, `apps/`, or the repository root.
- Deterministic canary execution additionally requires macOS, `nostr-rs-relay` 0.8.12, and the `git`, `uname`, `ps`, `kill`, and `rustc` commands used by `apps/canary/README.md`, `apps/canary/src/lib.rs`, `apps/canary/src/artifacts.rs`, and `apps/canary/src/relay.rs`.

**Production:**
- The current deliverable is the downstream-assembled Rust library in `crates/fava/src/lib.rs`; no production server, container, hosting target, Swift/Kotlin package, or deployment manifest is implemented in `Cargo.toml` or `apps/canary/Cargo.toml`.
- Concrete engine providers are current-process-only: `MemoryEventCache` in `crates/fava-event-cache-memory/src/lib.rs` and `MemoryWriteStore` in `crates/fava-write-store-memory/src/lib.rs`; persistent product providers are absent from the current members in `Cargo.toml`.
- Public-relay access is evidence-only and requires an explicit URL in `apps/canary/src/main.rs`; deterministic acceptance uses the local child relay in `apps/canary/README.md`.
