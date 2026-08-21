---
phase: 07-semantic-writes-and-capability-composition
plan: 07
subsystem: testing
tags: [rust, semantic-writes, external-falsifier, public-facade, future-kinds]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 04
    provides: exact generation completion guards and bounded semantic failure evidence
provides:
  - independent out-of-workspace semantic capability using fava as its sole normal dependency
  - public live-query rematerialization and retired-completion proof with a scripted transport witness
  - unchanged raw future-kind publication and query evidence without materializer selection
affects: [phase-07-verification, capability-composition, public-facade]
actuals:
  tokens: 16099
  tasks: 2
  commits: 7
tech-stack:
  added: []
  patterns:
    - external capability depends normally only on the public fava facade
    - source successors enter through public live-query relay ingestion
    - scripted transport gates exact publication acknowledgements without sleeps
key-files:
  created:
    - falsifiers/external-semantic-capability/Cargo.toml
    - falsifiers/external-semantic-capability/Cargo.lock
    - falsifiers/external-semantic-capability/src/lib.rs
    - falsifiers/external-semantic-capability/tests/public_capability.rs
    - falsifiers/external-semantic-capability/tests/support/mod.rs
  modified: []
key-decisions:
  - "The unrelated capability uses private deterministic set semantics over non-addressable replaceable kind 15001 and exports only functions plus approved Fava values and contracts."
  - "fava-subscriptions-standard is a dev-only dependency because a genuine outside-consumer source successor must cross Fava.observe(from_relays), not mutate the event cache directly."
  - "Raw custom kind 50001 follows WriteIntent::event unchanged even while an unrelated semantic materializer is selected."
patterns-established:
  - "External N+1 proof: separate workspace, fava-only normal edge, providers under dev-dependencies, no root member or product selection."
  - "Controlled completion proof: hold generation-one relay OK, install source-driven generation two, then release and observe exact currentness."
requirements-completed: [CAP-08, CAP-09]
coverage:
  - id: D1
    description: "An external non-addressable capability implements empty, inverse, preservation, deterministic ordering, duplicate idempotence, malformed input, and private bounds through public Fava contracts."
    requirement: CAP-08
    verification:
      - kind: integration
        ref: "falsifiers/external-semantic-capability/src/lib.rs#three guarded library tests"
        status: pass
      - kind: other
        ref: "cargo metadata: fava is the sole normal dependency"
        status: pass
    human_judgment: false
  - id: D2
    description: "Public Fava preview and live publication preserve one write and receipt across a qualified source successor while a released retired completion is inert and bounded materializer failure preserves current state."
    requirement: CAP-08
    verification:
      - kind: e2e
        ref: "falsifiers/external-semantic-capability/tests/public_capability.rs#external_capability_composes_through_public_fava"
        status: pass
      - kind: e2e
        ref: "falsifiers/external-semantic-capability/tests/public_capability.rs#external_retired_completion_and_failure_preserve_current"
        status: pass
    human_judgment: false
  - id: D3
    description: "Raw arbitrary future kind 50001 publishes and remains query-visible with exact unknown tags and content without a matching materializer or core switch."
    requirement: CAP-09
    verification:
      - kind: e2e
        ref: "falsifiers/external-semantic-capability/tests/public_capability.rs#raw_future_event_kind_publishes_unchanged"
        status: pass
    human_judgment: false
duration: 15min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 07: External Semantic Capability Falsifier Summary

**An independent Fava-only capability now rematerializes through the public facade under one stable receipt, rejects bounded hostile source state, and leaves raw future event kinds untouched.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-08-21T09:58:36Z
- **Completed:** 2026-08-21T10:13:34Z
- **Tasks:** 2
- **Files created:** 5 external-workspace artifacts

## Accomplishments

- Added a separate Cargo workspace whose sole normal dependency is `fava`; every concrete provider, standard implementation, Nostr fixture, and Tokio runtime edge is test-only.
- Implemented a private external materializer for unrelated kind 15001 with bounded opaque edits, inverses, deterministic set composition, duplicate idempotence, unrelated content/tag preservation, and typed malformed/oversized refusal.
- Proved preview/live parity, zero preview custody or effects, public relay-ingested source replacement, stable write/receipt identity, changing `MaterializationId`, retired completion inertness, duplicate-source inertness, and bounded failure preservation.
- Published and queried raw custom kind 50001 unchanged through `WriteIntent::event` while only the unrelated materializer was selected.
- Kept the external capability and kind absent from root members, selected products, vocabulary, and universal owner source/manifests.

## RED and Causal Evidence

