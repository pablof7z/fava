# Coding Conventions

**Analysis Date:** 2026-08-21

## Naming Patterns

**Files:**
- Use kebab-case for crate directories and package names, keeping the owner in the name: `crates/fava-query/`, `crates/fava-event-cache-memory/`, `falsifiers/external-null-cache/` (`Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`).
- Use snake_case for Rust module files and integration-test targets: `crates/fava/src/query_source.rs`, `crates/fava-publication/src/run.rs`, `crates/fava/tests/automatic_publication.rs`, and `crates/fava-write-store-redb/tests/process_kill.rs`.
- Use behavior-oriented kebab-case for Gherkin files rather than mirroring crate names: `features/automatic-publication.feature`, `features/explicit-live-query.feature`, and `features/write-recovery.feature` group application-visible promises.
- Keep each crate entry point at `src/lib.rs`; use `src/main.rs` only for a binary entry point such as `apps/canary/src/main.rs`. Split cohesive private machinery into named modules before a file crosses the size limit (`crates/fava/src/relay.rs`, `crates/fava-routing/src/chain.rs`, `crates/fava-write-store-memory/src/model.rs`).

**Functions:**
- Use snake_case and name the observable action or fact: `accept_materialized`, `preview_write_routes`, `record_outcome`, `recover_open`, and `run_real_relay_smoke` (`crates/fava-write-store/src/lib.rs`, `crates/fava/src/lib.rs`, `crates/fava-publication/src/run.rs`, `apps/canary/src/lib.rs`).
- Use `new` for direct construction, `builder` for staged assembly, and domain verbs for lifecycle work: `RelaySessionKey::new`, `Fava::builder`, `Publication::accept`, `Observer::open`, and `RouterSession::next_change` (`crates/fava-state/src/lib.rs`, `crates/fava/src/lib.rs`, `crates/fava-publication/src/lib.rs`, `crates/fava-routing/src/lib.rs`).
- Prefix predicates with `is_` or `has_` and use noun accessors for exact values: `is_empty`, `has_executor`, `current`, `revision` (`crates/fava-event-cache/src/lib.rs`, `apps/canary/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`).
- Name tests as complete behavioral statements in snake_case, such as `reconnect_uses_fresh_identity_and_rejects_old_subscription_frames`, `slow_receipt_consumer_gets_explicit_lag_instead_of_silent_loss`, and `every_m5_commit_and_effect_boundary_survives_sigkill_exactly` (`crates/fava/tests/multi_relay.rs`, `crates/fava/tests/write_bounds.rs`, `crates/fava-write-store-redb/tests/process_kill.rs`).

**Variables:**
- Use descriptive snake_case tied to exact responsibilities: `event_cache`, `write_store`, `relay_evidence`, `receipt_id`, and `source_revision` make ownership explicit (`crates/fava/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`).
- Keep identity and lifecycle qualifiers in names when they affect correctness: `next_identity`, `current_subscription`, `route_revision`, `generation`, and `cancel_rx` distinguish otherwise similar facts (`crates/fava-write-store-redb/src/lib.rs`, `crates/fava/src/relay.rs`, `crates/fava-publication/src/run.rs`).
- Use `SCREAMING_SNAKE_CASE` for constants, including owner-local bounds and process protocol keys such as `MAX_ROUTERS`, `MAX_DESTINATIONS`, `ATTEMPT_TIMEOUT`, and `CHILD_BOUNDARY` (`crates/fava-routing/src/chain.rs`, `crates/fava-publication/src/run.rs`, `crates/fava-write-store-redb/tests/process_kill.rs`).

**Types:**
- Use approved UpperCamelCase nouns for structs, enums, and traits: `Query`, `RoutePlan`, `WriteIntent`, `Receipt`, and `RelaySessionKey` (`docs/internals/vocabulary.toml`, `crates/fava-query/src/lib.rs`, `crates/fava-routing/src/lib.rs`, `crates/fava-write/src/lib.rs`).
- Suffix public failure types with `Error` and terminal facts with a precise noun: `RouterError`, `PublicationError`, `WriteStoreError`, `ObservationClosed`, and `CanaryError` (`crates/fava-routing/src/lib.rs`, `crates/fava-publication/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/lib.rs`).
- Wrap semantic identities in small newtypes instead of passing raw integers: `WriteId`, `ReceiptId`, `SourceRevision`, and `QueryRevision` preserve meaning at boundaries (`crates/fava-write/src/lib.rs`, `crates/fava-query/src/lib.rs`).
- Derive only traits justified by value semantics; immutable semantic values commonly derive `Clone`, `Debug`, `Eq`, `Hash`, and ordering traits, while lifecycle owners such as `Observer` and `Observation` do not (`crates/fava-state/src/lib.rs`, `crates/fava-observe/src/lib.rs`).

