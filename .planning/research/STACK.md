# Technology Stack

**Project:** Fava
**Scope:** Complete M1 and deliver M2-M11; preserve completed M0 behavior/evidence
**Researched:** 2026-08-21
**Overall confidence:** HIGH for specification fit; MEDIUM for ecosystem currency and unbuilt native packaging

## Decision Rule

Repository specifications decide behavior and ownership. Ecosystem research selects mechanisms only inside those boundaries.

- **Existing** — verified in this checkout.
- **Normative** — required by Fava's authoritative specifications.
- **Recommended** — researched choice where specifications leave room.

One database engine does not merge `EventCache`, `WriteStore`, and `FetchCache`; Tokio does not own operation identity or cancellation; UniFFI does not make generated bindings the semantic API.

## Current Baseline to Preserve

| Technology/artifact | Current state | Status | Decision |
|---|---:|---|---|
| Rust | 1.90.0, edition 2024, MSRV 1.90 | Existing | Keep as reproducible build/MSRV baseline. Add current-stable CI instead of silently raising it. |
| Tokio | 1.53.1 | Existing | Keep as runtime implementation, subordinate to Fava lifecycle owners. |
| `nostr` | 0.45.3 | Existing | Keep for protocol values, parsing, verification, and message primitives only. |
| `thiserror` | 2.0.20 | Existing | Keep for typed errors. |
| M1 memory providers | workspace crates | Existing | Keep as reference/ephemeral implementations and owner-test fixtures, not a persistent profile. |
| Canary client | `tokio-tungstenite` 0.30.0 | Existing | Keep as independent wire witness; do not automatically reuse it in the engine. |
| M0 relay fixture | `nostr-rs-relay` 0.8.12 | Existing | Preserve exact M0 evidence. Later matrices may add newer pinned binaries without rewriting M0. |
| Swift/Kotlin/FFI | absent | Existing absence | M11 creates first-class external artifacts; there is no compatibility path yet. |

M1 remains incomplete. Persistent standard providers, engine networking, and native products do not exist yet.

## Recommended Stack

### Core Framework

| Technology | Version | Status | Purpose | Why |
|---|---:|---|---|---|
| Rust | MSRV 1.90.0; current-stable CI 1.97.1 | Existing + Recommended policy | Semantic engine/contracts | Preserves the checked-in baseline while detecting forward breakage. Selected Rust dependencies support 1.90. |
| Cargo workspace | toolchain-supplied | Existing | Crate boundaries/profile assembly | Maps to semantic owners, contracts, provider implementations, profiles, canary, and FFI without plugins. |
| `nostr` | 0.45.3 | Existing | Nostr primitives/verification | Reuses protocol mechanics without importing another client's routing, storage, lifecycle, or receipts. |
| Tokio | 1.53.1 | Existing | Executor, timers, I/O, bounded channels | Mature embeddable runtime; Fava retains lifecycle, exact identity, refusal, and stale-completion authority. |
| `tokio-util` | 0.7.19 | Recommended | `CancellationToken`, `TaskTracker`, local runtime aids | Useful mechanics, subordinate to Fava operation/generation IDs. |
| `bytes` | 1.12.1 | Recommended | Wire/service byte buffers | Enables byte budgets before unconstrained text/JSON allocation. |
| `futures-util` | 0.3.34 | Existing canary; Recommended narrowly | Transport stream/sink mechanics | Do not expose its types in neutral contracts. |
| `thiserror` | 2.0.20 | Existing | Typed errors | Avoids stringly failure surfaces. |
| `serde` | 1.0.229 | Existing canary; Recommended at boundaries | Wire, durable schema, diagnostics, FFI projections | Use explicit versioned representations, not serialization as the domain model. |
| `tracing` | 0.1.44 | Recommended | Internal structured instrumentation | Typed `fava-diagnostics` and independent proxy captures remain public/wire evidence. |
| `tracing-subscriber` | 0.3.23 | Recommended for binaries/tests | Canary/lab/example logs | Keep subscriber policy out of the embeddable core. |

Core rules:

