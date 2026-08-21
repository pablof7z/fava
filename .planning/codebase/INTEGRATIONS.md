# External Integrations

**Analysis Date:** 2026-08-21

## Milestone Integration Boundary

**Implemented M0-M6:**
- Real/local Nostr relay processes, NIP-01 WebSocket read/write traffic, local Nostr signing, durable Redb write custody, memory cache/store profiles, application-selected router policies, exact relay diagnostics, and reconstructable filesystem evidence are executable through `crates/` and `apps/canary/`.
- Completed integration claims are owned by `docs/issues/0002-m0-evidence-foundation.md`, `docs/issues/0004-explicit-live-query.md`, `docs/issues/0005-multi-relay-observation.md`, `docs/issues/0006-ordered-automatic-routing.md`, `docs/issues/0007-durable-explicit-publication.md`, and `docs/issues/0008-automatic-write-routing.md`.

**Specified M7-M11:**
- Replaceable-edit protocol crates, NIP-42 authentication, hostile/provider isolation qualification, NIP-05/NIP-11 HTTP services and fetch cache, persistent event-cache profiles, the full external-provider matrix, FFI, Swift, and Kotlin are specified only in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- Do not connect new work to a presumed auth service, HTTP client, persistent event cache, native SDK, or deployment service; none exists in `Cargo.toml`, `apps/canary/scenarios.json`, or the tracked source tree.

## APIs & External Services

**Nostr Relays:**
- The standard engine transport opens `ws://` and `wss://` relay sessions through `tokio-tungstenite` in `crates/fava-transport-websocket/src/lib.rs`. It assigns a fresh numeric generation, bounds outgoing text frames to 1,048,576 bytes by default, classifies failed sends as definite or ambiguous handoff, rejects binary frames, and closes idempotently.
  - SDK/Client: NIP-01 values encode/decode through `nostr` and `serde_json` in `crates/fava-wire/src/lib.rs`; subscription planning lives in `crates/fava-subscriptions/`, while `crates/fava-publisher-nip01/src/lib.rs` publishes over the public transport contract.
  - Endpoint selection: exact relay URLs are public `RelayUrl`/`RelaySessionKey` values from `crates/fava-state/src/lib.rs`; explicit acquisition and application-selected ordered routers are assembled by `crates/fava/src/lib.rs` and reconciled in `crates/fava/src/routes.rs`.
  - TLS: `crates/fava-transport-websocket/Cargo.toml` enables the `rustls-tls-webpki-roots` feature on `tokio-tungstenite`; no repository-managed certificate or private key is used.
  - Auth: relay work carries a `RelayAccess` identity from `crates/fava-state/src/lib.rs`, but no NIP-42 authentication exchange or auth-policy provider is implemented. The deterministic lab explicitly sets `nip42_auth = false` in `apps/canary/src/relay.rs`.

**Third-Party Relay Process:**
- `nostr-rs-relay` 0.8.12 is the pinned real-process acceptance dependency documented in `apps/canary/README.md` and version-enforced in `apps/canary/src/relay.rs`.
  - SDK/Client: Tokio starts and supervises the executable in `apps/canary/src/relay.rs`; the binary is installed separately and does not enter `Cargo.toml`.
  - Configuration: every lab relay receives an isolated loopback address, generated TOML, persistent SQLite data directory, logs, and explicit event/frame/buffer limits from `apps/canary/src/relay.rs`.
  - Lifecycle: readiness probing, hard kill, same-directory restart, SIGTERM, and fallback kill are owned by `apps/canary/src/relay.rs`.
  - Witnessing: `apps/canary/src/proxy.rs` records bidirectional WebSocket frames independently of Fava diagnostics.

**Public Relay Reconnaissance:**
- `apps/canary/src/recon.rs` performs bounded, read-only, evidence-only WebSocket reconnaissance against an explicit caller-provided relay URL.
  - SDK/Client: `apps/canary/src/wire.rs`.
  - Auth: none; no default public relay or credential source exists.
  - Failure semantics: an external failure is recorded in the evidence bundle and returned as a failure, not a skipped success.

