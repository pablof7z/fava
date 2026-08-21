---
phase: 07-semantic-writes-and-capability-composition
plan: 09
subsystem: validation
tags: [rust, semantic-writes, deliberate-break, nyquist, cargo, bazel]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 08
    provides: shared public-Fava corpus and four deterministic M7 canaries
provides:
  - causal byte-restored stale-generation and protocol-dependency sensitivity experiments
  - Cargo-resolved exact feature mappings with module-qualified test names
  - complete CAP-01 through CAP-09 validation, security, dependency, vocabulary, Cargo, Bazel, and line evidence
affects: [phase-07-verification, milestone-07-closeout]
actuals:
  tokens: 9907
  tasks: 2
  commits: 6
tech-stack:
  added: []
  patterns:
    - checksum-restored deliberate breaks that require behavioral or intended compile failure
    - feature prose resolved through Cargo metadata and exact test-list discovery
    - fixed-shape seed-redacted CLI evidence produced from a clean revision
key-files:
  created:
    - .planning/phases/07-semantic-writes-and-capability-composition/07-09-SUMMARY.md
  modified:
    - tools/tests/test_semantic_write_feature.py
    - features/semantic-writes.feature
    - docs/issues/0010-m7-semantic-writes-and-capability-composition.md
    - .planning/phases/07-semantic-writes-and-capability-composition/07-VALIDATION.md
    - .planning/phases/07-semantic-writes-and-capability-composition/deferred-items.md
key-decisions:
  - "Feature mappings name the actual Cargo package and integration target, then require exactly one listed test rather than trusting a source path."
  - "The detailed M7 section owns four canaries, including replaceable-edit-inverse, despite the global roster omission."
  - "Raw EventBuilder values retain caller-owned created_at, kind, tags, content, author, and identity; semantic edits retain engine-owned monotonic rematerialization time."
requirements-completed: [CAP-01, CAP-02, CAP-03, CAP-04, CAP-05, CAP-06, CAP-07, CAP-08, CAP-09]
coverage:
  - id: D1
    description: "The exact current-MaterializationId guard detects a retired completion that would otherwise mutate successor state."
    requirement: CAP-06
    verification:
      - kind: integration
        ref: "crates/fava/tests/semantic_write_publication/interleavings.rs#retired_completion_is_attributable_and_inert"
        status: pass
      - kind: other
        ref: "docs/issues/0010-m7-semantic-writes-and-capability-composition.md#current-materialization-identity"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every feature scenario resolves to one real Cargo test target and one exact listed test, including module-qualified names."
    requirement: CAP-04
    verification:
      - kind: unit
        ref: "tools/tests/test_semantic_write_feature.py#SemanticWriteFeatureMappingTests"
        status: pass
    human_judgment: false
  - id: D3
    description: "Protocol crates cannot acquire signer or provider authority and expose only their approved function surfaces."
    requirement: CAP-07
    verification:
      - kind: other
        ref: "docs/issues/0010-m7-semantic-writes-and-capability-composition.md#protocol-dependency-direction"
        status: pass
      - kind: integration
        ref: "crates/fava-nip02/tests/public_api.rs#external_surface_uses_only_approved_functions_and_types"
        status: pass
      - kind: integration
        ref: "crates/fava-bookmarks/tests/public_api.rs#external_surface_uses_only_approved_functions_and_types"
        status: pass
    human_judgment: false
  - id: D4
    description: "All M7 behavior, restart, shared-corpus, four-canary, external, dependency, vocabulary, Cargo, Bazel, boundedness, and ASVS L1 gates pass together."
    requirement: CAP-09
    verification:
      - kind: other
        ref: ".planning/phases/07-semantic-writes-and-capability-composition/07-VALIDATION.md"
        status: pass
    human_judgment: false
duration: 18min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 09: Final M7 Validation Summary

**Two byte-restored sensitivity experiments and one comprehensive gate now prove generation safety, protocol isolation, exact executable mappings, and all CAP-01 through CAP-09 behavior together.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-08-21T14:14:00Z
- **Completed:** 2026-08-21T14:32:00Z
- **Tasks:** 2
- **Files modified:** 6 including this summary

## Accomplishments

- Removed only the authoritative current-`MaterializationId` predicate: first-value remained green, while the exact retired-completion test compiled and failed on accepted generation-one mutation. The source returned byte-identically and the publication target passed 12/12.
- Added module-qualified mapping grammar and Cargo metadata plus exact `--list` resolution. Seven mapper tests now refuse missing, non-test, empty, zero-test, duplicate, and malformed destinations.
- Added a temporary forbidden `fava_signer` import to NIP-02: compilation failed with the intended E0432 undeclared dependency. The source returned byte-identically and NIP-02 passed 7+1.
- Replaced the stale validation draft with exact target counts, four clean CLI evidence paths, dependency and rustdoc allowlists, ASVS L1 dispositions, corrected CAP mappings, and complete Nyquist state.
- Passed root, canary, and external build/check/test/format/strict-Clippy, all 34 Bazel tests, vocabulary, code-line, restored-source, phase-range, and clean-worktree gates.