1. Keep values in semantic-owner crates, contracts in separate neutral crates, and providers in implementation crates. Universal owners never import standard implementations.
2. Select a statically known implementation set through explicit builders/profile crates. No runtime registry, dynamic loading, service locator, or hidden feature-selected fallback.
3. Prefer object-safe contracts with explicit future aliases or equally transparent signatures. Do not make `async-trait` a workspace default merely to hide allocation/lifetime behavior.
4. Use bounded Tokio `mpsc` for commands/work, `watch` for latest-state observation, and `oneshot` for one completion. No unbounded queues; no broadcast lag as causal design.
5. Carry exact operation/generation identity in Fava types. Future drop, task ID, cancellation token, or native task is not completion authority.

### Runtime and Provider Isolation

Tokio can time out waiting; it cannot forcibly stop arbitrary provider code. Add an explicit provider-execution boundary before M8:

| Mechanism | Recommendation | Gates |
|---|---|---|
| Dispatch | Per-provider/capability bounded executor with declared queue/concurrency | Boundedness, isolation |
| Owner interaction | Snapshot input, leave owner lock/transaction, invoke provider, validate exact identity before commit | Ownership, stale-result isolation |
| Blocking work | Declared blocking lane; never synchronous DB/hostile work on core async workers | Isolation |
| Panic | Contain unwind where safe, convert to attributable failure, quarantine/close affected scope | Isolation |
| Deadline | Owner deadline/cancel request; reject late result even if work cannot be interrupted | Ownership, boundedness |
| Shutdown | Stop intake, cancel, drain within budget, return typed incomplete-shutdown evidence | Boundedness, proof |

`spawn_blocking`, `timeout`, `abort`, or `CancellationToken` alone does not prove isolation. M8 must deliberately block, panic, cancel, and submit stale completions through the public contract.

### Database

| Technology | Version | Status | Purpose | Why |
|---|---:|---|---|---|
| `redb` | 4.2.0 | Recommended standard | `fava-event-cache-redb`, `fava-write-store-redb`, `fava-fetch-cache-redb` | Pure Rust, embedded, ACID/MVCC, recovery/checksums, stable-format goals, MSRV 1.90; architecture names these crates. |
| Memory providers | workspace | Existing + Normative support | Reference semantics/ephemeral profile | Inspectable falsifiers; never silently claim durability. |
| `rusqlite` + `bundled` SQLite | 0.40.2 | Recommended M10 alternative | Materially different external provider | Stronger replaceability proof than another Redb wrapper; not standard. |

Redb policy:

- Retain three semantic authorities even under one technology. Prefer separate files/handles so lifecycle, corruption, retention, and crash evidence stay attributable.
- `WriteStore` acceptance uses `Durability::Immediate` or equivalently proved durable commit. Never acknowledge after `Durability::None`, memory queue, or unflushed batch.
- Cache batching is allowed only where the owning contract/profile exposes resulting watermark/shortfall semantics.
- Run transactions in the blocking provider lane. Never hold a Fava owner lock across DB calls or a DB write transaction across network/signer/provider work.
- Store explicit provider-schema and semantic-record versions with bounds and migration/refusal policy. No raw Rust layouts or unversioned `bincode`.
- Kill processes before, during, and after transaction/receipt boundaries. Recovery distinguishes WriteStore truth from cache truth.
- Bound keys, values, results, migrations, retained evidence, and corruption reports before allocation/observation.

Redb's single-writer model fits serialized mutation owners only if Fava does not turn all stores into one global transaction authority.

### Network and Platform Services

| Technology | Version | Status | Purpose | Why |
|---|---:|---|---|---|
| `tokio-websockets` | 0.13.3 | Recommended engine transport | `fava-transport-websocket` | Strict, Tokio-native, MSRV 1.89, explicit maximum payload, direct platform-verifier support. Different implementation from canary strengthens evidence. |
| `tokio-tungstenite` | 0.30.0 | Existing canary witness | Independent canary/proxy client | Already proved and exposes frame/message limits. Keep outside standard engine path. |
| `reqwest` | 0.13.4 | Recommended | NIP-05/NIP-11 and bounded HTTPS services | Rustls default in 0.13; mature timeout, redirect, body-stream, connection controls. Not for relay WebSockets. |
| `rustls-platform-verifier` | 0.7.0 | Recommended | Apple/Android platform trust | Uses Security Framework/Android Trust Manager. Android component/initialization must be packaged and proved. |
| `nostr-rs-relay` | 0.10.0 future matrix; preserve 0.8.12 M0 | Recommended fixture | First real relay | SQLite-backed, NIP-42 documented; pin tag, commit, flags, binary hash. |
| `strfry` | 1.1.1 | Recommended second relay | M8 relay diversity | LMDB-backed and operationally different; pin tag, commit, flags, binary hash. |

