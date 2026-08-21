---
phase: 07-semantic-writes-and-capability-composition
plan: 03
subsystem: publication
tags: [rust, semantic-writes, publication, materialization, routing, preview, tdd]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 01
    provides: bounded edit and materializer contracts with exact materialization identity
  - phase: 07-semantic-writes-and-capability-composition
    plan: 02
    provides: atomic semantic custody, generation compare-and-set, and recovery facts
provides:
  - public Fava semantic materializer selection and first-value publication
  - one exact source observation inside each bounded live semantic receipt runner
  - strict newer-or-empty successor materialization with exact self exclusion
  - effect-free semantic write-route preview through the live derivation helper
affects: [07-04, 07-05, 07-06, publication, routing, recovery, public-facade]
actuals:
  tokens: 18886
  tasks: 2
  commits: 8
tech-stack:
  added: []
  patterns:
    - exact kind-indexed materializer selection capped before custody
    - canonical cache plus write-store winner selection with exact local-generation exclusion
    - source observation embedded in the existing bounded receipt runner
    - one materialize-validate-route derivation shared by preview and live custody
key-files:
  created:
    - crates/fava-publication/src/materialization.rs
    - crates/fava/tests/semantic_write_publication.rs
    - crates/fava/tests/support/semantic_write.rs
  modified:
    - crates/fava-publication/src/lib.rs
    - crates/fava-publication/src/run.rs
    - crates/fava/src/lib.rs
    - crates/fava-write/src/materialization.rs
    - crates/fava/BUILD.bazel
key-decisions:
  - "ReplaceableEventMaterializer identifies its exact non-addressable replaceable Kind while supports validates the edit format; no format noun or registry was added."
  - "The existing receipt runner owns semantic source observation, so runner count remains bounded by write-store active capacity and no semantic queue exists."
  - "A source successor must be strictly newer than the consumed source; removal becomes None instead of falling back to an older retained event."
  - "Preview opens and immediately closes the same bounded local source snapshots, invokes the same pure derivation, and never accepts custody or starts live work."
patterns-established:
  - "Exact semantic selection: author plus kind query, current ReceiptId and MaterializationId exclusion, then the standard evaluator winner."
  - "Generation transition: checked injected timestamp, materializer, public event validation, route derivation, store CAS, then generation-scoped signing."
requirements-completed: [CAP-01, CAP-02, CAP-03, CAP-04, CAP-05]
coverage:
  - id: D1
    description: "A supported first-value edit crosses public Fava, becomes an observable local event, signs, routes, publishes, and settles under one receipt."
    requirement: CAP-01
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#first_value_edit_publishes_through_public_fava"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#first_value_receives_exact_injected_timestamp"
        status: pass
    human_judgment: false
  - id: D2
    description: "Selection bounds, duplicates, unsupported edits, and exhausted admission refuse before custody or publication effects."
    requirement: CAP-02
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#materializer_selection_bounds_refuse_before_custody"
        status: pass
    human_judgment: false
  - id: D3
    description: "Only a strictly newer qualified independent source advances the generation; self, equal, older, duplicate, wrong-actor, and wrong-kind facts are inert."
    requirement: CAP-03
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#newer_source_rematerializes_once_and_preserves_unrelated_fields"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#own_local_materialization_does_not_create_a_second_generation"
        status: pass
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#equal_older_unqualified_and_duplicate_sources_are_inert"
        status: pass
    human_judgment: false
  - id: D4
    description: "Removing the consumed source produces one empty-source successor and never selects an older fallback."
    requirement: CAP-04
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#source_removal_selects_next_or_empty_once"
        status: pass
    human_judgment: false
  - id: D5
    description: "Semantic preview matches the unchanged-source live route while leaving custody, receipt notifications, signing, router acquisition, publishing, and transport untouched."
    requirement: CAP-05
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication.rs#semantic_preview_matches_initial_route_with_zero_effects"
        status: pass
    human_judgment: false
duration: 32min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 03: Semantic Publication and Preview Summary

**Public Fava now publishes first semantic materializations, tracks exact live source successors under one bounded receipt runner, and previews the same route without custody or effects.**

