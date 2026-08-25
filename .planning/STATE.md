---
gsd_state_version: 1.0
current_phase: 07.3
current_phase_name: Architecture Gate Integrity and Requirement Traceability
status: not_started
stopped_at: Phase 07.2 complete and merged; remediation series 07.3-07.9 inserted from the 2026-08-23 architecture audit
last_updated: "2026-08-23T14:30:00Z"
last_activity: 2026-08-23
last_activity_desc: Phase 07.2 merged; full-workspace architecture audit complete; remediation phases 07.3-07.9 inserted before Phase 8
state_head: HEAD
progress:
  total_phases: 22
  completed_phases: 9
  total_plans: 38
  completed_plans: 38
  percent: 41
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-21)

**Core value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.
**Current focus:** Phase 8 — Authentication, Hostile Boundaries, and Boundedness

## Current Position

Phase: 07.3 (Architecture Gate Integrity and Requirement Traceability) — NOT STARTED
Plan: none authored
Status: not_started
Last activity: 2026-08-23 — Phase 07.2 merged; architecture audit complete; remediation series inserted

Progress: [████░░░░░░] 41%

Phase progress is 9/22 after the Phase 07.1.1 verdict revocation. Phase 07.2 (Runtime Signer Lifecycle) completed and
merged as `0b23b52`, delivering `crates/fava-session` and migrating
`fava-publication` off its private signer registry.

A full-workspace architecture audit on 2026-08-23 recorded **164 findings
(59 critical, 80 major, 25 minor)** in `.planning/audit/2026-08-23/LEDGER.md`
and inserted the remediation series 07.3-07.9 before Phase 8.

**Completion verdicts under revocation.** The audit established that
`.planning/REQUIREMENTS.md` was authored after M6 shipped and reverse-engineered
from finished code, that 113 of 131 authoritative spec requirement IDs appear
nowhere in `.planning/`, that the M2-M6 verification records cite evidence
authored by the change they verify, and that CI has never run `cargo test` —
`.github/workflows/` holds one file running two Python steps. Phase 07.9 revokes
or re-earns the M1, M2, M3, M5, M6, and 07.1.1 verdicts, downgrades M4, and
retains Phase 7. Until then the completed-phase counts above are provisional
and must not be read as proof.

## Performance Metrics

**Velocity:**