Transport/service policy:

- Transport owns connect/read/write mechanics, not subscriptions, routing, provenance, or receipts.
- Set handshake/close deadlines, maximum frame/message/payload bytes, pending outbound frames/bytes, redirects, concurrency, and retained diagnostics. Defaults are not Fava bounds.
- Enforce outer byte bounds before JSON/Nostr parsing. Keep TLS, DNS, connect, upgrade, decode, relay notice, timeout, cancellation, and local refusal attributable.
- Select one Rustls crypto provider explicitly during profile assembly; never let feature unification choose accidentally.
- Use platform roots in shipping native artifacts. Any web-PKI-root variant is a separately named/evidenced profile.
- For HTTP, bound redirects, streamed body bytes, response time, concurrency, and cache/evidence retention.
- Keep canary helpers independent from product transport code.

Use `nostr` primitives, not high-level `nostr-sdk` client/language bindings. That layer owns a relay pool, subscriptions, database integration, and lifecycle—the authorities Fava must own and prove.

### Native SDK and FFI

| Technology | Version | Status | Purpose | Why |
|---|---:|---|---|---|
| UniFFI | 0.32.0 | Recommended | Low-level Swift/Kotlin bindings from `fava-ffi` | Official Swift/Kotlin, async, foreign-trait, custom-type support; one projection layer reduces drift. |
| `uniffi-bindgen` | exactly 0.32.0 | Recommended tool | Generate source/header/modulemap from compiled library | Pin with runtime crate; no floating global CLI. |
| Rust iOS targets | `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios` when needed | Recommended | Device/simulator static libraries | Official targets; package static library as XCFramework. |
| XCFramework + SwiftPM | Xcode-supported | Recommended product | Binary framework + idiomatic Swift wrapper | Consumer tests install produced package, not repository-relative files. |
| `cargo-ndk` | 4.1.2 | Recommended tool | Android `cdylib` per ABI | Reproducible target/platform/linker selection. |
| AGP / Gradle | 9.2.0 / 9.4.1 | Recommended | AAR build/test | Pin wrapper; own modern lazy task integration, not legacy `libraryVariants` examples. |
| Kotlin | 2.4.10 | Recommended | Kotlin wrapper/tests | Generated bindings behind small idiomatic API. |
| Android NDK | 28.2.13676358 | Recommended | Native toolchain | Pin exact side-by-side version. |
| JDK | 17 | Recommended floor | Gradle/AGP runtime | Stable baseline; record exact CI distribution/version. |
| JNA | 5.19.1 | Recommended pinned | UniFFI Kotlin invocation | Lock in AAR; do not inherit unknown app version. |
| Kotlin coroutines | 1.11.0 | Recommended pinned | Suspend/Flow wrappers | Fava still owns cancellation/terminal state. |

FFI rules:

1. Keep UniFFI annotations, generated-type concessions, flattened errors, callbacks only in `fava-ffi`; core crates remain ordinary Rust.
2. Export a small ABI: owned handles, immutable snapshots, bounded collections, explicit errors, cancel/close, terminal completion. Never export actors, channels, provider traits, DB handles, or Tokio types.
3. Generated code is low-level ABI transport. Hand-written wrappers own stable naming, async sequence/flow, lifecycle, `Sendable`/threading, error taxonomy.
4. Native task cancellation calls explicit Fava cancellation. Dropping a foreign future is not semantic cancellation.
5. Compile selected runtime/provider profile into each artifact; no plugin lookup, reflection registry, or hidden fallback.
6. Package binary, source, header/modulemap, wrapper, JNA/coroutine metadata, and Android verifier component. Capstones start only from published packages.
7. Rust retains mutable truth. Native objects hold handles/snapshots, never duplicate cache, subscription, write, cancellation, or receipt state.

