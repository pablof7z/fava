---
phase: 07-semantic-writes-and-capability-composition
verified: 2026-08-21T17:45:03Z
status: passed
score: 12/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
implementation_head: f97ecd8c0f8fd3793860cce95380ddcae9521aa3
verified_head: 1dd7e5e7cd6dce0cec90829ac8e116ff19a081a0
decision_coverage:
  honored: 0
  total: 0
  not_honored: []
human_verification: []
---

# Phase 7: Semantic Writes and Capability Composition Verification Report

**Phase Goal:** As a Fava application developer, I want to express replaceable-event edits through independent protocol crates, so that they reuse one durable publication lifecycle and survive source-state changes.
**Verified:** 2026-08-21T17:45:03Z
**Status:** PASSED
**Re-verification:** No — initial goal-backward verification
**Implementation head:** `f97ecd8c0f8fd3793860cce95380ddcae9521aa3`
**Verified documentation head:** `1dd7e5e7cd6dce0cec90829ac8e116ff19a081a0`

## User Flow Coverage

User story: “As a Fava application developer, I want to express replaceable-event edits through independent protocol crates, so that they reuse one durable publication lifecycle and survive source-state changes.”

| Step | Expected | Codebase evidence | Status |
|---|---|---|---|
| Construct an operation | A developer calls a protocol helper and receives an authorless edit | `fava-nip02/src/lib.rs:37-55`; `fava-bookmarks/src/lib.rs:46-90`; opposing-operation unit tests | VERIFIED |
| Select protocol meaning | Application assembly supplies the materializer without giving it lifecycle ownership | `fava/src/lib.rs:340-357`; `fava-write/src/materialization.rs:25-45`; Cargo/Bazel provider-path checks are empty | VERIFIED |
| Accept and publish | The edit is accepted with one resolved author, materialized, committed, routed, signed, and delivered through the ordinary receipt | `fava-publication/src/lib.rs:87-128`; named first-value and author-custody tests passed | VERIFIED |
| Survive source changes | A newer qualified source rematerializes the edit while preserving unrelated state and stable receipt identity | `fava-publication/src/run.rs:238-300`; named rematerialization and store-CAS tests passed | VERIFIED |
| Ignore retired work | Old generation completions remain attributable but cannot mutate current state | `fava-write-store/src/receipt.rs:57-80`; exact retired-completion and route-revision tests passed | VERIFIED |
| Outcome | Independent protocol crates reuse one durable lifecycle and continue across source changes | NIP-02/bookmarks shared corpus, external N+1 lifecycle, SIGKILL recovery, and four public-Fava CLI canaries passed | VERIFIED |

## Goal Achievement

The current authoritative edit is `{ kind, identifier, change }`: no author, stored inverse, or encoding version. Acceptance separately freezes the author. Older PLAN wording about an edit-owned author/inverse and global addressable refusal was superseded by the current GOALS/ARCHITECTURE contract; verification follows the authority order in `AGENTS.md` and checks opposing edits plus addressable identifiers.

### Observable Truths