## Performance

- **Duration:** 32 min
- **Started:** 2026-08-21T08:47:47Z
- **Completed:** 2026-08-21T09:19:21Z
- **Tasks:** 2
- **Files created/modified:** 14 implementation, test, build, and lock files

## Accomplishments

- Extended the approved materializer contract with exact `kind()` selection, capped selection at 64, refused duplicates and unsupported formats, and preserved `supports(&edit)` as format validation.
- Routed first-value and source-backed edits through public `Fava`, ordinary write-store custody, signing, routing, publishing, receipt observation, and stable `MaterializationId` evidence.
- Kept cache and write-store source watches open across acceptance inside the existing receipt runner, excluding only the exact current receipt and materialization before canonical winner collapse.
- Applied strict source succession: newer independent source or `None`; equal, older, duplicate, unqualified, self, and older fallback observations do nothing.
- Shared exact selection, checked timestamp injection, materialization, validation, and route derivation between live custody and semantic preview; preview commits no store or receipt fact and opens no live router, signer, publisher, or transport work.
- Re-exported the existing event, coordinate, edit, materialization, and write values needed by external public-facade consumers without adding nominal vocabulary.

## RED and Causal Evidence

- **Task 1 RED:** `cargo test -p fava --test semantic_write_publication` failed at the public facade because the required event, coordinate, edit, materializer, timestamp, and intent-error values were not re-exported. Commit: `89a64b1`.
- **Task 2 RED:** the eight-test target compiled; `newer_source_rematerializes_once_and_preserves_unrelated_fields` and `source_removal_selects_next_or_empty_once` timed out without generation 2, while `semantic_preview_matches_initial_route_with_zero_effects` received the prior typed refusal. Commit: `841d0c9`.
- **Strict-order deliberate break:** changing the successor comparison from `>` to `>=` made `equal_older_unqualified_and_duplicate_sources_are_inert` observe `MaterializationId(2)` instead of `MaterializationId(1)`.
- **Preview deliberate break:** disabling facade delegation made `semantic_preview_matches_initial_route_with_zero_effects` fail with the named deliberate refusal. Both breaks were reverted before GREEN verification.

## Task Commits

1. **Task 1 RED: Specify public semantic publication tracer** — `89a64b1` (test)
2. **Task 1 GREEN: Publish first semantic materialization** — `0f5e33f` (feat)
3. **Task 2 RED: Specify live rematerialization and preview** — `841d0c9` (test)
4. **Task 2 GREEN: Rematerialize live semantic writes** — `f2995aa` (feat)
5. **Blocking build fix: Declare semantic publication Bazel edge** — `2a5187e` (fix)
6. **Blocking build fix: Refresh dependency locks** — `58dcb7a` (chore)
7. **Recovery fix: Retain the semantic runner** — `c207294` (fix)

**Plan metadata:** this commit

## Files Created/Modified

- `crates/fava-write/src/materialization.rs` — materializer exact-kind selection method.
- `crates/fava-publication/src/materialization.rs` — bounded selection, exact sources, shared derivation, strict successor qualification, and preview support.
- `crates/fava-publication/src/lib.rs` — semantic acceptance, recovery, preview, provider wiring, and source lifecycle.
- `crates/fava-publication/src/run.rs` — source changes inside the receipt runner, generation-scoped signing, and recovered initial reconciliation.
- `crates/fava/src/lib.rs` — public value re-exports, builder selection, and semantic preview delegation.
- `crates/fava/tests/semantic_write_publication.rs` and `crates/fava/tests/support/semantic_write.rs` — eight public tracer, bound, successor, inertness, removal, and zero-effect preview behaviors.
- Cargo/Bazel manifests and lockfiles — explicit `fava-query` publication edge plus the semantic publication Bazel test graph.

## Decisions Made

