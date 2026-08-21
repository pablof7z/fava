---
phase: 02-exact-single-relay-live-query
verified: 2026-08-21T05:35:30Z
status: passed
score: 10/10 requirements verified
execution_origin: pre-gsd
---

# Phase 2: Exact Single-Relay Live Query Verification

**Phase Goal:** Applications can run one exact explicit live query against a real relay with verified admission, source-scoped evidence, and deterministic cancellation.

## Reconciliation Basis

M2 predates GSD phase artifacts. The owning completion record is
`docs/issues/0004-explicit-live-query.md`; the implementation commit is
`7fac920`. No retrospective PLAN.md or SUMMARY.md is fabricated.

## Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| READ-01 | ✓ SATISFIED | exact explicit relay path bypasses automatic routing |
| READ-02 | ✓ SATISFIED | live work begins when the public observation opens |
| READ-03 | ✓ SATISFIED | bounded NIP-01 wire and WebSocket transport corpora |
| READ-04 | ✓ SATISFIED | ingest attributes session, request generation, context, and subscription |
| READ-05 | ✓ SATISFIED | forged, wrong-subscription, off-filter, stale, and terminal input refusal |
| READ-06 | ✓ SATISFIED | EOSE exists only after the attributed wire frame |
| READ-07 | ✓ SATISFIED | public relay facts distinguish silence, EOSE, auth, close, failure, and cancellation |
| READ-08 | ✓ SATISFIED | real-relay canary receives live events after EOSE |
| READ-09 | ✓ SATISFIED | exact CLOSE, pending-pull wakeup, and post-cancel refusal |
| READ-10 | ✓ SATISFIED | idempotent transport/query close and owned-resource release |

## Current Validation

The 2026-08-21 fast validation set passed. Preserved M2 run bundles contain
passing `explicit-read-eose`, `explicit-read-live-after-eose`, and
`explicit-read-cancel` reports against `nostr-rs-relay 0.8.12`, plus the
deliberate forged-event failure recorded by the issue. External scenarios were
inspected, not rerun, during this reconciliation.

## Gaps Summary

No M2 gaps remain.
