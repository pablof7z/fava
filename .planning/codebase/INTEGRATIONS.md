# External Integrations

**Analysis Date:** 2026-08-20

## APIs & External Services

**Nostr Relays:**
- Implemented engine crates have no relay transport; the current local-source workspace stops at cache/write-store merging in `Cargo.toml`, `crates/fava/src/lib.rs`, and `crates/fava-observe/src/lib.rs`.
- The independent canary speaks NIP-01 JSON frames to WebSocket relays with `nostr` 0.45.3 and `tokio-tungstenite` 0.30.0 in `apps/canary/Cargo.toml` and `apps/canary/src/wire.rs`.
  - SDK/Client: `nostr` builds/verifies `EVENT`, `REQ`, `CLOSE`, `OK`, and `EOSE` values in `apps/canary/src/wire.rs`; `tokio-tungstenite` opens `ws://` or `wss://` streams in `apps/canary/src/wire.rs`.
  - Endpoint selection: deterministic runs use generated loopback `ws://127.0.0.1:<port>/` endpoints in `apps/canary/src/relay.rs`; reconnaissance requires `--relay <URL>` in `apps/canary/src/main.rs`.
  - Auth: no external credentials; the lab signs with seed-derived disposable Nostr keys in `apps/canary/src/lib.rs`, while public reconnaissance is bounded and read-only in `apps/canary/src/recon.rs`.

**Third-Party Relay Process:**
- `nostr-rs-relay` 0.8.12 is the pinned real-relay dependency documented in `apps/canary/README.md` and version-checked in `apps/canary/src/relay.rs`.
  - SDK/Client: Tokio executes the child in `apps/canary/src/relay.rs`; the process is outside the root Cargo graph in `Cargo.toml`.
  - Configuration: each run creates isolated loopback/SQLite configuration and data under paths owned by `apps/canary/src/relay.rs` and `apps/canary/src/artifacts.rs`.
  - Lifecycle: readiness probing, hard kill, same-data restart, and graceful SIGTERM are owned by `apps/canary/src/relay.rs`.

**Host Tooling:**
- Canary manifests capture revision, dirty state, host, and toolchain by invoking `git`, `uname`, and `rustc` in `apps/canary/src/lib.rs`.
- Resource/process evidence invokes `ps` in `apps/canary/src/artifacts.rs` and `kill` in `apps/canary/src/relay.rs`; these are local canary integrations, not engine dependencies.

## Data Storage

**Databases:**
- No persistent Fava database provider is implemented in `Cargo.toml`; concrete engine storage is `crates/fava-event-cache-memory/` and `crates/fava-write-store-memory/`.
  - Connection: not applicable; `MemoryEventCache` and `MemoryWriteStore` keep `BTreeMap` state behind process-local `Mutex` values in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
  - Client: provider contracts are `EventCache` in `crates/fava-event-cache/src/lib.rs` and `WriteStore` in `crates/fava-write-store/src/lib.rs`; `FavaBuilder` assembles them in `crates/fava/src/lib.rs`.
  - Bounds: both memory providers default to 10,000 entries and refuse overflow in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
- The canary-managed relay uses its own persistent SQLite engine with `in_memory = false` in `apps/canary/src/relay.rs`; SQLite is not a linked Fava backend in `Cargo.toml`.
  - Connection: `apps/canary/src/relay.rs` passes the per-run local data directory through `--db`.
  - Client: the external executable is installed as documented in `apps/canary/README.md`.

**File Storage:**
- Local filesystem only - runs create evidence, relay data, wire/process logs, resource samples, reports, manifests, and hashes through `apps/canary/src/artifacts.rs`.
- Default evidence lives under `apps/canary/runs/` as selected by `apps/canary/src/main.rs`; `.gitignore` excludes new run output and `apps/canary/README.md` documents it.
- No object-storage SDK or remote file service is declared in `Cargo.toml` or `apps/canary/Cargo.toml`.