Research host: Xcode 26.6, Swift 6.3.3, JDK 17.0.12. These are initial lab baselines, not a supported matrix. M11 must prove Swift 6 concurrency/`Sendable`, Kotlin cancellation, and package installation in disposable simulator/device processes.

### Testing and Evidence

| Tool | Version | Purpose | Policy |
|---|---:|---|---|
| Rust tests | 1.90 baseline | Owner/contract/doc tests | Primary mechanism; use public Fava API where gates require it. |
| `proptest` | 1.11.0 | Query algebra, normalization, state/order/bounds | Shrink failures; retain useful seeds. |
| `loom` | 0.7.2 | Small synchronization/lifecycle primitives | Narrow Loom-compatible modules only. |
| `cargo-nextest` | 0.9.143 | Process isolation/resource groups | Groups for relays/crash/hostile/native; no retries for deterministic evidence; doc tests separately. |
| `cargo-deny` | 0.20.2 | Advisories/licenses/bans/sources | Required before release; exceptions need owner/expiry. |
| `cargo-hack` | 0.6.45 | Feature/MSRV/profile matrix | Detect hidden dependencies and accidental profiles. |
| AndroidX runner/core | 1.7.0 | Packaged AAR consumer tests | Separate consumer app/process. |
| AndroidX JUnit | 1.3.0 | Instrumentation integration | Align observable cases with Rust owner scenarios. |

Owner tests prove semantics; unchanged contract suites cover memory, Redb, hostile, and external providers; crash supervisors kill processes; canary uses independent proxy and relays; native capstones consume packages and prove Rust-parity lifecycle, cancellation, bounds, restart, and failure.

Containers aid installation but are not evidence. Record binary/image digest, command, configuration, wire exchange, and result.

### Infrastructure

| Area | Recommendation | Reason |
|---|---|---|
| Locks/pins | Commit locks for tools/products; pin relay commits/native wrappers | Reproducible evidence/upgrades |
| Rust CI | MSRV 1.90, stable 1.97.1, fmt, Clippy, nextest, doc, deny, profile matrix | Baseline honesty/drift detection |
| Native CI | Pin Xcode, SDK/NDK, JDK, Gradle, Kotlin/AGP, target, artifact hash | Rust compile is not native-product proof |
| Release manifest | Versions, selected profile, targets/ABIs, lock hashes, checksums, licenses | External reproducibility/identity |
| Tool installs | `cargo install --locked --version ...` or repository runner | No floating CI |
| Secrets | No credentials/private keys/signing material in fixtures/logs | Preserve capability/signer boundaries |

## Architecture Gate Fit

| Gate | Stack consequence |
|---|---|
| Ownership | `nostr` owns mechanics; Fava owns query/write/lifecycle; stores stay separate; wrappers hold handles, not truth. |
| Dependency direction | Values -> contracts -> implementations -> profiles. FFI projects outward; core never imports generated bindings. |
| Replaceability | Same suites cover memory, Redb, hostile, SQLite. No standard-provider private bypass/registry. |
| Failure isolation | Provider lanes, no calls under owner locks, typed stages, panic containment, deadlines, exact late-result rejection. |
| Boundedness | Bounded channels/bytes/frames/provider queues/DB results/FFI collections/observations. |
| Behavioral proof | Owner/property/model tests, crash processes, independent canary, two relays, external package, native capstones. |

## Milestone Adoption Map

| Milestone | Stack adoption | Warning |
|---|---|---|
| M1 | Existing Rust/Tokio/`nostr`; add `proptest`; finish memory behavior | Do not add network/persistence to disguise incomplete semantics. |
| M2 | Add bounded `bytes`, neutral wire/transport contracts | Bound before JSON/Nostr parsing. |
| M3 | `tokio-websockets` + platform TLS; keep Tungstenite canary | Engine/witness share no helpers or WS implementation. |
| M4 | Public contract suites/hostile fixtures | No runtime registry. |
| M5 | Redb WriteStore, signer, crash supervisor, immediate durability | Receipt advances only after durable owner boundary. |
| M6 | Narrow capability/external-signer contracts | Never expose secrets or move signer authority native-side. |
| M7 | Reuse owners/contracts/runtime | Keep routing/acquisition/provenance distinct. |
| M8 | Provider executor, hostile provider, two pinned relays | Timeout is not isolation; prove stale rejection/recovery. |
| M9 | Redb caches, Reqwest/TLS, schema/migration/crash tests | Never merge stores or hide service provenance. |
| M10 | External SQLite provider, `cargo-hack`, consumer builds | Public contracts only; no standard helpers. |
| M11 | FFI, UniFFI, XCFramework/SwiftPM, AAR/Maven, consumer tests | Bindings/compile success are insufficient. |