| # | Truth | Status | Evidence |
|---:|---|---|---|
| 1 | Protocol crates expose ordinary event values or authorless edits and opposing operations without owning signing, routing, delivery, or receipts | VERIFIED | Public NIP-02/bookmark helpers; exact public-surface tests; normal dependency graphs exclude lifecycle/provider crates |
| 2 | Acceptance freezes and persists the author once; every generation uses it | VERIFIED | Edit shape in `fava-write/src/edit.rs:12-16`; custody tuple in memory/redb; `author::accepted_author_scopes_sources_signing_and_every_generation` passed |
| 3 | A first-value edit materializes and publishes through the ordinary receipt | VERIFIED | `Publication::accept` semantic branch; `first_value_edit_publishes_through_public_fava` passed; verifier CLI first-value bundle passed |
| 4 | New qualified source state rematerializes a live edit, preserving unrelated changes and stable operation/receipt identity | VERIFIED | Source selection and runner CAS wiring; `newer_source_rematerializes_once_and_preserves_unrelated_fields` and `memory_generation_swap_is_compare_and_set` passed |
| 5 | Retired signer, route, publisher, and delivery work is attributable and inert | VERIFIED | Exact current-generation validation is used by lifecycle mutations; retired-completion and delayed-route tests passed |
| 6 | Neutral edit/materializer boundaries are bounded, authorless, addressable, and reject malformed or mismatched values before effects | VERIFIED | `ReplaceableEventEdit::new`, materializer output validation, contract/failure/redb tests, and vocabulary gate |
| 7 | The write store solely owns atomic semantic custody, one coordinate owner, generations, receipt identity, failure evidence, and recovery | VERIFIED | Neutral `WriteStore` contract plus memory/redb implementations; admission/current-guard/failure/recovery behavioral tests |
| 8 | Preview/live selection and routing share one path; provider failure, panic, cancellation, capacity, and stale work remain scoped | VERIFIED | `prepare_semantic`/`materialize_and_route` shared path; publication/failure targets contain explicit behavioral coverage |
| 9 | Durable semantic custody survives strict-schema redb reopen and real SIGKILL without duplicate resumption | VERIFIED | Schema v2 refuses mismatch; `semantic::semantic_successor_and_failed_source_resume_once` passed independently |
| 10 | Two unrelated in-tree protocols satisfy the same public lifecycle corpus without core kind switches | VERIFIED | Both exact shared-corpus tests passed; universal-owner switch scan returned empty |
| 11 | An external N+1 capability and raw future kinds work through public Fava without universal-core behavior changes | VERIFIED | External crate has sole normal dependency `fava`; external lifecycle and raw-kind tests passed independently |
| 12 | Feature mapping, causal deliberate breaks, four CLI canaries, vocabulary, dependency, bounds, line, Cargo, and Bazel exit gates are executable and current | VERIFIED | Mapper 10/10; issue break records; verifier reran all four CLIs; vocabulary and dependency gates passed; final audits target implementation head `f97ecd8` |

