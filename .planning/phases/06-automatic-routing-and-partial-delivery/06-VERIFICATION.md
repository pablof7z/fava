---
phase: 06-automatic-routing-and-partial-delivery
verified: 2026-08-21T05:35:30Z
status: passed
score: 12/12 requirements verified
execution_origin: pre-gsd
---

# Phase 6: Automatic Routing and Partial Delivery Verification

**Phase Goal:** Applications can publish immediately to known automatic destinations and add later destinations under the same signed event and receipt without duplicate delivery.

## Reconciliation Basis

M6 predates GSD phase artifacts. The owning completion record is
`docs/issues/0008-automatic-write-routing.md`; the implementation commit is
`309e421`. No retrospective PLAN.md or SUMMARY.md is fabricated.

## Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| WRITE-12 | ✓ SATISFIED | application-selected ordered router chain is the automatic write policy |
| WRITE-13 | ✓ SATISFIED | outbox missing-list acquisition uses exact explicit kind:10002 indexer query |
| WRITE-14 | ✓ SATISFIED | hint router independently uses references and admitted relay evidence |
| WRITE-15 | ✓ SATISFIED | app-relay router contributes configured write scope |
| WRITE-16 | ✓ SATISFIED | fallback contribution reacts and retracts independently |
| WRITE-17 | ✓ SATISFIED | known destinations deliver before unresolved discovery settles |
| WRITE-18 | ✓ SATISFIED | later destinations join the same receipt and signed event |
| WRITE-19 | ✓ SATISFIED | duplicate contributions produce one handoff per relay |
| WRITE-20 | ✓ SATISFIED | route withdrawal retires only pre-handoff work and preserves historical facts |
| WRITE-21 | ✓ SATISFIED | open work applies exact route revisions and lane generations |
| WRITE-22 | ✓ SATISFIED | side-effect-free preview matches initial publication routing |
| WRITE-23 | ✓ SATISFIED | routing, fan-out, text, destination, receipt, and history bounds are typed/atomic |

## Current Validation

The 2026-08-21 fast validation set passed. Preserved M6 bundles report all four
real-relay scenarios passing, including five-relay partial delivery and later
route expansion under one receipt. The wait-for-settlement falsifier is recorded
in the issue.

## Gaps Summary

No M6 gaps remain. Partial-handoff cancellation and full attempt-history
retention remain explicitly unpromised product decisions, not failed M6 gates.