**Host Tooling:**
- The canary invokes `git`, `uname`, and `rustc` to capture revision/dirty/platform/toolchain facts in `apps/canary/src/lib.rs`.
- Resource and process evidence invokes `ps` in `apps/canary/src/artifacts.rs` and `kill` in `apps/canary/src/relay.rs`; crash qualification spawns the current test/canary executable in `crates/fava-write-store-redb/tests/process_kill.rs` and `apps/canary/src/publication_support.rs`.

## Data Storage

**Databases:**
- Redb 4.2.0 is the standard durable Fava write-store provider in `crates/fava-write-store-redb/src/lib.rs` and `crates/fava-write-store-redb/src/ops.rs`.
  - Connection: applications pass one exact filesystem path to `RedbWriteStore::open` or `open_bounded` in `crates/fava-write-store-redb/src/lib.rs`; no URL or environment variable is used.
  - Client: the `redb` crate is pinned in `Cargo.toml` and used only behind the neutral `WriteStore` and `QuerySource` contracts from `crates/fava-write-store/src/lib.rs` and `crates/fava-query/src/lib.rs`.
  - Persisted state: the `receipts` table stores JSON-encoded receipts by `ReceiptId`, while the `meta` table stores the next identity. Acceptance commits both atomically with `Durability::Immediate` in `crates/fava-write-store-redb/src/lib.rs`.
  - Recovery: in-flight `Attempting` destinations recover as exact `Unknown` outcomes before open work resumes in `crates/fava-write-store-redb/src/lib.rs`; process-kill evidence lives in `crates/fava-write-store-redb/tests/process_kill.rs`.
  - Bounds: standard active and retained-terminal limits are 10,000 each; committed receipt changes use a 256-entry causal broadcast in `crates/fava-write-store-redb/src/lib.rs`.
- Memory providers remain selectable in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
  - Connection: not applicable; state is process-local.
  - Client: providers implement the public contracts from `crates/fava-event-cache/src/lib.rs` and `crates/fava-write-store/src/lib.rs` and are assembled through `FavaBuilder` in `crates/fava/src/lib.rs`.
- Each canary relay uses third-party SQLite persistence with `in_memory = false` in generated configuration from `apps/canary/src/relay.rs`.
  - Connection: the relay process receives a per-run directory through its `--db` argument.
  - Client: Fava reaches this database only through real NIP-01 WebSocket traffic; it does not link or inspect the relay SQLite database to decide application correctness.

**File Storage:**
- Local filesystem only - canary runs create JSONL evidence, wire logs, app/relay stdout and stderr, resource samples, reports, manifests, relay configuration/database files, and SHA-256 hashes through `apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, and `apps/canary/src/proxy.rs`.
- Default evidence lives under ignored `apps/canary/runs/`; callers may select another directory with `--runs-dir` in `apps/canary/src/main.rs`.
- Durable publication state uses an application-selected Redb path through `crates/fava-write-store-redb/src/lib.rs`.
- No object-storage SDK or remote file service is declared in `Cargo.toml` or `apps/canary/Cargo.toml`.

**Caching:**
- `MemoryEventCache` in `crates/fava-event-cache-memory/src/lib.rs` is the only concrete event-cache provider. It stores admitted signed relay events and evidence in bounded process-local memory.
- `MemoryWriteStore` in `crates/fava-write-store-memory/src/lib.rs` is the volatile write-store option; `RedbWriteStore` in `crates/fava-write-store-redb/src/lib.rs` is the durable option.
- No persistent event-cache provider, null cache in the product workspace, generic fetch cache, Redis/Memcached integration, or NIP-05/NIP-11 service cache exists. M9 specifies those additions in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.
- `falsifiers/external-null-cache/src/lib.rs` is an external-provider replaceability proof, not a production cache package or root-workspace dependency.

## Authentication & Identity

**Auth Provider:**
- No OAuth, OIDC, password, token, hosted identity, or NIP-42 auth-policy provider is integrated.
  - Implementation: Nostr authorship and relay access are separate values in `crates/fava-state/src/lib.rs` and `crates/fava-write/src/lib.rs`; `crates/fava-signer-local/src/lib.rs` signs for an exact Nostr public key selected by the event author.
  - Lab identity: `apps/canary/src/lib.rs` derives disposable keys from caller-selected seeds and signs real events locally through `nostr`.
  - Relay auth: generated lab configuration disables NIP-42 in `apps/canary/src/relay.rs`; M8 owns the specified NIP-42 implementation in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`.