- Total plans completed: 26
- Average duration: 35 minutes
- Total execution time: 7 hours 55 minutes

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
| Phase 07.1.1 P09 | 24min | 3 tasks | 13 files |
| Phase 07.1.1 P10 | 21min | 3 tasks | 7 files |
| Phase 07.1.1 P11 | 40min | 3 tasks | 10 files |
| Phase 07.1.1 P12 | 6min | 2 tasks | 5 files |
| Phase 07.2 P01 | 9min | 2 tasks | 19 files |
| Phase 07.2 P02 | 20min | 2 tasks | 6 files |

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
- [Phase 07.1.1]: The NIP-29 capability is `fava-simple-groups`; its README is the public North Star for the multi-relay `SimpleGroup`, event-local decoders, and kind-10009 edits.
- [Phase 07.1.1]: One group id uses a normalized non-empty relay sequence; event-local state decoders leave relay-local selection to generic query evidence, and reads/writes reuse ordinary Fava lifecycles.
- [Phase 07.1]: Unscoped publish(edit) compiles but refuses MissingAuthor before custody until by(author) supplies the frozen author; a sole registered signer is never selected implicitly.
- [Phase 07.1]: Facade Write holds stable identities and reads current receipt facts through Publication; neutral owners retain custody and receipt authority.
- [Phase 07.1]: PublishAs is a borrowed edit-only handle and PublishTo is a borrowed sealed-payload handle; neither owns providers or performs work until publish receives a valid payload.
- [Phase 07.1]: WriteRouting remains publicly re-exported from fava-write, while Explicit stores a first-occurrence sequence and keyed destination maps remain derived lane facts.
- [Phase 07.1]: Equivalent exact event contributions expose the newest receipt local evidence to semantic queries while every WriteId and ReceiptId remains independently readable.
- [Phase 07.1]: all() requires settled routing and exact terminal facts for every currently desired destination; mixed terminal outcomes satisfy it.
- [Phase 07.1]: Settlement subscribes before reading and reloads complete durable state after relevant or lagged notifications; terminal refusal carries the full Receipt.
- [Phase 07.1]: Only a fully valid NIP-02 tag entry enters duplicate membership; invalid entries never reserve targets.
- [Phase 07.1]: Whole-event contact-list failures are ContactListError values; every entry-local failure remains exact ContactListEntryError.
- [Phase 07.1]: Follow uses the established fava-state RelayUrl directly, with no NIP-02 relay wrapper or lifecycle.
- [Phase 07.1]: IntoContactAuthors is sealed and supports exact owned or borrowed one/many key shapes; every shape feeds the same present author axis.
- [Phase 07.1]: contact_list applies no global limit because ordinary replacement evaluation selects one newest kind-3 event independently for every author coordinate.
- [Phase 07.1]: followers_of uses exact lowercase p with canonical subject hex; follows_of owns no mutable state and preserves snapshot then entry order.
- [Phase 07.1]: follow and unfollow accept standard ToString inputs parsed by upstream PublicKey::parse, avoiding new target vocabulary.
- [Phase 07.1]: NIP-02 metadata add uses distinct opcode 3 while original 33-byte add/remove meanings remain unchanged.
- [Phase 07.1]: Kind-3 edits preserve every nonmatching tag and content byte; only exact valid-key target matches are retained once or removed.
- [Phase 07.1]: Facade callers use publish/by/to; neutral intents stay in fava-write preview/store fixtures, and Plan 08 owns shared capability support without a compatibility adapter.
- [Phase 07.1]: Application harnesses publish through by and to and consume Write; embedded providers import WriteIntent only from fava-write.
- [Phase 07.1]: Facade publication exposes payload scopes and returned Write, while neutral WriteIntent and AcceptedWrite stay in their provider contract crates.
- [Phase 07.1]: WriteRouting remains facade-visible because Receipt::routing returns it; all other old intent and wait compatibility doors are removed.
- [Phase 07.1]: Canary application flows publish payloads through publish, by, and to; only preview/store/provider boundaries construct neutral WriteIntent values.
- [Phase 07.1]: Recovered durable obligations without their original Write handle reattach through subscribe-before-read receipt facts and an exact ReceiptId.
- [Phase 07.1]: Applications publish payloads through optional inert by/to scopes and receive Write after synchronous durable acceptance; neutral WriteIntent and AcceptedWrite remain internal owner vocabulary.
- [Phase 07.1]: ContactList accounts for every p-tag entry; NIP-02 edit materialization owns foreign tag and content preservation.
- [Phase 07.1]: Simple-groups consumes the universal publication door without claiming Phase 07.1.1 delivery or a current ValueSet surface.
- [Phase 07.1]: Croissant is supervised as its exact executable, with executable SHA and source HEAD recorded separately.
- [Phase 07.1]: Kind 9007 and kind 3 cross the same public kind-blind publication lifecycle.
- [Phase 07.1]: PublishError retains the complete terminal Receipt without boxing; narrow Clippy allowances document that evidence boundary.
- [Phase 07.1]: Seed-bearing Cargo invocations use --quiet so Cargo cannot echo process-memory secrets in argv diagnostics.
- [Phase 07.1.1]: Query collection axes have one neutral bounded contract each; singleton kind selection uses `kinds([kind])` and cannot accumulate through a parallel scalar API.
- [Phase 07.1.1]: Vocabulary metadata exclusions apply to exact candidate spans; neighboring prose and path crate references remain enforced.
- [Phase 07.1.1]: Relay-observed replaceable state retains the union of newest event ids selected independently for each exact RelayUrl.
- [Phase 07.1.1]: OnlyRelays selects per requested RelayUrl while preserving the accepted-local replacement overlay; AnyLocal remains globally replaceable.
- [Phase 07.1.1]: `SimpleGroup::from_relays(id, first, rest)` accepts one required parsed `RelayUrl` plus a finite owned `Vec<RelayUrl>` tail, preserving the opaque id and first occurrences while making empty and arbitrary-iterator construction impossible.
- [Phase 07.1.1]: SimpleGroupStateEventKind inputs delegate to the bounded query-selection owner; content and state retain distinct AnyLocal and OnlyRelays authority without capability-private limits.
- [Phase 07.1.1]: Content query lowering delegates exact h-axis narrowing to query-owned `Query::intersect_tag_values`; disjoint axes stay present-empty match-nothing, unrelated fields survive, and every selected relay is asked without group-owned validation or failure translation.
- [Phase 07.1.1]: Unsigned preparation preserves every tag and appends one matching h tag only when no existing h tag's first value matches; signed preparation is absent.
- [Phase 07.1.1]: The facade consumes fava-simple-groups only through Cargo dev-dependencies and one Bazel test edge; production facade source and dependencies remain capability-blind.
- [Phase 07.1.1]: Applications compose simple_group.prepare(payload), fava.to(simple_group.relays()), and publish directly for an ordinary Write; SimpleGroup owns no publication method or lifecycle.
- [Phase 07.1.1]: Event-local state decoders accept EventValue, check only exact kind and the first d tag's first value, and leave id/signature verification, replacement, relay evidence, and projection to generic owners.
- [Phase 07.1.1]: Recognized semantic entries expose source-ordered Result values; repetitions and valid siblings survive malformed material, while unknown tags and unused extra values are ignored.
- [Phase 07.1.1]: Pins reuse EventCoordinate, saved relays reuse RelayUrl, and people entries reuse PublicKey/String tuples without new entry-evidence, attribution, or snapshot vocabulary.
- [Phase 07.1.1]: SavedGroupList decodes one kind-10009 event; crate-root save, rename, remove, and relay edits preserve unrelated material through the generic ReplaceableEventEdit lifecycle.
- [Phase 07.1.1]: The public simple-groups North Star composes pure preparation with ordinary facade to/publish/Write; SimpleGroup owns no lifecycle.
- [Phase 07.1.1]: Capability production dependencies equal fava-query, fava-state, fava-write, and nostr only for the typed relay-parser error; universal owners stay NIP-29 blind.
- [Phase 07.1.1]: Simple-group vocabulary remains an unsigned approval candidate; query builders return neutral QueryError values directly, relay construction delegates to fava-state, and the README and machine catalog mirror the compiler-derived surface without treating its size as a design target.
- [Phase 07.2]: Session is the sole mutable signer attachment owner; publication retains only per-write signer operations.
- [Phase 07.2]: Runtime add, explicit replace, and remove use one exact signer per pubkey with a fixed 64-entry bound and atomic typed refusal.
- [Phase 07.2]: Session revisions are coalescible wake signals only; publication reloads its exact event pubkey and admits completions by current attachment generation.
- [Phase 07.2]: Cancellation is advisory; a valid stale provider completion remains inert after replacement or removal.