**Score:** 12/12 truths verified (0 present-but-behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/fava-write/src/edit.rs` | Bounded authorless durable edit and accepted-author door | VERIFIED | Substantive; re-exported by `fava-write` and `fava`; used by publication, stores, protocols, external consumer |
| `crates/fava-write/src/materialization.rs` | Neutral materializer contract and generation identity | VERIFIED | Substantive; selected through public `FavaBuilder` |
| `crates/fava-write-store/src/lib.rs` | Replaceable store contract | VERIFIED | Memory/redb implement every semantic mutation and recovery seam |
| `crates/fava-write-store-memory/src/semantic.rs` | Atomic in-memory custody and CAS | VERIFIED | Query-source snapshots emitted only after committed state mutation |
| `crates/fava-write-store-redb/src/{semantic,schema}.rs` | Transactional durable custody and strict schema | VERIFIED | Schema v2; no compatibility decoder; recovery validation is wired at open |
| `crates/fava-publication/src/{lib,materialization,run}.rs` | Selection, materialization, rematerialization, and ordinary lifecycle | VERIFIED | Public accept/preview/recover paths use the same semantic owner |
| `crates/fava-nip02/src/lib.rs` | First protocol capability | VERIFIED | Follow/unfollow/materializer only; exact tests and public API compile checks |
| `crates/fava-bookmarks/src/lib.rs` | Unrelated second capability | VERIFIED | Event/coordinate bookmark opposing operations; exact tests and public API checks |
| `falsifiers/external-semantic-capability` | Public-only N+1 and raw-kind proof | VERIFIED | Outside root workspace; sole normal dependency is `fava`; lifecycle tests passed |
| `apps/canary/src/semantic_writes.rs` | Public-Fava process evidence | VERIFIED | Four registered M7 scenarios produced bounded seven-file bundles |
| `features/semantic-writes.feature` | Observable promise mapping | VERIFIED | Mapper resolves all seven scenarios to exact listed Cargo tests |
| `docs/issues/0010-m7-semantic-writes-and-capability-composition.md` | Causal falsifier record | VERIFIED | Exact break PASS records and checksum restoration are present |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| Protocol helper | `ReplaceableEventEdit` | `follow`/`unfollow` and bookmark helpers | WIRED | Public helper output enters `WriteIntent::edit_as` |
| `FavaBuilder` | `Publication` | selected `Arc<dyn ReplaceableEventMaterializer>` values | WIRED | No private default or core kind dispatch |
| `Publication::accept` | `WriteStore` | reservation, shared materialization/route, atomic reserved acceptance | WIRED | Provider effects cannot steal unreserved store capacity |
| Event-cache and write-store sources | rematerialization runner | exact query, deterministic winner, source-change loop | WIRED | Independent source closure/failure coverage exists |
| Runner completions | current receipt | write/receipt/materialization/event/session/attempt/revision CAS | WIRED | Retired completions refuse without current-state mutation |
| redb schema | recovery runner | persisted edit + author + source/current generation | WIRED | Materializers validated before recovered work starts |
| External N+1 crate | public `fava` | public materializer selection and write lifecycle | WIRED | No root workspace membership or private publication dependency |

### Data-Flow Trace (Level 4)

| Artifact | Data | Real source | Sink | Status |
|---|---|---|---|---|
| Protocol crates | Opaque change bytes | Caller operation (`follow`, bookmark, external insert/remove) | Accepted edit custody | FLOWING |
| Publication | Qualified source event | Independent event-cache/write-store snapshots | Protocol materializer and current local event | FLOWING |
| Memory/redb stores | Edit, accepted author, selected source, current generation | Atomic acceptance/CAS or durable row | Receipt query source and recovery | FLOWING |
| Query merge | Current local materialization | Write-store committed snapshot | Public observation | FLOWING |
| Canary evidence | Receipt/events/materialization/source/route facts | Public Fava execution | Seven-file bounded evidence bundle | FLOWING |

### Behavioral Spot-Checks

All commands ran against code-equivalent implementation head `f97ecd8`; current head `1dd7e5e` changes phase documentation only.

| Behavior | Command | Result | Status |
|---|---|---|---|
| Author frozen across generations | `cargo test -p fava --test semantic_write_publication author::accepted_author_scopes_sources_signing_and_every_generation -- --exact` | 1/1 | PASS |
| First-value publication | `cargo test -p fava --test semantic_write_publication first_value_edit_publishes_through_public_fava -- --exact` | 1/1 | PASS |
| Source-v2 rematerialization | `cargo test -p fava --test semantic_write_publication newer_source_rematerializes_once_and_preserves_unrelated_fields -- --exact` | 1/1 | PASS |
| Retired completion inertness | `cargo test -p fava --test semantic_write_publication interleavings::retired_completion_is_attributable_and_inert -- --exact` | 1/1 | PASS |
| Dropped-notification route recovery | `cargo test -p fava --test semantic_write_publication route_revision::delayed_route_after_rematerialization_commits_newer_revision -- --exact` | 1/1 | PASS |
| Atomic generation replacement | `cargo test -p fava --test semantic_write_store memory_generation_swap_is_compare_and_set -- --exact` | 1/1 | PASS |
| Author-separated custody | `cargo test -p fava --test semantic_write_store author::same_authorless_edit_has_independent_author_custody -- --exact` | 1/1 | PASS |
| Exact current guard | `cargo test -p fava --test semantic_write_store current_guard::memory_exact_current_guard_precedes_idempotence -- --exact` | 1/1 | PASS |
| NIP-02 shared corpus | exact `semantic_write_capabilities` test | 1/1 | PASS |
| Bookmarks shared corpus | exact `semantic_write_capabilities` test | 1/1 | PASS |
| External N+1 lifecycle | exact external `public_capability` test | 1/1 | PASS |
| Raw future kind unchanged | exact external `public_capability` test | 1/1 | PASS |
| SIGKILL successor/failure recovery | exact redb `process_kill` test | 1/1 | PASS |

### Probe Execution

No `scripts/**/tests/probe-*.sh` files or documented probe paths exist for this phase. The phase's four executable CLI canaries were run directly by this verifier under `/tmp/m7-verifier.Phorir`:

| Probe | Result | Status |
|---|---|---|
| `replaceable-edit-first-value` | 7 files, 3,569 bytes, exact current revision | PASS |
| `replaceable-edit-rematerialization` | 7 files, 5,570 bytes, exact current revision | PASS |
| `replaceable-edit-opposing-operations` | 7 files, 9,430 bytes, exact current revision | PASS |
| `protocol-crate-n-plus-one` | 7 files, 4,113 bytes, exact current revision | PASS |

Each manifest had the exact scenario, six artifact hashes, a 64-character seed hash, and current `fava_revision`/`canary_revision` `1dd7e5e7cd6dce0cec90829ac8e116ff19a081a0`.

### Requirements Coverage

| Requirement | Source plans | Status | Executable evidence |
|---|---|---|---|
| CAP-01 | 01, 03, 08, 09 | SATISFIED | Protocol helpers, first-value public path, opposing-operation corpus/CLI |
| CAP-02 | 01–04, 08, 09 | SATISFIED | Author custody/store/recovery/publication tests |
| CAP-03 | 01–05, 08, 09 | SATISFIED | No-prior materializer/store/publication/CLI tests |
| CAP-04 | 03, 08, 09 | SATISFIED | Qualified-source rematerialization and preservation tests |
| CAP-05 | 02–05, 08, 09 | SATISFIED | Stable write/receipt CAS, recovery, and SIGKILL tests |
| CAP-06 | 06, 08, 09 | SATISFIED | Exact stale-completion, cancellation, route-revision, and canary evidence |
| CAP-07 | 06, 08, 09 | SATISFIED | NIP-02 and bookmarks pass one parameterized public corpus |
| CAP-08 | 06–09 | SATISFIED | External public-only N+1, metadata, Cargo-tree, Bazel negative paths |
| CAP-09 | 07–09 | SATISFIED | Raw kind 50001 preserves caller timestamp/tags/content/identity through publication |

No Phase 7 requirement is orphaned; CAP-01 through CAP-09 are all claimed by plans and implemented.

### Test Quality Audit

| Test group | Linked requirements | Active | Skipped | Circular | Strongest assertion | Verdict |
|---|---|---:|---:|---:|---|---|
| `fava/semantic_write_contract` | CAP-01–03,05 | 4 | 0 | 0 | Value/contract | PASS |
| `fava/semantic_write_store` | CAP-02–05 | 11 | 0 | 0 | Behavioral state transition | PASS |
| `fava/semantic_write_publication` | CAP-01–06 | 19 | 0 | 0 | End-to-end behavioral | PASS |
| `fava/semantic_write_failures` | CAP-02–06 | 14 | 0 | 0 | Behavioral hostile path | PASS |
| redb semantic/restart | CAP-02–06 | 16 + 6 | 0 | 0 | Transaction/restart/SIGKILL | PASS |
| NIP-02 and bookmarks | CAP-01,07 | 7+1 and 9+1 | 0 | 0 | Value/behavioral | PASS |
| shared capability corpus | CAP-01,03–08 | 4 | 0 | 0 | End-to-end behavioral | PASS |
| external capability | CAP-08,09 | 3+3 | 0 | 0 | Public-consumer lifecycle | PASS |
| canary library/CLI | CAP-01–09 | 18 plus 4 CLI | 0 | 0 | Process/evidence behavioral | PASS |
| feature mapper/vocabulary | Phase exit gates | 10 + 4 | 0 | 0 | Fail-closed mapping/value | PASS |

Disabled requirement tests: 0. Circular expected-value generators: 0. Insufficient requirement assertions: 0. The small contract identity test alone is not sufficient proof of stable lifecycle identity; verification does not rely on it alone and uses store, publication, restart, external, and CLI behavior.

### Anti-Patterns and Disconfirmation

| Check | Result | Severity |
|---|---|---|
| `TBD`/`FIXME`/`XXX` in Phase 7 production/test files | None | None |
| Disabled requirement-linked tests | None | None |
| Public compatibility/registry/factory/profile/migration vocabulary | Exact allowlists and vocabulary checker pass | None |
| Universal NIP-02/bookmark behavior switch | Repository-relative production scan empty | None |
| Rust files over 500 lines | None | None |
| Partial CAP after adversarial trace | None; each CAP has production wiring and behavioral evidence | None |
| Passing-but-misleading test risk | Contract identity test is narrow but supplemented by end-to-end tests; not used alone | Info |
| Uncovered non-goal error path | No isolated test was found for cleanup after a later source-open failure during multi-edit `Publication::recover`; cleanup code exists at `fava-publication/src/lib.rs:150-156`. This is not a Phase 7 required behavior and does not weaken a scored truth. | Info |

### Decision Coverage

No trackable decisions in `07-CONTEXT.md`; gate skipped non-blockingly (`0/0`).

### Human Verification Required

N/A — foundation/library phase with no visual or external-service UX. Every state transition, ordering, cleanup, restart, and composition invariant used for the verdict has an executable behavioral test; `behavior_unverified: 0`.

### Gaps Summary

No blocking or warning gaps. The Phase 7 goal and CAP-01 through CAP-09 are achieved at implementation head `f97ecd8c0f8fd3793860cce95380ddcae9521aa3` and remain verified at documentation head `1dd7e5e7cd6dce0cec90829ac8e116ff19a081a0`.

---

_Verified: 2026-08-21T17:45:03Z_
_Verifier: gsd-verifier_
