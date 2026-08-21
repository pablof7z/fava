---
phase: 07-semantic-writes-and-capability-composition
reviewed: 2026-08-21T15:10:16Z
depth: standard
files_reviewed: 86
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - MODULE.bazel.lock
  - apps/canary/Cargo.lock
  - apps/canary/Cargo.toml
  - apps/canary/README.md
  - apps/canary/scenarios.json
  - apps/canary/src/lib.rs
  - apps/canary/src/lib_tests.rs
  - apps/canary/src/main.rs
  - apps/canary/src/semantic_failure.rs
  - apps/canary/src/semantic_n_plus_one.rs
  - apps/canary/src/semantic_process.rs
  - apps/canary/src/semantic_write_store.rs
  - apps/canary/src/semantic_write_support.rs
  - apps/canary/src/semantic_writes.rs
  - apps/canary/src/semantic_writes_tests.rs
  - crates/fava-bookmarks/BUILD.bazel
  - crates/fava-bookmarks/Cargo.toml
  - crates/fava-bookmarks/src/bounds.rs
  - crates/fava-bookmarks/src/lib.rs
  - crates/fava-bookmarks/src/tests.rs
  - crates/fava-bookmarks/tests/public_api.rs
  - crates/fava-nip02/BUILD.bazel
  - crates/fava-nip02/Cargo.toml
  - crates/fava-nip02/src/bounds.rs
  - crates/fava-nip02/src/lib.rs
  - crates/fava-nip02/src/tests.rs
  - crates/fava-nip02/tests/public_api.rs
  - crates/fava-publication/BUILD.bazel
  - crates/fava-publication/Cargo.toml
  - crates/fava-publication/src/delivery.rs
  - crates/fava-publication/src/lib.rs
  - crates/fava-publication/src/materialization.rs
  - crates/fava-publication/src/run.rs
  - crates/fava-publisher/src/lib.rs
  - crates/fava-query-standard/tests/source_merge.rs
  - crates/fava-write-store-memory/src/lib.rs
  - crates/fava-write-store-memory/src/lifecycle.rs
  - crates/fava-write-store-memory/src/semantic.rs
  - crates/fava-write-store-redb/BUILD.bazel
  - crates/fava-write-store-redb/Cargo.toml
  - crates/fava-write-store-redb/src/lib.rs
  - crates/fava-write-store-redb/src/lifecycle.rs
  - crates/fava-write-store-redb/src/ops.rs
  - crates/fava-write-store-redb/src/schema.rs
  - crates/fava-write-store-redb/src/semantic.rs
  - crates/fava-write-store-redb/src/validation.rs
  - crates/fava-write-store-redb/tests/process_kill.rs
  - crates/fava-write-store-redb/tests/process_kill/semantic.rs
  - crates/fava-write-store-redb/tests/semantic_write_store.rs
  - crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs
  - crates/fava-write-store/src/lib.rs
  - crates/fava-write/BUILD.bazel
  - crates/fava-write/src/builder.rs
  - crates/fava-write/src/edit.rs
  - crates/fava-write/src/lib.rs
  - crates/fava-write/src/materialization.rs
  - crates/fava-write/tests/event_builder.rs
  - crates/fava/BUILD.bazel
  - crates/fava/Cargo.toml
  - crates/fava/src/lib.rs
  - crates/fava/tests/automatic_publication.rs
  - crates/fava/tests/semantic_write_capabilities.rs
  - crates/fava/tests/semantic_write_contract.rs
  - crates/fava/tests/semantic_write_failures.rs
  - crates/fava/tests/semantic_write_failures/support.rs
  - crates/fava/tests/semantic_write_publication.rs
  - crates/fava/tests/semantic_write_publication/interleavings.rs
  - crates/fava/tests/semantic_write_store.rs
  - crates/fava/tests/support/semantic_write.rs
  - crates/fava/tests/support/semantic_write_capability_lifecycle.rs
  - crates/fava/tests/support/semantic_write_capability_protocol.rs
  - crates/fava/tests/write_bounds.rs
  - docs/internals/vocabulary.toml
  - docs/issues/0010-m7-semantic-writes-and-capability-composition.md
  - docs/issues/0012-fava-write-bounds-bazel-edge.md
  - falsifiers/external-semantic-capability/Cargo.lock
  - falsifiers/external-semantic-capability/Cargo.toml
  - falsifiers/external-semantic-capability/src/capability.rs
  - falsifiers/external-semantic-capability/src/lib.rs
  - falsifiers/external-semantic-capability/tests/public_capability.rs
  - falsifiers/external-semantic-capability/tests/support/mod.rs
  - falsifiers/external-semantic-capability/tests/support/waits.rs
  - features/semantic-writes.feature
  - tools/tests/test_semantic_write_feature.py