## Code Style

**Formatting:**
- Run `cargo fmt --all -- --check` or `bazel build //... --config=fmt-check`; `rustfmt.toml` fixes Rust 2024, 100-column width, field-init shorthand, and try shorthand (`rustfmt.toml`, `.bazelrc`).
- Keep code within the repository size policy: 500 lines is the soft limit and 800 lines is the hard limit. The largest tracked Rust source is `crates/fava-query/src/lib.rs` at exactly 500 lines, so extend it by extracting a cohesive owner-preserving module (`AGENTS.md`).
- Prefer deterministic standard-library collections where observable order or identity matters; query selection, route plans, receipt destinations, and relay evidence use `BTreeMap` and `BTreeSet` (`crates/fava-query/src/lib.rs`, `crates/fava-routing/src/lib.rs`, `crates/fava-write/src/lib.rs`).
- Keep bounds beside the owner that enforces them rather than in a generic constants module: routing bounds live in `crates/fava-routing/src/chain.rs`; write and durable receipt bounds live in `crates/fava-write/src/lib.rs` and `crates/fava-write-store/src/lib.rs`.

**Linting:**
- Every workspace crate inherits `[workspace.lints]`; `unsafe_code = "forbid"`, `missing_docs = "warn"`, and Clippy `all` plus `pedantic` are the baseline (`Cargo.toml`, `crates/fava-query/Cargo.toml`).
- Treat lint warnings as failures with `cargo clippy --workspace --all-targets -- -D warnings` or `bazel build //... --config=clippy` (`.bazelrc`, `docs/issues/0008-automatic-write-routing.md`).
- Standalone workspaces must declare or run their own lint policy: `apps/canary/Cargo.toml` repeats the workspace lint set, while `falsifiers/external-null-cache/Cargo.toml` is validated with an explicit `cargo clippy ... -D warnings` command recorded in `docs/issues/0001-local-source-merge.md`.
- Use narrowly scoped lint exceptions with a rationale. Current examples are object-safe future type complexity in `crates/fava-transport/src/lib.rs`, route-chain type complexity in `crates/fava-routing/src/lib.rs`, and cohesive orchestration arguments in `crates/fava/src/relay.rs`.

## Import Organization

**Order:**
1. Import `std` modules first and separate them with a blank line from non-`std` imports (`crates/fava-query/src/lib.rs`, `apps/canary/src/lib.rs`).
2. Group workspace crates and external crates together, keeping related items in one braced import rather than repeated one-item imports (`crates/fava-observe/src/lib.rs`, `crates/fava/tests/local_source_merge.rs`).
3. In test modules, import test-only collaborators, then bring the parent module into scope with `use super::*` or a narrow `use super::{...}` (`crates/fava-observe/src/lib.rs`, `apps/canary/src/lib.rs`).

**Path Aliases:**
- Cargo package names are imported through their underscore crate identifiers, such as `fava_event_cache`, `fava_query_standard`, and `fava_write_store_memory`; no source-level path alias system is configured (`Cargo.toml`, `crates/fava/tests/local_source_merge.rs`).
- Re-export public semantic types from their owning crate or the thin facade instead of duplicating definitions: `fava-state` re-exports Nostr event types and `fava` re-exports selected query-facing values (`crates/fava-state/src/lib.rs`, `crates/fava/src/lib.rs`).

## Error Handling

