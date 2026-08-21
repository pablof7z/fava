---
phase: 07-semantic-writes-and-capability-composition
plan: 06
subsystem: protocol
tags: [rust, nostr, nip-02, nip-51, semantic-writes, replaceable-events, tdd]
requires:
  - phase: 07-semantic-writes-and-capability-composition
    plan: 01
    provides: bounded opaque ReplaceableEventEdit values and replaceable-event materializer contract
  - phase: 07-semantic-writes-and-capability-composition
    plan: 04
    provides: exact source validation and isolated semantic materialization lifecycle
provides:
  - pure NIP-02 follow and unfollow edit construction and selected materializer
  - pure public NIP-51 event and coordinate bookmark editing and selected materializer
  - private bounded codecs that preserve opaque content and unmanaged tags exactly
  - closed protocol vocabulary and negative lifecycle-provider dependency proofs
affects: [07-07, 07-08, selected capability assembly, semantic-write canary]
actuals:
  tokens: 71519
  tasks: 3
  commits: 7
tech-stack:
  added: []
  patterns:
    - protocol helpers return opaque edits and expose selected materializers only through the neutral trait
    - protocol codecs retain the first exact target tag, remove target duplicates, and preserve all unmanaged bytes and order
    - public bookmark edits treat encrypted content as opaque data and never parse or mutate it
key-files:
  created:
    - crates/fava-nip02/src/lib.rs
    - crates/fava-nip02/src/tests.rs
    - crates/fava-bookmarks/src/lib.rs
    - crates/fava-bookmarks/src/tests.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - MODULE.bazel.lock
    - docs/internals/vocabulary.toml
key-decisions:
  - "NIP-02 and public NIP-51 expose functions plus the existing materializer trait object; decoded lists, codecs, targets, and materializer implementations remain private."
  - "A matching existing full tag is retained with relay hint or petname intact, exact duplicates are removed, and a missing target appends one canonical tag without globally sorting unrelated state."
  - "Public bookmarks never interpret encrypted private content; source content is copied exactly for both add and remove edits."
  - "Protocol materializers verify the exact signed source event, actor, kind, successor timestamp, format, and bounds before producing an unsigned successor."
patterns-established:
  - "Private versioned codec: the edit payload owns bounded operation data while all decoded protocol state remains crate-private."
  - "Target-local normalization: normalize only exact target membership and preserve every unmanaged or malformed tag byte-for-byte and in source order."
requirements-completed: [CAP-06, CAP-07, CAP-08]
coverage:
  - id: D1
    description: "NIP-02 follow and unfollow produce bounded kind-3 semantic edits with exact-source refusal, inverses, duplicate handling, and byte-preserving unrelated state."
    requirement: CAP-06
    verification:
      - kind: unit
        ref: "crates/fava-nip02/src/tests.rs#four guarded NIP-02 behavior tests"
        status: pass
      - kind: other
        ref: "cargo test -p fava-nip02 --lib"
        status: pass
    human_judgment: false
  - id: D2
    description: "Public NIP-51 event and coordinate bookmarks produce bounded kind-10003 edits while encrypted content remains opaque."
    requirement: CAP-07
    verification:
      - kind: unit
        ref: "crates/fava-bookmarks/src/tests.rs#four guarded public-bookmark behavior tests"
        status: pass
      - kind: other
        ref: "cargo test -p fava-bookmarks --lib"
        status: pass
    human_judgment: false
  - id: D3
    description: "Both protocol crates keep concrete protocol nouns private and have no dependency path to publication, runtime, transport, store-provider, signer, routing, publisher, delivery, or cache implementations."
    requirement: CAP-08
    verification:
      - kind: other
        ref: "python3 tools/check_vocabulary.py and tools.tests.test_vocabulary_check"
        status: pass
      - kind: other
        ref: "Cargo tree and Bazel somepath negative dependency gates"
        status: pass
    human_judgment: false
duration: 20min
completed: 2026-08-21
status: complete
---

# Phase 07 Plan 06: NIP-02 and Public Bookmark Protocols Summary

**Pure NIP-02 and public NIP-51 helpers now produce bounded semantic edits and selected materializers without exposing decoded protocol state or acquiring lifecycle dependencies.**

## Performance

- **Duration:** 20 min
- **Started:** 2026-08-21T09:58:12Z
- **Completed:** 2026-08-21T10:17:48Z
- **Tasks:** 3
- **Files created/modified:** 13 protocol, test, workspace, build, vocabulary, and summary files

## Accomplishments

- Added `fava-nip02` with public `follow`, `unfollow`, and `materializer` functions over existing public values; all contact-list decoding and materialization machinery is private.
- Added `fava-bookmarks` with event-id and address-coordinate add/remove helpers plus `materializer`; private encrypted bookmark content is never decoded or changed.
- Preserved content and every unmanaged, unknown, or malformed tag exactly and in order, retained one existing full target tag including hints or petname, removed exact target duplicates, and appended only a missing canonical target.
- Enforced exact actor, kind, signature, event identity, successor timestamp, edit format, input bounds, and output bounds with existing typed refusals.
- Registered only the already-approved crate ownership under existing `ContactList` and `BookmarkList` vocabulary concepts and proved the protocol crates cannot reach lifecycle/provider implementations.