M0 needs no stack change; its exact relay/witness versions remain historical evidence.

## Alternatives Considered

| Category | Recommended | Alternative | Why not |
|---|---|---|---|
| Engine WebSocket | `tokio-websockets` | Reuse canary Tungstenite | Weakens witness independence. |
| Nostr layer | `nostr` primitives | `nostr-sdk` client/bindings | Competing relay/storage/subscription/lifecycle owners. |
| Standard store | Redb | SQLite only | Architecture names Redb; SQLite is stronger M10 falsifier. |
| External falsifier | SQLite | Second Redb wrapper | Different mechanics better prove independence. |
| Selection | Builders/profile crates | Plugins/locator/registry/env fallback | Hidden behavior and poor native closure. |
| Async | Bounded mpsc/watch/oneshot | Unbounded/broadcast everywhere | Concealed overload/causal loss. |
| Cancellation | Fava operation/identity | Future drop/task abort alone | Cannot prove work stopped or authorize late result. |
| FFI | UniFFI + idiomatic wrappers | Two manual bridges | Duplicated ABI drift without semantic independence. |
| Delivery | XCFramework/SwiftPM, AAR/Maven | Raw bindings/repo paths | No external installation/completeness proof. |
| Data layer | Direct provider APIs | SQLx/ORM/RocksDB framework | Extra surface without serving contracts. |
| Tests | Rust/proptest/Loom/process/native | Central Cucumber runtime | Scenarios do not replace owner evidence. |

## Dependency and Installation Policy

Add dependencies only with the first real vertical slice. Pin centrally; implementations opt in:

```toml
[workspace.dependencies]
nostr = "=0.45.3"
tokio = "=1.53.1"
tokio-util = "=0.7.19"
bytes = "=1.12.1"
futures-util = "=0.3.34"
thiserror = "=2.0.20"
serde = "=1.0.229"
tracing = "=0.1.44"

# Add with owning implementation phase only.
tokio-websockets = "=0.13.3"
reqwest = "=0.13.4"
rustls-platform-verifier = "=0.7.0"
redb = "=4.2.0"
rusqlite = "=0.40.2"
uniffi = "=0.32.0"
proptest = "=1.11.0"
```

```bash
cargo install --locked cargo-nextest --version 0.9.143
cargo install --locked cargo-deny --version 0.20.2
cargo install --locked cargo-hack --version 0.6.45
cargo install --locked cargo-ndk --version 4.1.2
```

Exact pins identify tested implementations. Upgrade in focused slices, record rationale, run proportional evidence, and raise MSRV explicitly. No floating `latest`, dynamic Maven versions, or unpinned global `uniffi-bindgen`.

## What Not to Standardize Yet

Do not hide specification-owned product choices in library defaults:

- Query windowing, partial handoff cancellation, outage backfill, full-history.
- Cache retention and recommended persistent profile.
- Write quorum/routing beyond owner contracts.
- Native OS/API floors and final ABI promise.
- Service refresh/caching beyond bounded attributable results.

## Phase Research Flags

| Phase | Confidence | Further proof/research |
|---|---|---|
| M1 | HIGH | No stack uncertainty should block semantic completion. |
| M2-M3 | MEDIUM | Payload/queue/close/TLS/hostile-frame behavior on targets. |
| M5 | MEDIUM | Redb immediate durability/platform filesystem process kills. |
| M8 | HIGH approach; MEDIUM fixtures | Pin relays; prove block/panic isolation. |
| M9 | MEDIUM | Schemas/migrations and bounded HTTP. |
| M10 | MEDIUM | Compile external provider from public crates/run suites. |
| M11 | MEDIUM | UniFFI cancel, Swift 6 Sendable, Android verifier/JNA, ABI/device processes. |