**Patterns:**
- Return `Result` with typed, scoped errors at public library boundaries; `thiserror::Error` enums carry precise refusal variants in `crates/fava-query/src/lib.rs`, `crates/fava-event-cache/src/lib.rs`, and `crates/fava-write-store/src/lib.rs`.
- Use `#[error(transparent)]` and `#[from]` when one owner exposes another owner's refusal without erasing its type, as in `ObserveError` and `WriteStoreError` (`crates/fava-observe/src/lib.rs`, `crates/fava-write-store/src/lib.rs`).
- Map implementation failures to the provider's scoped refusal instead of panicking: poisoned locks and exhausted counters become `Refused(String)` errors in `crates/fava-event-cache-memory/src/lib.rs` and `crates/fava-write-store-memory/src/lib.rs`.
- Refuse invalid or oversized input before opening work or committing partial state: query construction, route contribution validation, relay fanout, and receipt mutation return exact errors (`crates/fava-query/src/lib.rs`, `crates/fava-routing/src/chain.rs`, `crates/fava-write/src/lib.rs`, `crates/fava-write-store/src/lib.rs`).
- Use `?` for propagation in production code; reserve `expect` for tests and locally proven constants with causal messages (`crates/fava/tests/local_source_merge.rs`, `crates/fava-event-cache-memory/src/lib.rs`).
- The canary intentionally collapses heterogeneous orchestration failures into `CanaryError(String)`, records the failure, and has `apps/canary/src/main.rs` print once and exit non-zero (`apps/canary/src/lib.rs`, `apps/canary/src/main.rs`).

## Logging

**Framework:** No application logging dependency is used; library crates expose typed results and facts, while the canary writes evidence artifacts directly (`Cargo.toml`, `apps/canary/src/artifacts.rs`).

**Patterns:**
- Keep core crates silent; no `println!`, `eprintln!`, or logging facade appears under `crates/`, so new library behavior should return typed state/errors rather than emit process output (`crates/fava/src/lib.rs`, `crates/fava-observe/src/lib.rs`).
- Record external-effect evidence as flushed JSONL plus reports, process facts, wire frames, and artifact hashes through `RunArtifacts` (`apps/canary/src/artifacts.rs`, `apps/canary/src/proxy.rs`).
- Restrict terminal output to the CLI boundary and last-resort proxy task diagnostics (`apps/canary/src/main.rs`, `apps/canary/src/proxy.rs`).

## Comments

**When to Comment:**
- Start each Rust module with `//!` explaining its owned role, as in `crates/fava-query-standard/src/lib.rs` and `apps/canary/src/wire.rs`.
- Use inline comments for non-obvious design constraints or lint exceptions, not to narrate mechanics; see the object-safe future rationale in `crates/fava-transport/src/lib.rs` and build-authority notes in `MODULE.bazel`.
- Put durable product meaning and deliberate-break descriptions in behavior files, not implementation comments (`features/local-source-merge.feature`, `features/relay-lab.feature`).