- Materializers are indexed by the already-approved `Kind`; `supports` remains responsible for edit-format ownership. A registry, descriptor, factory, profile, or compatibility layer was unnecessary.
- Source ownership remains in existing `QuerySource` watches, and durable admission remains atomic in `WriteStore`; a preflight read avoids invoking materialization when capacity is already exhausted without replacing store authority.
- Source removal retains the prior timestamp floor and materializes from `None`; an older retained event cannot become a fallback generation.
- Late signer completion records refusal only if the exact unsigned body is still current, and each replacement cancels the prior generation's signer before starting the next.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking contract gap] Added exact materializer kind selection and publication query edges**
- **Found during:** Task 1 preflight
- **Issue:** The approved materializer contract had no exact kind selector, and `fava-publication` lacked its required `fava-query` Cargo/Bazel edges.
- **Fix:** Added `kind() -> Kind`, kept `supports(&edit)` for format checks, wired `fava-query`, and refreshed root, canary, and Bazel lock inputs.
- **Files modified:** `crates/fava-write/src/materialization.rs`, `crates/fava-publication/Cargo.toml`, `crates/fava-publication/BUILD.bazel`, `Cargo.lock`, `apps/canary/Cargo.lock`, `MODULE.bazel.lock`
- **Commits:** `0f5e33f`, `58dcb7a`

**2. [Rule 3 - Blocking build metadata] Declared the semantic test support's direct Bazel dependency**
- **Found during:** Final Bazel verification
- **Issue:** Cargo compiled the new shared test support through normal dependencies, while Bazel required its direct `fava-routing` edge.
- **Fix:** Added only `//crates/fava-routing:lib` to `//crates/fava:semantic_write_publication`.
- **Files modified:** `crates/fava/BUILD.bazel`
- **Commit:** `2a5187e`

**3. [Rule 1 - Lifecycle bug] Kept recovered semantics alive through initial reconciliation and router-open refusal**
- **Found during:** Task 2 lifecycle review
- **Issue:** A recovered receipt waited for a later source notification before reconciling its initial snapshots, and an automatic router-open refusal could end the semantic source owner while custody remained live.
- **Fix:** Reconciled recovered initial facts before signing and allowed the semantic runner to remain live without an automatic router session.
- **Files modified:** `crates/fava-publication/src/run.rs`
- **Commit:** `c207294`

---

**Total deviations:** 3 auto-fixed blocking/correctness issues. **Impact on plan:** Required exact selection, reproducible builds, and live lifecycle correctness; no new vocabulary or compatibility path.

## Deferred Issues

- Full `bazel test //...` exposes a pre-existing unrelated `//crates/fava:write_bounds` metadata defect: its test imports `fava_routing` without declaring `//crates/fava-routing:lib`. `git show c58bf22:crates/fava/BUILD.bazel` proves the omission predates Plan 07-03. Recorded in `deferred-items.md`; the Plan 07-03 Bazel target passes.

## Verification

- Both guarded plan groups — exact required counts 3 and 5; full target 8 passed.
- `cargo test -p fava --test semantic_write_contract` — 4 passed.
- `cargo test -p fava-publication` — passed.
- `cargo check --workspace` — passed.
- `cargo test --workspace --all-targets` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo test --manifest-path apps/canary/Cargo.toml --all-targets` — 7 passed.
- `cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings` — passed.
- `bazel test //crates/fava:semantic_write_publication` — passed.
- `bazel test //...` — Plan 07-03 target passed; overall command stopped at the verified pre-existing `//crates/fava:write_bounds` missing direct dependency.
- `python3 tools/check_vocabulary.py` — passed.
- `python3 -m unittest tools.tests.test_vocabulary_check` — 4 passed.
- `cargo fmt --all -- --check`, `git diff --check`, hard/soft line guards, and forbidden public noun scan — passed.
- External falsifier manifests referenced by later milestone validation do not yet exist in this checkout; no falsifier command was available for Plan 07-03.

## Known Stubs

None.

## Next Phase Readiness

- Plan 07-04 can add exact generation propagation across signer, router, publisher, and delivery completions using the now-live successor path.
- Plan 07-05 can implement durable redb semantic custody and recover the same public publication behavior without changing the facade.

## Self-Check: PASSED

All created artifacts and seven implementation/evidence commits exist, all Plan 07-03 owned tests and Bazel targets pass, `status: complete` is present, and the unrelated full-Bazel defect is recorded with pre-plan proof.