## RED and Causal Evidence

- **Task 1 RED:** the four required NIP-02 tests failed to compile because `follow`, `unfollow`, and `materializer` did not exist. Commit: `84b2957`.
- **Task 1 deliberate break:** replacing retention of the existing full target tag made `follow_preserves_unrelated_state_and_orders_deterministically` fail on relay-hint and petname preservation. Restoring target-local retention returned the suite green.
- **Task 2 RED:** the four required public-bookmark tests failed to compile because event/coordinate add/remove helpers and `materializer` did not exist. Commit: `a259c24`.
- **Task 2 deliberate break:** replacing opaque source content with empty content made `bookmark_preserves_unrelated_state_and_orders_deterministically` fail. Restoring exact content copying returned the suite green.

## Task Commits

1. **Task 1 RED: Specify NIP-02 semantic edits** — `84b2957` (test)
2. **Task 1 GREEN: Implement NIP-02 semantic edits** — `5bc70a6` (feat)
3. **Task 2 RED: Specify public bookmark edits** — `a259c24` (test)
4. **Task 2 GREEN: Implement public bookmark edits** — `7736a65` (feat)
5. **Task 3: Close protocol vocabulary and dependencies** — `64bc9ad` (test)
6. **Build metadata: Refresh Bazel dependency lock** — `1ad9030` (chore)

**Plan metadata:** this commit

## Files Created/Modified

- `crates/fava-nip02/src/lib.rs` and `src/tests.rs` — private bounded kind-3 codec/materializer and five unit tests, including the public-surface guard.
- `crates/fava-bookmarks/src/lib.rs` and `src/tests.rs` — private bounded kind-10003 public-bookmark codec/materializer and five unit tests, including opaque-content and public-surface guards.
- Both protocol `Cargo.toml` and `BUILD.bazel` files — libraries and local unit-test targets with neutral/domain normal dependencies only.
- `Cargo.toml` and `Cargo.lock` — workspace membership and locked local package metadata.
- `MODULE.bazel.lock` — generated Bazel crate-universe refresh caused by adding the two Cargo workspace members.
- `docs/internals/vocabulary.toml` — assigned approved `fava-nip02` and `fava-bookmarks` crate ownership to the existing Nostr concepts without registering symbols.

## Decisions Made

- The only public capability selection point is `materializer() -> Arc<dyn ReplaceableEventMaterializer>`; no protocol descriptor, registry, factory, target enum, decoded list, or error noun was added.
- Duplicate normalization is target-local. One original matching tag survives intact and in place, further matches disappear, and unrelated ordering never changes.
- NIP-51 address bookmarks accept the existing `EventCoordinate` only when it denotes a replaceable/addressable event and encode its canonical coordinate into an `a` tag.
- The codec format is private and versioned; wrong format and size are rejected before decoding untrusted edit data.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Required generated build metadata] Refreshed the Bazel module lock**
- **Found during:** Bazel verification after adding both workspace crates
- **Issue:** Bazel's Cargo extension updated its generated crate-universe inputs because the root Cargo workspace gained `fava-nip02` and `fava-bookmarks`.
- **Fix:** Retained and committed the generated `MODULE.bazel.lock` refresh after explicit orchestrator approval.
- **Files modified:** `MODULE.bazel.lock`
- **Verification:** Both local Bazel unit targets and `bazel test //...` passed.
- **Committed in:** `1ad9030`

---

**Total deviations:** 1 auto-fixed blocking build-metadata issue. **Impact on plan:** Required metadata only; no dependency, public API, protocol behavior, or vocabulary expansion.

## Issues Encountered

- The direct `unittest` file-path spelling is not importable as a Python module; the plan's canonical `python3 -m unittest tools.tests.test_vocabulary_check` invocation passed all four tests.
- No unresolved issue remains.

## Verification

- Exact `--list` guards found all four required NIP-02 tests and all four required bookmark tests; each crate's full five-test library suite passed.
- Both named deliberate breaks failed their intended preservation test before restoration.
- `cargo check --workspace --all-targets` and `cargo test --workspace --all-targets` — passed.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check` — passed.
- `python3 tools/check_vocabulary.py` and `python3 -m unittest tools.tests.test_vocabulary_check` — passed (4 tests).
- Cargo tree forbidden lifecycle/provider scans and Bazel `somepath` queries — empty.
- `bazel test //crates/fava-nip02:unit_tests //crates/fava-bookmarks:unit_tests` — 2/2 passed.
- `bazel test //...` — 30/30 tests passed.
- Public nominal, stub/skipped-test, 800-line global, 500-line touched-file, and `git diff --check` gates — passed.

## Known Stubs

None.

## User Setup Required

None.

## Next Phase Readiness

- Both protocol capabilities are ready for Plan 07-07 selected-capability assembly and Plan 07-08 public conformance/canary proof.
- No blockers remain.

## Self-Check: PASSED

- All declared protocol, test, Cargo, Bazel, lock, and vocabulary files exist.
- All six task/build commits are present on `worktree-agent-m7-p06`.

---
*Phase: 07-semantic-writes-and-capability-composition*
*Completed: 2026-08-21*