### Pending Todos

1 pending — Evaluate pagination through query primitives (major, docs).

### Blockers/Concerns

- No current blocker.
- Targeted research remains required during planning for Phases 8-11; recommendations do not override specifications.

### Roadmap Evolution

- Phase 06.1 inserted after Phase 6: Literal Tag-Value Query Semantics Remediation
- Phase 07.1 inserted after Phase 7: Universal publication vocabulary and typed NIP-02 reads (URGENT)
- Phase 07.1.1 inserted after Phase 07.1: Deliver fava-simple-groups as the multi-relay NIP-29 capability
- Phase 07.2 inserted after Phase 7: Runtime signer lifecycle and parked-write wakeup (URGENT)
- Phase 07.3 inserted after Phase 7: Make the vocabulary, requirement, and verification gates truthful (URGENT)
- Phase 07.4 inserted after Phase 7: Reshape transport, subscription, evidence, and diagnostics contracts (URGENT)
- Phase 07.5 inserted after Phase 7: Create the fava-runtime execution owner (URGENT)
- Phase 07.6 inserted after Phase 7: Restore fava-observe live-query ownership and delete the facade relay layer (URGENT)
- Phase 07.7 inserted after Phase 7: Facade lifecycle and fava-session signer ownership (URGENT)
- Phase 07.8 inserted after Phase 7: Independent correctness defects in ingest, routing, and cache (URGENT)
- Phase 07.9 inserted after Phase 7: Evidence reconstruction and milestone verdict revocation (URGENT)

## Deferred Items

Items acknowledged and deferred at milestone close, most recent first:

| Category | Item | Status | Deferred At | Milestone |
|----------|------|--------|-------------|-----------|
| *(none)* | | | | |

## Session Continuity

Last session: 2026-08-23T14:01:18Z
Stopped at: Completed and verified Phase 07.2
Resume file: None
