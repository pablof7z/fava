---
gsd_state_version: 1.0
current_phase: 07.1
current_phase_name: Universal publication vocabulary and typed NIP-02 reads
status: executing
stopped_at: Completed 07.1-05-PLAN.md
last_updated: "2026-08-22T00:54:30.154Z"
last_activity: 2026-08-22
last_activity_desc: Plan 07.1-05 completed; typed NIP-02 reads and pure follow projection delivered
state_head: d602f1dbe1c7d426b19f200e31adee472d808e31
progress:
  total_phases: 14
  completed_phases: 8
  total_plans: 24
  completed_plans: 17
  percent: 57
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-21)

**Core value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.
**Current focus:** Phase 07.1 — Universal publication vocabulary and typed NIP-02 reads

## Current Position

Phase: 07.1 (Universal publication vocabulary and typed NIP-02 reads) — EXECUTING
Plan: 6 of 12
Status: Ready to execute
Last activity: 2026-08-22 — Plan 07.1-05 completed; typed NIP-02 reads and pure follow projection delivered

Progress: [██████░░░░] 57%

Phase progress is 8/14. Phases 1-6 predate GSD plans; Phases 06.1 and 7 completed
all authored plans and their review and verification gates.

## Performance Metrics

**Velocity:**

- Total plans completed: 13
- Average duration: 35 minutes
- Total execution time: 7 hours 35 minutes

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| Phase 07 | 9 | 407 min | 45 min |
| Phase 06.1 | 3 | 42 min | 14 min |
| Phase 07.1 | 1 | 6 min | 6 min |

**Recent Trend:**

- Last 5 plans: 38 min, 15 min, 9 min, 18 min, 6 min
- Trend: Phase 07.1 began below the Phase 06.1 remediation average

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
| Phase 07.1 P01 | 6min | 2 tasks | 9 files |
| Phase 07.1 P02 | 21min | 3 tasks | 16 files |
| Phase 07.1 P03 | 11min | 2 tasks | 7 files |
| Phase 07.1 P04 | 16min | 2 tasks | 12 files |
| Phase 07.1 P05 | 10min | 2 tasks | 11 files |

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
- [Phase 07.1]: Unscoped publish(edit) compiles but refuses MissingAuthor before custody until by(author) supplies the frozen author; a sole registered signer is never selected implicitly.
- [Phase 07.1]: Facade Write holds stable identities and reads current receipt facts through Publication; neutral owners retain custody and receipt authority.
- [Phase 07.1]: PublishAs is a borrowed edit-only handle and PublishTo is a borrowed sealed-payload handle; neither owns providers or performs work until publish receives a valid payload.
- [Phase 07.1]: WriteRouting remains publicly re-exported from fava-write, while Explicit stores a first-occurrence sequence and keyed destination maps remain derived lane facts.
- [Phase 07.1]: Equivalent exact event contributions expose the newest receipt local evidence to semantic queries while every WriteId and ReceiptId remains independently readable.
- [Phase 07.1]: all() requires settled routing and exact terminal facts for every currently desired destination; mixed terminal outcomes satisfy it.
- [Phase 07.1]: Settlement subscribes before reading and reloads complete durable state after relevant or lagged notifications; terminal refusal carries the full Receipt.
- [Phase 07.1]: Only a fully valid NIP-02 row enters duplicate membership; invalid rows never reserve targets.
- [Phase 07.1]: Whole-event contact-list failures are ContactListError values; every row-local failure remains exact ContactListRowEvidence.
- [Phase 07.1]: Follow uses the established fava-state RelayUrl directly, with no NIP-02 relay wrapper or lifecycle.
- [Phase 07.1]: IntoContactAuthors is sealed and supports exact owned or borrowed one/many key shapes; every shape feeds the same present author axis.
- [Phase 07.1]: contact_list applies no global limit because ordinary replacement evaluation selects one newest kind-3 event independently for every author coordinate.
- [Phase 07.1]: followers_of uses exact lowercase p with canonical subject hex; follows_of owns no mutable state and preserves snapshot then row order.

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

Last session: 2026-08-22T00:54:30.069Z
Stopped at: Completed 07.1-05-PLAN.md
Resume file: None