findings:
  critical: 7
  warning: 2
  info: 0
  total: 9
status: issues_found
---

# Phase 07: Code Review Report

**Reviewed:** 2026-08-21T15:10:16Z
**Depth:** standard
**Files Reviewed:** 86
**Status:** issues_found

## Summary

The phase is not shippable. Seven correctness or isolation defects break exact-generation, bounded-admission, source-failure, raw-facade, or canary guarantees. Two evidence defects let required checks pass without proving the claimed behavior. The focused memory-store, publication, canary process, vocabulary, and feature-mapping suites pass, demonstrating that current tests do not discriminate these branches.

All nine Phase 07 summaries were cross-checked against the commit-range file list; the scope above contains every existing non-planning source/config artifact changed in `6fe21f745297b4af414e52269c3ae1c813cbf28f..HEAD`.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-01 [BLOCKER]: Memory idempotence bypasses exact current-generation validation

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava-write-store-memory/src/semantic.rs:179-192`

**Issue:** `install_materialization` returns success for an identical event/source before calling `require_current`. A stale `expected` materialization/source, or a terminal receipt that still retains semantic custody, can therefore be accepted as an idempotent success. The redb provider correctly performs the exact-current guard first at `crates/fava-write-store-redb/src/semantic.rs:128-140`, so the two selected providers have observably different CAS semantics. The redb-only regression at `crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs:19-69` proves the intended order; the memory corpus has no equivalent.

**Fix:** Move `require_current(...)` before the memory idempotent-return branch. Add a memory-provider parity test that submits the identical body/source with a stale materialization ID, stale source ID, and terminal receipt, requires refusal/no notification, then proves the exact current replay remains idempotent.

### CR-02 [BLOCKER]: Universal publication accepts a materializer that ignores the injected timestamp

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava-publication/src/materialization.rs:175-205`

**Issue:** Publication computes the owner-controlled `created_at` and supplies it to the materializer, but `validate_materialization` checks only ordinary event validity, ID, actor, and coordinate (`:390-407`). A third-party materializer can return any valid timestamp—including stale or far-future time—and the first generation is accepted and signed. Successor monotonicity in the stores is not an exact equality check and does not protect the first generation. This violates the exact injected-time contract at the universal boundary.

**Fix:** Pass the injected timestamp into `validate_materialization` and refuse unless `event.created_at == injected_created_at`, before routing or store mutation. Add malicious external-materializer tests for both first value and successor that return a valid event with the wrong timestamp and assert zero custody/effects or preserved current state respectively.

### CR-03 [BLOCKER]: Capacity exhaustion invokes provider code before admission

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava-publication/src/lib.rs:87-107`

**Issue:** Semantic acceptance opens sources and invokes the selected third-party materializer before the write store performs its atomic capacity admission. At full active capacity, an ultimately refused request can still execute arbitrary provider code, panic handling, source work, and routing. The test at `crates/fava/tests/semantic_write_publication.rs:142-167` explicitly requires one materializer call in this exhausted case, so it enshrines rather than detects the failure. Phase ownership requires an admission permit before that work.

**Fix:** Add a store-owned reservation/permit operation derived from the provider's active capacity, acquire it before `prepare_semantic`, and consume or release it atomically with acceptance/refusal. Change the capacity test to require zero materializer, signer, router, publisher, transport, custody, task, and notification effects when no permit is available.

### CR-04 [BLOCKER]: Closing either semantic source silently disables the surviving source

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava-publication/src/materialization.rs:36-59`

**Issue:** `OpenedSemanticSources::next_change` returns `false` when either the cache or write-store change stream fails/closes. The runner then removes all semantic state and closes both streams at `crates/fava-publication/src/run.rs:84-91`. The receipt remains live with no failure evidence, but later qualified changes from the still-valid source can never rematerialize it. This violates source failure isolation: one source failure erases the other source's ongoing contribution.

**Fix:** Track cache and write-store stream liveness independently, retain each source's last valid snapshot, and continue selecting from/observing the surviving source. Persist bounded, source-attributed failure/shortfall evidence and stop semantic observation only when both sources are unavailable or the receipt ends. Add tests closing each source independently and then changing the other source.