## Monitoring & Observability

**Error Tracking:**
- None - no Sentry, OpenTelemetry, hosted metrics, or remote error-tracking dependency is declared in `Cargo.toml` or `apps/canary/Cargo.toml`.

**Logs:**
- Public current-state diagnostics are bounded in-memory facts owned by `crates/fava-diagnostics/src/lib.rs` and exposed as `DiagnosticsSnapshot` by `Fava::diagnostics` in `crates/fava/src/lib.rs`.
- Typed errors/refusals are returned through contract and facade APIs, including `crates/fava-transport/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-routing/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, and `crates/fava/src/lib.rs`.
- Canary evidence is local and reconstructable: `evidence.jsonl`, application/relay logs, `resources.csv`, `report.md`, `manifest.json`, artifact hashes, and proxy frames are written by `apps/canary/src/artifacts.rs`, `apps/canary/src/proxy.rs`, `apps/canary/src/recon.rs`, and the scenario modules.
- Relay logging is enabled only for the supervised child through `RUST_LOG=info` in `apps/canary/src/relay.rs`.

## CI/CD & Deployment

**Hosting:**
- None - `crates/fava/Cargo.toml` builds an embeddable Rust library, not a hosted service. The canary is a local binary in `apps/canary/Cargo.toml`, and its deterministic relay processes bind to `127.0.0.1` in `apps/canary/src/relay.rs`.

**CI Pipeline:**
- GitHub Actions runs only the architectural vocabulary job in `.github/workflows/architecture.yml` on pull requests and pushes to `main`.
- The CI job checks out source, installs Python 3.13, runs `python3 tools/check_vocabulary.py`, and runs `python3 -m unittest tools/tests/test_vocabulary_check.py`.
- Root Rust build/test, strict Clippy, formatting, canary tests/live scenarios, falsifier tests, and Bazel tests are explicit local milestone gates recorded in `docs/issues/0001-local-source-merge.md` through `docs/issues/0008-automatic-write-routing.md`; they are not jobs in `.github/workflows/architecture.yml`.
- No release packaging, artifact upload, container build, deployment, or native SDK pipeline exists.

## Environment Configuration

**Required env vars:**
- None. `apps/canary/src/main.rs` requires explicit CLI arguments for relay binary override, seed, runs directory, and public reconnaissance relay URL.
- `RUST_LOG` is not caller-required; `apps/canary/src/relay.rs` supplies it to the child relay.

**Secrets location:**
- Not applicable - no secrets loader, credential file, token store, or environment-file dependency is used by the tracked Rust workspaces.
- Canary signing keys are disposable values derived from `--seed` in `apps/canary/src/lib.rs`; evidence records public event/process facts, not an external credential integration.

## Webhooks & Callbacks

**Incoming:**
- None - no HTTP server or webhook endpoint exists in `crates/` or `apps/canary/`.
- The only listeners are WebSocket/TCP test and evidence fixtures: the loopback proxy in `apps/canary/src/proxy.rs`, hostile/scripted witnesses in `apps/canary/src/hostile.rs`, and test listeners such as `crates/fava-transport-websocket/tests/conformance.rs`.

**Outgoing:**
- No HTTP webhooks. Engine egress is NIP-01 WebSocket traffic through `crates/fava-transport-websocket/src/lib.rs` and `crates/fava-publisher-nip01/src/lib.rs`.
- Deterministic acceptance traffic stays local through the proxy and supervised `nostr-rs-relay` processes in `apps/canary/src/proxy.rs` and `apps/canary/src/relay.rs`.
- Public network access occurs only when the caller explicitly invokes reconnaissance with `--relay` in `apps/canary/src/main.rs`.

---

*Integration audit: 2026-08-21*