**JSDoc/TSDoc:**
- Not applicable; Rustdoc is required for public APIs by `missing_docs = "warn"` in `Cargo.toml` and `apps/canary/Cargo.toml`.
- Document public fallible functions with a `# Errors` section and link named error types or variants (`crates/fava-query/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, `apps/canary/src/lib.rs`).
- Mark constructors, builders, accessors, and immutable transforms with `#[must_use]` when silently discarding the result would be suspicious (`crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava/src/lib.rs`).

## Function Design

**Size:** Keep one decision or lifecycle step per function and extract protocol, process, and artifact responsibilities into owner modules; `apps/canary/src/lib.rs` delegates to `apps/canary/src/wire.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, and `apps/canary/src/artifacts.rs`.

**Parameters:**
- Prefer precise structs for multi-field operations (`SmokeOptions`, `ReconOptions`) and generic iterators for value collections (`Query::authors`, `Query::from_relays`) (`apps/canary/src/lib.rs`, `apps/canary/src/recon.rs`, `crates/fava-query/src/lib.rs`).
- Accept provider implementations through public traits and `Arc`, keeping universal owners independent from concrete providers (`crates/fava/src/lib.rs`, `crates/fava-observe/src/lib.rs`).
- Use consuming builder methods returning `Self` for declarative configuration and validate at the transition into work (`crates/fava-query/src/lib.rs`, `crates/fava/src/lib.rs`).

**Return Values:**
- Return immutable current-state snapshots behind `Arc` for live observations and explicit typed facts for accepted writes (`crates/fava-observe/src/lib.rs`, `crates/fava-write-store/src/lib.rs`).
- Preserve exact absence and refusal distinctions with `Option` inside `Result` rather than sentinel values (`crates/fava-event-cache/src/lib.rs`, `crates/fava-write-store/src/lib.rs`).

## Module Design

**Exports:**
- Put shared values in their semantic-owner crate and keep contracts separate from implementations; the active split is visible in `crates/fava-query/`, `crates/fava-event-cache/`, `crates/fava-event-cache-memory/`, `crates/fava-write-store/`, and `crates/fava-write-store-memory/` (`AGENTS.md`, `docs/spec/ARCHITECTURE.md`).
- Keep `fava` a thin public assembly facade that re-exports only application-facing values and accepts providers through neutral contracts (`crates/fava/src/lib.rs`).
- Keep canary implementation modules private and expose only scenario inputs, outcomes, registry access, and runners (`apps/canary/src/lib.rs`).

**Barrel Files:**
- Rust crate roots at `src/lib.rs` are the only barrel-like surface; no nested `mod.rs` barrels are present (`crates/fava/src/lib.rs`, `apps/canary/src/lib.rs`).
- Prefer explicit `pub use` from the owning crate over a generic common/prelude module (`crates/fava-state/src/lib.rs`, `crates/fava/src/lib.rs`, `AGENTS.md`).

## Repository-Specific Guardrails

- Write observable behavior, executable evidence, and then production code; confirm the evidence is red before implementation and under its named deliberate break (`AGENTS.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Keep each mutable fact and lifecycle under one owner; dependencies flow from semantic values to neutral contracts to providers (`AGENTS.md`, `docs/spec/ARCHITECTURE.md`).
- Keep externally influenced inputs, outputs, queues, observations, and retained evidence bounded or return typed refusal/shortfall (`AGENTS.md`, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, `crates/fava-routing/src/chain.rs`).
- Do not add hidden runtime feature flags or compatibility behavior; provider selection is static assembly through public contracts (`AGENTS.md`, `crates/fava/src/lib.rs`).
- Use exact operation, subscription, receipt, route revision, and generation identity for late completions; keep cancellation, failure, and provider closure attributable to their owner (`AGENTS.md`, `crates/fava/src/relay.rs`, `crates/fava-publication/src/run.rs`, `crates/fava-observe/src/lib.rs`).

## Architectural Vocabulary

- Treat `docs/internals/vocabulary.toml` as a closed registry for concepts, public Rust symbols, specified public symbols, and crate names. Run `python3 tools/check_vocabulary.py` and `python3 -m unittest tools/tests/test_vocabulary_check.py` for every architectural or public-API change (`AGENTS.md`, `tools/check_vocabulary.py`).
- A new crate, public or cross-crate nominal type, provider contract, persisted entity, configuration concept, lifecycle owner, synonym, wrapper, or adjective-qualified variant requires a separate focused architecture change approved by Pablo (`AGENTS.md`).
- Keep contracts separate from implementations even with one provider. Active examples are `fava-transport` / `fava-transport-websocket`, `fava-signer` / `fava-signer-local`, `fava-publisher` / `fava-publisher-nip01`, and `fava-delivery` / `fava-delivery-standard` (`Cargo.toml`, `AGENTS.md`).
- Keep policy out of neutral cores: NIP-65 parsing is in `crates/fava-nip65/`; outbox, hints, app-relay, and fallback policies live in their own router crates; ordered composition stays in `crates/fava-routing/` (`docs/issues/0008-automatic-write-routing.md`).

## Implemented Scope Boundary

- Treat M0 through M6 as implemented and evidenced. Their complete issue records are `docs/issues/0002-m0-evidence-foundation.md`, `docs/issues/0001-local-source-merge.md`, and `docs/issues/0004-explicit-live-query.md` through `docs/issues/0008-automatic-write-routing.md`; their behavior files use built status and `apps/canary/scenarios.json` enables scenarios through M6.
- Treat M7 through M11 as specified only. Their goals and candidate artifacts occur in `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, but no M7+ scenario is registered in `apps/canary/scenarios.json`; do not cite planned signatures, crates, protocol services, native SDKs, or parity artifacts as existing conventions.

---

*Convention analysis: 2026-08-21*