## Confidence Assessment

| Area | Confidence | Notes |
|---|---|---|
| Existing checkout | HIGH | Verified from manifests/toolchain/layout. |
| Specification fit | HIGH | Constrained by architecture/testing/plan. |
| Rust versions | MEDIUM | Official metadata/docs; can move after date. |
| Redb fit | HIGH | Architecture names providers; official durability checked; Fava crash proof pending. |
| Transport | MEDIUM | Features verified; hostile/live matrix pending. |
| Native | MEDIUM | Official approach; packaging/cancel/concurrency unproved. |
| Relays | MEDIUM | Official tags/capabilities; future binaries/config must be pinned. |

## Sources

### Repository authorities — HIGH

- [Rewrite goals](../../docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md)
- [Architecture](../../docs/spec/ARCHITECTURE.md)
- [Testing guide](../../docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md)
- [Implementation plan](../../docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md)
- [Project](../PROJECT.md)
- [Observed stack](../codebase/STACK.md)

### Official ecosystem sources — MEDIUM

- [Rust releases](https://blog.rust-lang.org/releases/), [iOS targets](https://doc.rust-lang.org/rustc/platform-support/apple-ios.html)
- [Tokio](https://crates.io/crates/tokio), [mpsc](https://docs.rs/tokio/latest/tokio/sync/mpsc/), [watch](https://docs.rs/tokio/latest/tokio/sync/watch/), [shutdown](https://tokio.rs/tokio/topics/shutdown)
- [nostr](https://crates.io/crates/nostr), [rust-nostr](https://rust-nostr.org/)
- [tokio-websockets](https://crates.io/crates/tokio-websockets), [API](https://docs.rs/tokio-websockets/latest/tokio_websockets/), [limits](https://docs.rs/tokio-websockets/latest/tokio_websockets/proto/struct.Limits.html)
- [tokio-tungstenite](https://crates.io/crates/tokio-tungstenite), [limits](https://docs.rs/tungstenite/latest/tungstenite/protocol/struct.WebSocketConfig.html)
- [Redb](https://github.com/cberner/redb), [design](https://github.com/cberner/redb/blob/master/docs/design.md), [durability](https://docs.rs/redb/latest/redb/enum.Durability.html)
- [rusqlite](https://crates.io/crates/rusqlite), [repository](https://github.com/rusqlite/rusqlite)
- [Reqwest](https://github.com/seanmonstar/reqwest/releases), [TLS](https://docs.rs/reqwest/latest/reqwest/tls/), [platform verifier](https://github.com/rustls/rustls-platform-verifier)
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/), [0.32](https://github.com/mozilla/uniffi-rs/releases/tag/v0.32.0), [async](https://mozilla.github.io/uniffi-rs/latest/futures.html), [bindings](https://mozilla.github.io/uniffi-rs/latest/bindings.html), [Swift](https://mozilla.github.io/uniffi-rs/latest/swift/xcode.html), [Kotlin](https://mozilla.github.io/uniffi-rs/latest/kotlin/gradle.html)
- [Apple XCFramework](https://developer.apple.com/documentation/xcode/creating-a-multi-platform-binary-framework-bundle), [binary Swift packages](https://developer.apple.com/documentation/xcode/distributing-binary-frameworks-as-swift-packages)
- [AGP 9.2](https://developer.android.com/build/releases/agp-9-2-0-release-notes), [Kotlin releases](https://kotlinlang.org/docs/releases.html), [cargo-ndk](https://github.com/bbqsrc/cargo-ndk/releases)
- [Proptest](https://proptest-rs.github.io/proptest/proptest/state-machine.html), [Loom](https://docs.rs/loom/latest/loom/), [Nextest](https://nexte.st/docs/configuration/test-groups/), [cargo-deny](https://embarkstudios.github.io/cargo-deny/checks/), [cargo-hack](https://github.com/taiki-e/cargo-hack)
- [nostr-rs-relay](https://github.com/scsibug/nostr-rs-relay), [strfry](https://github.com/hoytech/strfry)

Version claims are current as of the research date. Official docs establish capability; Fava executable evidence must validate every behavior/platform path.
