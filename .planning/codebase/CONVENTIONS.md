# Coding Conventions

**Analysis Date:** 2026-08-20

## Naming Patterns

**Files:**
- Use kebab-case for crate directories and package names, keeping the semantic owner in the name: `crates/fava-query/`, `crates/fava-event-cache-memory/`, and `falsifiers/external-null-cache/` are the current patterns (`Cargo.toml`, `falsifiers/external-null-cache/Cargo.toml`).
- Use snake_case for Rust module files and integration-test targets: `apps/canary/src/artifacts.rs`, `apps/canary/src/relay.rs`, and `crates/fava/tests/local_source_merge.rs` are representative.
- Use behavior-oriented kebab-case for Gherkin files rather than mirroring crate names: `features/local-source-merge.feature` and `features/relay-lab.feature` group application-visible promises.
- Keep each crate entry point at `src/lib.rs`; use `src/main.rs` only for a binary entry point such as `apps/canary/src/main.rs` (`crates/fava/src/lib.rs`).

**Functions:**
- Use snake_case and name the observable action or fact: `accept_materialized`, `coordinate_for_event`, `run_real_relay_smoke`, and `require_complete_query` are the established pattern (`crates/fava-write-store/src/lib.rs`, `crates/fava-state/src/lib.rs`, `apps/canary/src/lib.rs`).
- Use `new` for direct construction, `builder` for staged assembly, and domain verbs for lifecycle work: `RelaySessionKey::new`, `Fava::builder`, `Observer::open`, and `Observation::close` are the current examples (`crates/fava-state/src/lib.rs`, `crates/fava/src/lib.rs`, `crates/fava-observe/src/lib.rs`).
- Prefix predicates with `is_` or `has_` and use noun accessors for exact values: `is_empty`, `has_executor`, `current`, and `revision` follow this rule (`crates/fava-event-cache/src/lib.rs`, `apps/canary/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `crates/fava-query/src/lib.rs`).
- Name tests as complete behavioral statements in snake_case, such as `accepted_local_event_is_visible_without_cache_pollution` and `second_source_open_failure_closes_the_first_source` (`crates/fava/tests/local_source_merge.rs`, `crates/fava-observe/src/lib.rs`).

**Variables:**
- Use descriptive snake_case tied to semantic roles: `event_cache`, `write_store`, `relay_evidence`, `receipt_id`, and `source_revision` make ownership explicit (`crates/fava/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava-write/src/lib.rs`).
- Keep identity and lifecycle qualifiers in names when they affect correctness: `next_identity`, `cache_open`, `writes_open`, `generation`, and `started_unix_ms` distinguish otherwise similar facts (`crates/fava-write-store-memory/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/lib.rs`).
- Use `SCREAMING_SNAKE_CASE` for constants, including local bounds such as `RELAY_VERSION` and `FRAME_LIMIT` (`apps/canary/src/relay.rs`, `apps/canary/src/wire.rs`).

**Types:**
- Use UpperCamelCase nouns for structs, enums, and traits: `EventQuery`, `QuerySource`, `SourceSnapshot`, and `MemoryEventCache` are representative (`crates/fava-query/src/lib.rs`, `crates/fava-event-cache-memory/src/lib.rs`).
- Suffix public failure types with `Error` and terminal facts with a precise noun: `QueryError`, `QuerySourceError`, `ObservationClosed`, and `CanaryError` (`crates/fava-query/src/lib.rs`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/lib.rs`).
- Wrap semantic identities in small newtypes instead of passing raw integers: `WriteId`, `ReceiptId`, `SourceRevision`, and `QueryRevision` preserve meaning at boundaries (`crates/fava-write/src/lib.rs`, `crates/fava-query/src/lib.rs`).
- Derive only traits justified by value semantics; immutable semantic values commonly derive `Clone`, `Debug`, `Eq`, `Hash`, and ordering traits, while lifecycle owners such as `Observer` and `Observation` do not (`crates/fava-state/src/lib.rs`, `crates/fava-observe/src/lib.rs`).

## Code Style

**Formatting:**
- Run `cargo fmt --all -- --check`; formatting is controlled by `rustfmt.toml` with Rust 2024 edition, 100-column width, field-init shorthand, and try shorthand (`rustfmt.toml`, `docs/issues/0001-local-source-merge.md`).
- Keep code within the repository size policy: 500 lines is the soft limit and 800 lines is the hard limit for code files; a file above 500 lines needs a concrete cohesion reason (`AGENTS.md`).
- Prefer deterministic standard-library collections where observable order or identity matters; current semantic code uses `BTreeMap` and `BTreeSet` throughout query and state evaluation (`crates/fava-state/src/lib.rs`, `crates/fava-query-standard/src/lib.rs`).

