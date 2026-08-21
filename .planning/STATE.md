---
gsd_state_version: 1.0
current_phase: 07.1
current_phase_name: Universal publication vocabulary and typed NIP-02 reads
status: Ready to execute
stopped_at: Phase 07.1 planned and independently verified; run $gsd-execute-phase 07.1
last_updated: "2026-08-21T23:22:04.747Z"
last_activity: 2026-08-22
last_activity_desc: Phase 07.1 planned in 12 plans across 9 waves; plan verification passed
state_head: 6db7222b3f8f6ad660519937567ec32c56153776
progress:
  total_phases: 14
  completed_phases: 8
  total_plans: 24
  completed_plans: 12
  percent: 50
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-21)

**Core value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.
**Current focus:** Phase 07.1 — Universal publication vocabulary and typed NIP-02 reads

## Current Position

Phase: 07.1 (Universal publication vocabulary and typed NIP-02 reads) — READY TO EXECUTE
Plan: —
Status: Ready to execute
Last activity: 2026-08-22 — Phase 07.1 plan verification passed

Progress: [██████░░░░] 57%

Phase progress is 8/14. Phases 1-6 predate GSD plans; Phases 06.1 and 7 completed
all authored plans and their review and verification gates.

## Performance Metrics

**Velocity:**

- Total plans completed: 12
- Average duration: 37 minutes
- Total execution time: 7 hours 29 minutes

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| Phase 07 | 9 | 407 min | 45 min |
| Phase 06.1 | 3 | 42 min | 14 min |

**Recent Trend:**

- Last 5 plans: 134 min, 38 min, 15 min, 9 min, 18 min
- Trend: Phase 06.1 remediation slices completed well below the Phase 7 capstone duration

**Per-Plan Metrics:**

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 07 P01 | 11min | 2 tasks | 13 files |
| Phase 07 P02 | 18min | 2 tasks | 8 files |
| Phase 07 P03 | 32min | 2 tasks | 20 files |
| Phase 07 P04 | 58min | 2 tasks | 30 files |
| Phase 07 P05 | 40min | 2 tasks | 21 files |
| Phase 07 P06 | 39min | 3 tasks | 17 files |
| Phase 07 P07 | 37min | 2 tasks | 8 files |
| Phase 07 P08 | 134min | 3 tasks | 24 files |
| Phase 07 P09 | 38min | 2 tasks | 14 files |
| Phase 06.1 P01 | 15min | 3 tasks | 7 files |
| Phase 06.1 P02 | 9min | 2 tasks | 8 files |
| Phase 06.1 P03 | 18min | 2 tasks | 9 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Current planning decisions:

- M0 remains a completed prerequisite baseline outside the active roadmap.
- M1-M6 were completed before GSD phase artifacts existed; their focused issue records, commits, current validation, and phase verification reports are the completion provenance.
- No retrospective PLAN.md or SUMMARY.md files were invented during reconciliation.
- Major phases correspond to M1 through M11; focused inserted phases may repair or add approved public capabilities before the next major milestone.
- Every phase uses MVP mode; milestone names require every authoritative exit gate and mapped requirement Definition of Done item to pass.
- Windowing/resume tokens, outage backfill, partial-handoff cancellation, full attempt-history retention, and the recommended persistent event-cache profile remain explicitly unpromised pending their owning phase evidence.
- [Phase 06.1]: Use nostr::filter::SingleLetterTag directly with no Fava wrapper or compatibility alias.
- [Phase 06.1]: Canonical literal tag axes union exact values per case-sensitive key and preserve present-empty match-nothing semantics.
- [Phase 06.1]: Evaluate exact tag cells locally without delegating Fava query meaning to the upstream whole-filter matcher.
- [Phase 06.1]: Attempt all 300 no-grouping subscriptions concurrently; batch at 32 only after the controlled relay's exact capacity refusal.
- [Phase 06.1]: Compare exact serving RelaySessionKey values per logical result; observation timestamps remain execution-local facts.
- [Phase 06.1]: Ignore a crate-like vocabulary candidate only when the same line identifies that exact token as a /tmp evidence path.
- [Phase 07]: Apps own complete raw event bodies through EventBuilder, including exact created_at, tags, content, and kind; semantic edits retain engine-owned monotonic rematerialization time. — Raw events are complete caller-authored values, while semantic edits must be rematerializable against changing sources without a hidden clock override.
- [Phase 07]: `ReplaceableEventEdit` is exactly `{ kind, identifier, change }`; acceptance freezes and persists the author separately, and opposing operations are separate edits. — This preserves addressable coordinates without giving protocol values lifecycle ownership.
- [Phase 07.1.1]: The NIP-29 capability is `fava-simple-groups`; its README is the public North Star and multi-relay `Group` aggregation is required, not provisional.
- [Phase 07.1.1]: One group id may be aggregated over a non-empty host set while each relay's records remain independently authoritative; reads/writes reuse ordinary Fava lifecycles.

### Pending Todos

1 pending — Evaluate pagination through query primitives (major, docs).

### Blockers/Concerns

- No current blocker.
- Phase 06.1 awaits final goal verification; implementation and code review are complete.
- Targeted research remains required during planning for Phases 8-11; recommendations do not override specifications.

### Roadmap Evolution

- Phase 06.1 inserted after Phase 6: Literal Tag-Value Query Semantics Remediation
- Phase 07.1 inserted after Phase 7: Universal publication vocabulary and typed NIP-02 reads (URGENT)
- Phase 07.1.1 inserted after Phase 07.1: Deliver fava-simple-groups as the multi-relay NIP-29 capability

## Deferred Items

Items acknowledged and deferred at milestone close, most recent first:

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-08-21T20:45:19.179Z
Stopped at: Phase 07.1 inserted after Phase 7; run $gsd-spec-phase 07.1
Resume file: None
