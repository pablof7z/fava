---
phase: 07-semantic-writes-and-capability-composition
reviewed: 2026-08-21T16:50:24Z
depth: deep
files_reviewed: 80
files_reviewed_list:
  - apps/canary/README.md
  - apps/canary/scenarios.json
  - apps/canary/src/main.rs
  - apps/canary/src/semantic_delivery_support.rs
  - apps/canary/src/semantic_n_plus_one.rs
  - apps/canary/src/semantic_process.rs
  - apps/canary/src/semantic_write_store.rs
  - apps/canary/src/semantic_write_support.rs
  - apps/canary/src/semantic_writes.rs
  - apps/canary/src/semantic_writes_tests.rs
  - crates/fava-bookmarks/src/lib.rs
  - crates/fava-bookmarks/src/tests.rs
  - crates/fava-bookmarks/tests/public_api.rs
  - crates/fava-nip02/BUILD.bazel
  - crates/fava-nip02/Cargo.toml
  - crates/fava-nip02/src/lib.rs
  - crates/fava-nip02/src/tests.rs
  - crates/fava-nip02/tests/public_api.rs
  - crates/fava-publication/src/delivery.rs
  - crates/fava-publication/src/lib.rs
  - crates/fava-publication/src/materialization.rs
  - crates/fava-publication/src/run.rs
  - crates/fava-write-store-memory/src/lib.rs
  - crates/fava-write-store-memory/src/lifecycle.rs
  - crates/fava-write-store-memory/src/semantic.rs
  - crates/fava-write-store-memory/src/state.rs
  - crates/fava-write-store-redb/src/lib.rs
  - crates/fava-write-store-redb/src/ops.rs
  - crates/fava-write-store-redb/src/schema.rs
  - crates/fava-write-store-redb/src/semantic.rs
  - crates/fava-write-store-redb/src/validation.rs
  - crates/fava-write-store-redb/tests/process_kill/semantic.rs
  - crates/fava-write-store-redb/tests/semantic_write_store.rs
  - crates/fava-write-store-redb/tests/semantic_write_store/recovery.rs
  - crates/fava-write-store/src/lib.rs
  - crates/fava-write-store/src/receipt.rs
  - crates/fava-write/BUILD.bazel
  - crates/fava-write/src/edit.rs
  - crates/fava-write/src/lib.rs
  - crates/fava-write/src/materialization.rs
  - crates/fava-write/tests/replaceable_edit.rs
  - crates/fava/BUILD.bazel
  - crates/fava/src/lib.rs
  - crates/fava/tests/automatic_publication.rs
  - crates/fava/tests/semantic_write_capabilities.rs
  - crates/fava/tests/semantic_write_contract.rs
  - crates/fava/tests/semantic_write_failures.rs
  - crates/fava/tests/semantic_write_failures/faults.rs
  - crates/fava/tests/semantic_write_failures/reservation.rs
  - crates/fava/tests/semantic_write_failures/source_isolation.rs
  - crates/fava/tests/semantic_write_failures/support.rs
  - crates/fava/tests/semantic_write_failures/transient_reads.rs
  - crates/fava/tests/semantic_write_failures/validation.rs
  - crates/fava/tests/semantic_write_publication.rs
  - crates/fava/tests/semantic_write_publication/author.rs
  - crates/fava/tests/semantic_write_publication/interleavings.rs
  - crates/fava/tests/semantic_write_publication/shared_capacity.rs
  - crates/fava/tests/semantic_write_store.rs
  - crates/fava/tests/semantic_write_store/author.rs
  - crates/fava/tests/semantic_write_store/current_guard.rs
  - crates/fava/tests/support/semantic_write.rs
  - crates/fava/tests/support/semantic_write_capability_lifecycle.rs
  - crates/fava/tests/support/semantic_write_capability_protocol.rs
  - crates/fava/tests/support/semantic_write_capability_signer.rs
  - docs/internals/vocabulary.toml
  - docs/issues/0013-edit-author-at-taker.md
  - docs/issues/0014-publish-door-ergonomics.md
  - docs/issues/0015-publish-scope-vocabulary.md
  - docs/issues/0016-runtime-handle-at-assembly.md
  - docs/issues/0017-routers-required-at-assembly.md
  - docs/spec/ARCHITECTURE.md
  - docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md
  - docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md
  - falsifiers/external-semantic-capability/src/bin/public_event_builder.rs
  - falsifiers/external-semantic-capability/src/capability.rs
  - falsifiers/external-semantic-capability/src/lib.rs
  - falsifiers/external-semantic-capability/tests/public_capability.rs
  - falsifiers/external-semantic-capability/tests/support/mod.rs
  - features/semantic-writes.feature
  - tools/tests/test_semantic_write_feature.py
findings:
  critical: 4
  warning: 0
  info: 0
  total: 4
status: issues_found
---

# Phase 07: Code Review Report

**Reviewed:** 2026-08-21T16:50:24Z
**Depth:** deep
**Files Reviewed:** 80
**Status:** issues_found

## Summary