### CR-05 [BLOCKER]: Store read failures are treated as receipt deletion and permanently end the runner

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava-publication/src/run.rs:52-69`

**Issue:** The main loop converts `store.receipt(...)` with `.ok().flatten()`, conflating a provider error with `Ok(None)`. Any transient redb/read failure therefore exits the runner, closes routing/signing/semantic ownership, removes its cancellation entry, and leaves the durable non-terminal receipt stranded until an external restart. The same collapse occurs during initialization at `:145-159`, and destination lanes independently treat read failure as disappearance at `crates/fava-publication/src/delivery.rs:100-109`.

**Fix:** Match `Ok(Some(_))`, `Ok(None)`, and `Err(_)` explicitly. Only `Ok(None)` may end ownership. Keep the owned runner alive across a bounded retry/reopen policy for store errors and surface attributable diagnostics/failure evidence where the store can accept it. Add an injected transient-read-error store that fails once, then proves the same receipt resumes signing/routing/delivery/rematerialization without restart.

### CR-06 [BLOCKER]: The public raw builder cannot be used through the public Fava dependency alone

**File:** `/Users/pablofernandez/Work/nnn-m7/crates/fava/src/lib.rs:30-34`

**Issue:** The facade re-exports `EventBuilder`, but not the `Tag` type required by `EventBuilder::from_parts`, `.tags`, and `.tag`, nor `EventBuildError` returned by `.build()` (`crates/fava-write/src/builder.rs:28-34,58-76`). An application depending only on `fava` cannot construct validated arbitrary tags or name/match builder refusal without adding an implementation-layer `nostr` or `fava-write` dependency. The alleged outside-consumer proof masks the missing facade because it imports `nostr::event::Tag` from an explicit dev-dependency at `falsifiers/external-semantic-capability/tests/public_capability.rs:8-16,265-285`.

**Fix:** Re-export the existing `Tag` and `EventBuildError` symbols from `fava` (no new vocabulary), then add a compile/run consumer whose only normal dependency is `fava` and which builds arbitrary ordered tags, exact `created_at`, arbitrary kind/content, and pattern-matches an oversized-builder refusal through that facade.

### CR-07 [BLOCKER]: Successful canary owners can leave detached descendants running

**File:** `/Users/pablofernandez/Work/nnn-m7/apps/canary/src/semantic_process.rs:48-64`

**Issue:** When the process-group owner exits successfully and both pipe readers reach EOF before the deadline, `run_owned` returns immediately without checking or terminating the rest of the process group. A child that redirects stdin/stdout/stderr and keeps running escapes cleanup. Existing tests cover only a descendant that retains inherited pipes (`:181-193`), while `semantic_n_plus_one` reports `owned_children_reaped: true` based solely on `owner_reaped` (`apps/canary/src/semantic_n_plus_one.rs:25-45,111`). The canary can thus claim bounded process cleanup while an external proof process continues mutating files or consuming resources.

**Fix:** On every owner exit, verify the process group is empty; terminate remaining group members within `CLEANUP_CAPACITY` before returning success. Rename evidence to distinguish owner reaping from group termination. Add a test spawning `sleep 30` with all standard streams redirected, let the shell exit zero, then require the descendant to be absent/zombie and group-clean evidence to be true.

## Warnings

### WR-01 [WARNING]: The rematerialization canary is vacuous for two claimed behaviors

**File:** `/Users/pablofernandez/Work/nnn-m7/apps/canary/src/semantic_writes.rs:127-219`

**Issue:** Source v1 already contains Bob, then the accepted edit is `follow(Bob)`, so both source v1 and source v2 (`Bob + Carol`) satisfy the final Bob/Carol assertion even if the edit is ignored. Separately, generation one is retired before its signer completes and never reaches a route/publication/delivery attempt; nevertheless the evidence records `retired_stale_effects: 0`. The canary therefore does not prove that the edit survives an unrelated source change or that a stale delivery completion is inert.

**Fix:** Start with a source that lacks Bob, update to a source that adds unrelated Carol but still lacks Bob, and require exactly one Bob plus one Carol after rematerialization. Also hold an actual generation-one delivery completion, install generation two, release the old completion, and assert generation-two receipt/event/route/delivery evidence remains byte-for-byte current.

### WR-02 [WARNING]: Feature-to-test mapping can silently change dependency resolution

**File:** `/Users/pablofernandez/Work/nnn-m7/tools/tests/test_semantic_write_feature.py:85-134`

**Issue:** The mapping verifier runs both `cargo metadata` and `cargo test` without `--locked`. A stale or missing root/standalone lock can be rewritten during what is presented as deterministic mapping evidence, allowing the mapping gate to pass against an uncommitted dependency graph and making an ostensibly read-only validation mutate the checkout.

**Fix:** Add `--locked` to both Cargo invocations, assert the relevant lockfile exists, and add a negative fixture proving an out-of-date lock fails rather than being updated.

---

_Reviewed: 2026-08-21T15:10:16Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_
