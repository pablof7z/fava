# Planning state reconciliation through M6

**Status:** complete
**Branch:** `planning/reconcile-m0-m6`

## Problem

The GSD project artifacts were generated from the pre-M1 codebase map after
M1-M6 had already landed. They therefore route to Phase 1, leave every v1
requirement unchecked, and describe M2-M11 as absent even though the milestone
ledger and `main` contain completed M1-M6 slices.

## Scope

- Refresh `.planning/codebase/` against the current M6 checkout.
- Reconcile `.planning/PROJECT.md`, `.planning/REQUIREMENTS.md`,
  `.planning/ROADMAP.md`, and `.planning/STATE.md` with the completed milestone
  ledger.
- Preserve the distinction between historical milestone execution and GSD plan
  execution: do not invent PLAN.md files, task summaries, dates, or validations.
- Backfill only the minimum phase evidence that GSD needs to recognize completed
  phases and route to Phase 7.

## Authority and evidence

- M0: `docs/issues/0002-m0-evidence-foundation.md`, commit `83a05ae`.
- M1: `docs/issues/0001-local-source-merge.md`, commit `6be0fa5`.
- M2: `docs/issues/0004-explicit-live-query.md`, commit `7fac920`.
- M3: `docs/issues/0005-multi-relay-observation.md`, commit `1f2c0ed`.
- M4: `docs/issues/0006-ordered-automatic-routing.md`, commit `9860711`.
- M5: `docs/issues/0007-durable-explicit-publication.md`, commit `7e5820f`.
- M6: `docs/issues/0008-automatic-write-routing.md`, commit `309e421`.

All milestone commits are ancestors of the reconciliation branch. Current fast
validation is green for the Cargo workspace, strict Clippy, formatting, the
independent canary and external-provider packages, vocabulary checks, and all
Bazel targets. Historical real-relay evidence bundles remain in their milestone
worktrees; those external scenarios are not rerun by this documentation slice.

## Exit gates

- The seven codebase-map documents describe the M6 checkout rather than the
  pre-M1 tracer.
- Phases 1-6 and their 66 mapped requirements are recorded complete with direct
  milestone-ledger evidence.
- Phase 7 is the current phase and the first incomplete phase.
- `gsd-tools` roadmap analysis, init, state snapshot, and smart entry agree that
  Phases 1-6 are complete and Phase 7 is next. Plan metrics remain truthfully
  0/0 because no retrospective plans or summaries are invented.
- Health reports no planning error; pre-existing clean milestone worktrees may
  remain visible as non-blocking stale-worktree warnings.
- No production source, normative specification, milestone evidence, remote, or
  ignored live-run bundle is changed.

## Outcome

- Refreshed all seven codebase-map documents against the M6 checkout.
- Reconciled the project, requirement, roadmap, state, and research ledgers:
  66 of 110 requirements are complete; the remaining 44 begin at Phase 7.
- Added six passing, pre-GSD phase verification records. Each names its original
  issue and commit; no retrospective plan or summary was fabricated.
- Confirmed GSD recognizes Phases 1-6 as complete and routes to Phase 7. The
  phase ledger is 6/11 while the independent plan ledger remains honestly 0/0.
- Confirmed the 66 checked requirements match the 66 verification rows exactly.
- `git diff --check` passes; stale-text and secret scans are empty; no production
  or normative-spec path changed. Health has no errors and only reports the six
  preserved milestone worktrees as non-repairable `W027` warnings.
