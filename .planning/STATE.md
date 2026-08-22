---
gsd_state_version: 1.0
current_phase: 07.1.1
current_phase_name: Multi-Relay Simple Groups
status: executing
stopped_at: Completed 07.1.1-08-PLAN.md
last_updated: "2026-08-22T11:25:30.591Z"
last_activity: 2026-08-22
last_activity_desc: Phase 07.1.1 execution started
state_head: 0ea48a654b899b2ce4e8fc306babff16bb92a30e
progress:
  total_phases: 14
  completed_phases: 9
  total_plans: 36
  completed_plans: 32
  percent: 64
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-21)

**Core value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.
**Current focus:** Phase 07.1.1 — Multi-Relay Simple Groups

## Current Position

Phase: 07.1.1 (Multi-Relay Simple Groups) — EXECUTING
Plan: 9 of 12
Status: Ready to execute
Last activity: 2026-08-22 — Phase 07.1.1 execution started

Progress: [██████░░░░] 64%

Phase progress is 8/14. Phases 1-6 predate GSD plans; Phases 06.1 and 7 completed
all authored plans and their review and verification gates.

## Performance Metrics

**Velocity:**

- Total plans completed: 25
- Average duration: 35 minutes
- Total execution time: 7 hours 35 minutes

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| Phase 07 | 9 | 407 min | 45 min |
| Phase 06.1 | 3 | 42 min | 14 min |
| Phase 07.1 | 1 | 6 min | 6 min |
| 07.1 | 12 | - | - |

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
| Phase 07.1 P06 | 11min | 2 tasks | 6 files |
| Phase 07.1 P07 | 18min | 3 tasks | 10 files |
| Phase 07.1 P08 | 6min | 2 tasks | 4 files |
| Phase 07.1 P09 | 13min | 3 tasks | 14 files |
| Phase 07.1 P10 | 11min | 2 tasks | 9 files |
| Phase 07.1 P11 | 17min | 3 tasks | 9 files |
| Phase 07.1 P12 | 34min | 3 tasks | 13 files |
| Phase 07.1.1 P01 | 5min | 2 tasks | 6 files |
| Phase 07.1.1 P03 | 8min | 2 tasks | 2 files |
| Phase 07.1.1 P02 | 13min | 3 tasks | 6 files |
| Phase 07.1.1 P04 | 15min | 2 tasks | 10 files |
| Phase 07.1.1 P05 | 14min | 3 tasks | 8 files |
| Phase 07.1.1 P06 | 14min | 3 tasks | 4 files |
| Phase 07.1.1 P07 | 21min | 3 tasks | 13 files |
| Phase 07.1.1 P08 | 19min | 2 tasks | 8 files |

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
- [Phase 07.1]: follow and unfollow accept standard ToString inputs parsed by upstream PublicKey::parse, avoiding new target vocabulary.
- [Phase 07.1]: NIP-02 metadata add uses distinct opcode 3 while original 33-byte add/remove meanings remain unchanged.
- [Phase 07.1]: Kind-3 edits preserve every nonmatching row and content byte; only exact valid-key target matches are retained once or removed.
- [Phase 07.1]: Facade callers use publish/by/to; neutral intents stay in fava-write preview/store fixtures, and Plan 08 owns shared capability support without a compatibility adapter.
- [Phase 07.1]: Application harnesses publish through by and to and consume Write; embedded providers import WriteIntent only from fava-write.
- [Phase 07.1]: Facade publication exposes payload scopes and returned Write, while neutral WriteIntent and AcceptedWrite stay in their provider contract crates.
- [Phase 07.1]: WriteRouting remains facade-visible because Receipt::routing returns it; all other old intent and wait compatibility doors are removed.
- [Phase 07.1]: Canary application flows publish payloads through publish, by, and to; only preview/store/provider boundaries construct neutral WriteIntent values.
- [Phase 07.1]: Recovered durable obligations without their original Write handle reattach through subscribe-before-read receipt facts and an exact ReceiptId.
- [Phase 07.1]: Applications publish payloads through optional inert by/to scopes and receive Write after synchronous durable acceptance; neutral WriteIntent and AcceptedWrite remain internal owner vocabulary.
- [Phase 07.1]: ContactList accounts for every p row; NIP-02 edit materialization owns foreign tag and content preservation.
- [Phase 07.1]: Simple-groups consumes the universal publication door without claiming Phase 07.1.1 delivery or a current ValueSet surface.
- [Phase 07.1]: Croissant is supervised as its exact executable, with executable SHA and source HEAD recorded separately.
- [Phase 07.1]: Kind 9007 and kind 3 cross the same public kind-blind publication lifecycle.
- [Phase 07.1]: PublishError retains the complete terminal Receipt without boxing; narrow Clippy allowances document that evidence boundary.
- [Phase 07.1]: Seed-bearing Cargo invocations use --quiet so Cargo cannot echo process-memory secrets in argv diagnostics.
- [Phase 07.1.1]: Keep Query::kind as the sole singleton-input surface; repeated calls union in the existing canonical BTreeSet without new query vocabulary.
- [Phase 07.1.1]: Vocabulary metadata exclusions apply to exact candidate spans; neighboring prose and path crate references remain enforced.
- [Phase 07.1.1]: Relay-observed replaceable state retains the union of newest event ids selected independently for each exact RelayUrl.
- [Phase 07.1.1]: OnlyRelays selects per requested RelayUrl while preserving the accepted-local replacement overlay; AnyLocal remains globally replaceable.
- [Phase 07.1.1]: Group::on is the sole one/many constructor; a private conversion bound accepts RelayUrl and string inputs without adding public host vocabulary.
- [Phase 07.1.1]: GroupRecords lowers its fixed all-record selection through six repeated approved Query::kind calls; content and records retain distinct AnyLocal and OnlyRelays authority.
- [Phase 07.1.1]: Group host bounds count every supplied item before duplicate normalization, so duplicate and infinite inputs cannot evade the universal route ceiling.
- [Phase 07.1.1]: Content helpers require an existing positive limit no greater than 4096 while retaining every unrelated Query field and AnyLocal authority.
- [Phase 07.1.1]: Unsigned preparation normalizes one exact h row losslessly; signed preparation validates and returns the original Event object.
- [Phase 07.1.1]: The facade consumes fava-simple-groups only through Cargo dev-dependencies and one Bazel test edge; production facade source and dependencies remain capability-blind.
- [Phase 07.1.1]: Applications compose group.prepare(payload), fava.to(group.hosts()), and publish directly and receive ordinary Write; Group owns no publication method or lifecycle.
- [Phase 07.1.1]: Invalid signed group context is refused by pure preparation before any facade custody, signer, router, publisher, transport, or wire interaction.
- [Phase 07.1.1]: Relay-authored records accept signed EventValue only and distinguish invalid id from invalid signature before typed decoding.
- [Phase 07.1.1]: Recognized multi-row inputs expose bounded source-ordered Result values; only a successfully parsed row reserves its exact duplicate key.
- [Phase 07.1.1]: Pinned addresses reuse EventCoordinate, saved hosts reuse RelayUrl, and people rows reuse PublicKey/String tuples without new row or attribution vocabulary.
- [Phase 07.1.1]: GroupSnapshot retains configured first-occurrence host order and selects one complete newest-valid typed record independently for each exact RelayUrl.
- [Phase 07.1.1]: Disagreement compares complete optional typed records, so observed versus unobserved differs without turning an empty view into a negative claim.
- [Phase 07.1.1]: Content keeps QuerySnapshot order, deduplicates repeated ids defensively, and merges every actual RelayEvidence observation.

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

Last session: 2026-08-22T11:25:30.504Z
Stopped at: Completed 07.1.1-08-PLAN.md
Resume file: None