**Linting:**
- Every workspace crate inherits `[workspace.lints]`; `unsafe_code = "forbid"`, `missing_docs = "warn"`, and Clippy `all` plus `pedantic` are the baseline (`Cargo.toml`, `crates/fava-query/Cargo.toml`).
- Treat lint warnings as failures in validation with `cargo clippy --workspace --all-targets -- -D warnings` (`docs/issues/0001-local-source-merge.md`).
- Standalone workspaces must declare or run their own lint policy: `apps/canary/Cargo.toml` repeats the workspace lint set, while `falsifiers/external-null-cache/Cargo.toml` is validated with an explicit `cargo clippy ... -D warnings` command recorded in `docs/issues/0001-local-source-merge.md`.
- Use narrowly scoped lint exceptions with a rationale; the current example preserves the specified asynchronous facade with `#[allow(clippy::unused_async)]` next to the reason (`crates/fava/src/lib.rs`).

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
- Refuse invalid inputs before opening work: empty relay sets, mismatched source authority, zero limits, and missing event IDs become typed errors in `crates/fava-query/src/lib.rs` and `crates/fava-write/src/lib.rs`.
- Use `?` for propagation in production code; reserve `expect` for tests and locally proven constants with causal messages, as shown in `crates/fava/tests/local_source_merge.rs` and `crates/fava-event-cache-memory/src/lib.rs`.
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
- Use inline comments for non-obvious design constraints or lint exceptions, not to narrate mechanics; the async-facade rationale in `crates/fava/src/lib.rs` is the current model.
- Put durable product meaning and deliberate-break descriptions in behavior files, not implementation comments (`features/local-source-merge.feature`, `features/relay-lab.feature`).

**JSDoc/TSDoc:**
- Not applicable; Rustdoc is required for public APIs by `missing_docs = "warn"` in `Cargo.toml` and `apps/canary/Cargo.toml`.
- Document public fallible functions with a `# Errors` section and link named error types or variants (`crates/fava-query/src/lib.rs`, `crates/fava-write-store/src/lib.rs`, `apps/canary/src/lib.rs`).
- Mark constructors, builders, accessors, and immutable transforms with `#[must_use]` when silently discarding the result would be suspicious (`crates/fava-state/src/lib.rs`, `crates/fava-query/src/lib.rs`, `crates/fava/src/lib.rs`).

## Function Design

**Size:** Keep one decision or lifecycle step per function and extract protocol, process, and artifact responsibilities into owner modules; `apps/canary/src/lib.rs` delegates to `apps/canary/src/wire.rs`, `apps/canary/src/relay.rs`, `apps/canary/src/proxy.rs`, and `apps/canary/src/artifacts.rs`.

**Parameters:**
- Prefer semantic structs for multi-field operations (`SmokeOptions`, `ReconOptions`) and generic iterators for value collections (`EventQuery::authors`, `EventQuery::from_relays`) (`apps/canary/src/lib.rs`, `apps/canary/src/recon.rs`, `crates/fava-query/src/lib.rs`).
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
- Prefer explicit `pub use` from the semantic owner over a generic common/prelude module (`crates/fava-state/src/lib.rs`, `crates/fava/src/lib.rs`, `AGENTS.md`).

## Repository-Specific Guardrails

- Write observable behavior, executable evidence, and then production code; confirm the evidence is red before implementation and under its named deliberate break (`AGENTS.md`, `docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md`).
- Keep each mutable fact and lifecycle under one owner; dependencies flow from semantic values to neutral contracts to providers (`AGENTS.md`, `docs/spec/ARCHITECTURE.md`).
- Keep externally influenced inputs, outputs, queues, observations, and retained evidence bounded or return typed refusal/shortfall (`AGENTS.md`, `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`).
- Do not add hidden runtime feature flags or compatibility behavior; provider selection is static assembly through public contracts (`AGENTS.md`, `crates/fava/src/lib.rs`).
- Use exact operation and generation identity for late completions and make cancellation, failure, and provider closure attributable to the owning work (`AGENTS.md`, `crates/fava-observe/src/lib.rs`, `apps/canary/src/relay.rs`).

---

*Convention analysis: 2026-08-20*