- **Task 1 RED:** the external library target failed with 16 unresolved capability helper/materializer references before implementation. Commit: `b378321`.
- **Task 2 RED:** all three named public tests compiled and failed at their explicit RED assertions before the scripted outside-consumer witness existed. Commit: `58bd1c5`.
- **Named deliberate break:** replacing qualified-source decoding with empty-source decoding made `external_capability_composes_through_public_fava` fail on exact preserved content (`alpha` instead of `alpha,omega` plus unrelated source body). Restoring the source application returned the named test green.

## Task Commits

1. **Task 1 RED: Specify external semantic capability** — `b378321` (test)
2. **Task 1 GREEN: Implement external semantic capability** — `934686a` (feat)
3. **Task 2 RED: Add failing external public lifecycle proof** — `58bd1c5` (test)
4. **Task 2 GREEN: Prove external public publication lifecycle** — `25da31d` (test)
5. **Postflight: Remove manifest trailing whitespace** — `87cf4e8` (style)

**Plan metadata:** `ff98c84` plus the final summary correction commit

## Files Created/Modified

- `falsifiers/external-semantic-capability/Cargo.toml` — isolated workspace with one normal facade dependency and explicit test-only assembly dependencies.
- `falsifiers/external-semantic-capability/Cargo.lock` — independent reproducible dependency lock.
- `falsifiers/external-semantic-capability/src/lib.rs` — private bounded codec/materializer plus three pure public-contract tests.
- `falsifiers/external-semantic-capability/tests/public_capability.rs` — three public lifecycle, failure, and raw-future behavioral proofs.
- `falsifiers/external-semantic-capability/tests/support/mod.rs` — private under-500-line scripted transport and public assembly harness.

## Decisions Made

- Capability-owned state uses a private content prefix and sorted set while preserving every unrelated source tag and the remainder of source content verbatim.
- The public integration uses `Fava::observe(Query::from_relays(...))` plus the standard subscription planner so the successor enters the canonical public ingest path.
- Relay `OK`, session close, EOSE, receipt changes, and query watches are the schedule barriers; no sleep establishes correctness.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Public-ingest dependency] Added the standard subscription planner as a dev-only edge**
- **Found during:** Task 2 preflight
- **Issue:** Current public `Fava` exposes canonical relay ingestion only through a live query, which requires a selected subscription planner.
- **Fix:** Added `fava-subscriptions-standard` only under dev-dependencies and drove source v2 through `Fava::observe(Query::from_relays(...))`.
- **Files modified:** `falsifiers/external-semantic-capability/Cargo.toml`, lockfile, public integration test
- **Verification:** Metadata still reports exactly one normal dependency (`fava`); the public rematerialization test passes.
- **Committed in:** `b378321`, `25da31d`

**2. [Rule 3 - Cohesion] Extracted private scripted support below the soft line limit**
- **Found during:** Task 2 implementation
- **Issue:** Keeping transport scheduling and three behavioral cases in one integration file would exceed the repository's 500-line cohesion threshold.
- **Fix:** Extracted `tests/support/mod.rs`; all three Rust files are 381, 244, and 440 lines.
- **Verification:** External all-target tests and strict Clippy pass.
- **Committed in:** `25da31d`

**3. [Rule 1 - Formatting] Removed an extra manifest EOF blank line**
- **Found during:** Final post-summary diff check
- **Issue:** `git diff --check` reported a new blank line at EOF in the isolated manifest.
- **Fix:** Removed the trailing blank line without changing dependencies or behavior.
- **Files modified:** `falsifiers/external-semantic-capability/Cargo.toml`
- **Verification:** The base-to-HEAD diff check and all six external tests pass.
- **Committed in:** `87cf4e8`

---

**Total deviations:** 3 auto-fixed blocking, cohesion, and formatting adjustments. **Impact on plan:** Test-only dependencies, private support structure, and whitespace only; no core, facade, product, or vocabulary change.

## Issues Encountered

None unresolved.

## Verification

- Exact guarded counts: 3 library tests and 3 public integration tests.
- `cargo test --manifest-path falsifiers/external-semantic-capability/Cargo.toml --all-targets` — 6 passed.
- `cargo clippy --manifest-path falsifiers/external-semantic-capability/Cargo.toml --all-targets -- -D warnings` — passed.
- Cargo metadata — exactly one normal dependency (`fava`); 12 explicit dev-dependencies including the public live-query planner.
- Relevant public semantic targets — 4 contract, 6 failure, and 12 publication tests passed.
- `cargo test --workspace --all-targets` — passed, including redb process-kill evidence.
- `python3 tools/check_vocabulary.py` and four vocabulary unit tests — passed.
- Root/product/core absence, exact-base change-amplification, formatting, diff, and line gates — passed.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 07-08 and final Phase 7 verification can consume the external N+1 and raw-future evidence. No blocker.

## Self-Check: PASSED

All five external artifacts and five implementation/evidence commits exist; every guarded, dependency, vocabulary, diff, line, relevant-public, and full-workspace verification passed.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