**Caching:**
- `MemoryEventCache` is the only concrete event cache; it is bounded, volatile, and current-process-only in `crates/fava-event-cache-memory/src/lib.rs`.
- `MemoryWriteStore` is the only concrete write store; it is bounded, volatile, and intended for tests or explicit ephemeral profiles in `crates/fava-write-store-memory/src/lib.rs`.
- No Redis, Memcached, remote cache, service cache, or persistent event-cache client is declared in `Cargo.toml`; replaceable roles are neutral contracts in `crates/fava-event-cache/src/lib.rs` and `crates/fava-write-store/src/lib.rs`.

## Authentication & Identity

**Auth Provider:**
- No external auth or identity provider is integrated in `Cargo.toml` or `apps/canary/Cargo.toml`.
  - Implementation: Nostr public keys and `RelayAccess` values live in `crates/fava-state/src/lib.rs`; no session, OAuth, OIDC, password, or token-exchange member exists in `Cargo.toml`.
  - Lab identity: `apps/canary/src/lib.rs` derives disposable keys from the seed with SHA-256 and signs locally through `nostr`.
  - Relay auth: generated lab configuration sets `nip42_auth = false` in `apps/canary/src/relay.rs`; `apps/canary/src/wire.rs` implements no NIP-42 client flow.

## Monitoring & Observability

**Error Tracking:**
- None - no Sentry, OpenTelemetry, hosted telemetry, or error-tracking dependency is declared in `Cargo.toml` or `apps/canary/Cargo.toml`.

**Logs:**
- The engine returns typed `thiserror` errors from `crates/fava/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, and `crates/fava-write-store/src/lib.rs`; it sends no logs externally.
- Canary output is local `evidence.jsonl`, app/relay stdout/stderr, `resources.csv`, `report.md`, and `manifest.json` written by `apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/lib.rs`.
- The loopback witness proxy records bidirectional frames as JSONL in `apps/canary/src/proxy.rs`; public-relay frames and terminal classification are preserved by `apps/canary/src/recon.rs`.
- Relay logging is enabled with internally supplied `RUST_LOG=info` in `apps/canary/src/relay.rs`.

## CI/CD & Deployment

**Hosting:**
- None - `crates/fava/Cargo.toml` is an embeddable library, and the repository remote does not deploy a running service.
- The canary is a local binary in `apps/canary/Cargo.toml`; its child relay binds only to `127.0.0.1` in `apps/canary/src/relay.rs`.

**CI Pipeline:**
- `.github/workflows/architecture.yml` checks the architectural vocabulary registry and its unit tests. Build/test entry points remain local and explicit: `Cargo.toml`, `apps/canary/Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`, and `bazel test //...` per `.bazelrc`.

## Environment Configuration

**Required env vars:**
- None; runtime choices are CLI inputs parsed in `apps/canary/src/main.rs`.
- `RUST_LOG` is not caller-required because `apps/canary/src/relay.rs` sets it for the supervised relay.

**Secrets location:**
- Not applicable - `Cargo.toml` and `apps/canary/Cargo.toml` use no secrets loader, credential store, or environment-file dependency.
- Canary keys are disposable in-memory values derived from `--seed` in `apps/canary/src/lib.rs`; evidence writes only public key/event identity through `apps/canary/src/lib.rs` and `apps/canary/src/artifacts.rs`.

## Webhooks & Callbacks

**Incoming:**
- None - no HTTP server or webhook endpoint exists in `crates/` or `apps/canary/`; the only listener is the loopback witness proxy in `apps/canary/src/proxy.rs` plus the supervised relay configured by `apps/canary/src/relay.rs`.

**Outgoing:**
- No HTTP webhooks - the only remote-capable call is an explicit WebSocket connection to a caller-supplied Nostr relay in `apps/canary/src/main.rs`, `apps/canary/src/recon.rs`, and `apps/canary/src/wire.rs`.
- Deterministic traffic stays local: client to loopback proxy to loopback `nostr-rs-relay` in `apps/canary/src/lib.rs`, `apps/canary/src/proxy.rs`, and `apps/canary/src/relay.rs`.