## RED and Sensitivity Evidence

- Feature mapper RED `9f88c86`: stale source-v2 destination, absent module-qualified grammar, and absent fail-closed target/list validation failed before `7149592` made all seven tests pass.
- `DELIBERATE_BREAK_M7_STALE_COMPLETION`: removing one predicate made only the intended retired-generation assertion fail; SHA-256 restored to `50f73279c139469f03f01247f4e5af692e291f19cc5944fef8e189221d9fb7af`.
- `DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY`: one temporary signer import failed with E0432 `no external crate fava_signer`; SHA-256 restored to `deefde7b77a75f8981c855c6dc46cae008dfeff79d5d527de56bbbda6156c0f2`.

## Task Commits

1. **Task 1: Prove stale-generation guard sensitivity** — `d511994` (test)
2. **Task 2 RED: Add failing exact-mapping guards** — `9f88c86` (test)
3. **Task 2 GREEN: Resolve feature mappings through Cargo** — `7149592` (test)
4. **Task 2: Close authoritative M7 validation** — `362f3e5` (test)
5. **Task 2 postflight: Record clean final canary evidence** — `fc912c1` (docs)

**Plan metadata:** this commit.

## Decisions Made

- The mapping package for the external falsifier is its real Cargo name, `external-semantic-capability-proof`; conceptual directory names cannot substitute for executable package identity.
- The detailed M7 canary section is the focused authority, so all four scenarios run even though the global roster omits the inverse row.
- Caller-selected raw event time and engine-selected semantic rematerialization time remain separate authorities; final evidence asserts both without a hidden clock override.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Verification command] Corrected repository-relative protocol exclusions**

- **Found during:** Task 2 dependency gate.
- **Issue:** The plan's `rg -g '!fava-nip02/**'` and bookmark equivalent did not exclude paths rooted under `crates/`, so the negative scan selected the protocol crates themselves.
- **Fix:** Used `!crates/fava-nip02/**` and `!crates/fava-bookmarks/**`; universal-owner production Rust then returned empty.
- **Files modified:** validation record only.
- **Verification:** Corrected scan, Cargo trees, and both Bazel `somepath` checks pass.
- **Commit:** `362f3e5`.

**2. [Rule 3 - Phase-range hygiene] Removed one historical trailing blank line**

- **Found during:** Final phase-range `git diff --check`.
- **Issue:** Plan 07-03's `deferred-items.md` ended with an added blank line and prevented the required M7 range gate.
- **Fix:** Removed only the blank line; no deferred-item text or behavior changed.
- **Files modified:** `.planning/phases/07-semantic-writes-and-capability-composition/deferred-items.md`.
- **Verification:** Both committed-range and current-range diff checks pass.
- **Commit:** `362f3e5`.

**3. [Rule 3 - Vocabulary gate] Regenerated evidence under a neutral temporary path**

- **Found during:** Post-validation vocabulary check.
- **Issue:** A temporary directory whose basename began with the project name matched the architecture checker's specified-crate grammar inside the validation document.
- **Fix:** Regenerated all four clean bundles under `/tmp/m7-final.dBVHwC` and recorded those exact paths.
- **Files modified:** validation record only.
- **Verification:** Four manifests are clean-revision, seed-redacted, fixed-shape, and bounded; vocabulary passes.
- **Commit:** `fc912c1`.

**Total deviations:** 3 auto-fixed verification/hygiene issues. **Impact:** Stronger fail-closed validation only; no production behavior, public API, architectural vocabulary, dependency, or compatibility change.

## Verification

- Exact guarded counts: 4 contract, 8 memory, 12 publication, 6 failures, 10 redb recovery, 6 process-kill, 7+1 NIP-02, 9+1 bookmarks, 3+3 external, 4 shared corpus, and 4 exact M7 canaries.
- Four clean CLI bundles: seven files each, 3,570-9,383 bytes, exact scenario IDs and artifact hashes, no raw seed.
- Root workspace: build, all-target check/test, format, strict all-target Clippy — PASS.
- Canary and external workspaces: format, all-target check/test, strict all-target Clippy — PASS.
- `bazel test //...` — 34/34 PASS.
- Protocol Cargo metadata/trees, Bazel paths, rustdoc JSON allowlists, public nominal scans, vocabulary, all-file 500/800-line, phase-range, restored-source, and clean-worktree gates — PASS.

## Known Stubs

None.

## Threat Flags

None. This plan adds no network, authentication, storage-schema, or product dependency surface; it validates existing M7 boundaries.

## User Setup Required

None.

## Next Phase Readiness

All nine M7 plans have complete summaries and every documented M7 exit gate has executable evidence. Phase 07 is ready for independent code review, security audit, and goal-backward verification.

## Self-Check: PASSED

All five plan commits exist; all created/modified artifacts exist; the final validation record is complete; no deliberate-break source diff or temporary repository residue remains.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