Re-review of the complete non-planning change set from `3290823` through `d1b80e0` found four shipping blockers. The repair series closes prior CR-01, CR-02, CR-04, CR-05, CR-06, CR-07, WR-01, and WR-02. CR-03's ordinary full-capacity case is fixed, but the new store reservation can still be stolen by an unreserved acceptance, so it remains open under concurrency.

The durable edit is now exactly `{ kind, identifier, change }`; author is resolved once and persisted beside custody; schema v2 refuses incompatible durable state; addressable selection uses exact author/kind/identifier matching; no executable `actor`, `format`, `inverse`, or codec-version edit path remains. Focused Cargo, Bazel, Python feature mapping, vocabulary, formatting, clippy, external-capability, and process-kill targets pass, but the current tests either miss or explicitly encode the defects below.

## Narrative Findings (AI reviewer)

## Critical Issues

### CR-03 [BLOCKER]: An unreserved acceptance can steal a reserved active slot

**Files:** `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-memory/src/lib.rs:91-99`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-memory/src/semantic.rs:99-143`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-redb/src/ops.rs:34-46`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-redb/src/semantic.rs:41-83`

**Issue:** `reserve_active` counts active receipts plus reservations, but both stores' ordinary `accept` paths and unreserved semantic paths check only active receipts. With capacity one, semantic acceptance A can reserve the only slot, then ordinary acceptance B can commit into that slot. A has already been authorized to invoke its external materializer, but `accept_reserved_materialized_edit` removes A's reservation and refuses because B made `active_count == capacity`. The reservation is therefore not a permit: provider work can still run for an edit that loses pre-custody admission to a racing path. This leaves the original CR-03 and T-07-10 mitigation incomplete.

**Fix:** Make every unreserved admission count outstanding reservations, or require every capacity-consuming path to acquire and atomically consume the same permit primitive. Once a current reservation is consumed, do not reject solely because an unreserved path stole its promised slot. Add a deterministic capacity-one interleaving for both stores: reserve A, attempt raw/unreserved B, then consume A; assert B refuses before custody and A commits without a second capacity refusal.

### CR-08 [BLOCKER]: Rematerialization rejects the authoritative equal-timestamp winner

**Files:** `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-publication/src/materialization.rs:356-375`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-memory/src/state.rs:57-74`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-write-store-redb/src/semantic.rs:461-478`, `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava/tests/semantic_write_publication.rs:315-373`

**Issue:** The standard evaluator correctly chooses the lower event id when two replaceable events share a timestamp, but `semantic_successor` admits only a strictly greater timestamp. The equal-timestamp test deliberately starts from the higher id, adds the lower-id winner, and asserts that the edit remains applied to the losing event. Semantic custody therefore diverges from the event view required by EVENT-002's timestamp-and-event-id tie-breaking. Both store guards duplicate the strict-timestamp rule, so fixing selection alone would still refuse the correct winner transition.

**Fix:** Apply the same `(created_at, event_id)` winner ordering as the query evaluator: a greater timestamp wins, and at equal timestamp the lower event id wins. Change both store guards to enforce that complete ordering instead of timestamp alone. Reverse the existing equal-timestamp assertion and cover the transition through both memory and redb stores.

### CR-09 [BLOCKER]: A rematerialization resets the live router revision behind durable state

**File:** `/Users/pablofernandez/Work/nnn-m7-reaudit/crates/fava-publication/src/run.rs:105-122`

**Issue:** On a new materialization notification, `open_routes(&latest)` immediately persists revision `latest.route_revision + 1`, but line 120 then resets the runner's local `route_revision` to the pre-update value from `latest`. The following route-update notification is ignored because its materialization id did not change. The first later contribution from the newly opened router therefore reuses the already committed revision; `apply_route_to_receipt` refuses it as stale, and the fallback shortfall uses the same stale revision and is also refused. A valid live route change after semantic rematerialization is silently lost.

**Fix:** Have route opening return the revision it actually committed, or reread the receipt after opening and initialize the local counter from durable current state. Also refresh the local revision on same-materialization receipt changes. Add a controlled semantic-source successor followed by a delayed router contribution and assert that the new destination commits at a strictly newer revision.

### CR-10 [BLOCKER]: Bounded-output errors bypass process-group cleanup

**File:** `/Users/pablofernandez/Work/nnn-m7-reaudit/apps/canary/src/semantic_process.rs:49-68`

**Issue:** After a successful owner exit, `collect_readers(...).await?` propagates any stdout/stderr reader error before `clean_process_group` runs. `read_bounded` returns exactly such an error when output exceeds 1 MiB. A canary child can therefore emit oversized output, retain a redirected descendant, exit successfully, and leave that descendant running indefinitely. The repaired cleanup covers normal successful output and timeout paths, but not output-bound, reader-I/O, or reader-join failures.

**Fix:** Make process-group cleanup unconditional after spawn. Capture the owner/read result without `?`, clean the group and reap/abort readers in a finally-style path, then return the original error (or a combined cleanup error). Add a test whose successful owner emits oversized output and leaves a redirected `sleep` descendant; assert the call refuses and the descendant is absent or zombie afterward.

---

_Reviewed: 2026-08-21T16:50:24Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: deep_
