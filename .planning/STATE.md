---
gsd_state_version: 1.0
current_phase: 07.7
current_phase_name: Facade Lifecycle and fava-session Signer Ownership
status: not_started
stopped_at: Phases 07.3-07.6 executed without GSD plans and merged (2026-08-23); 07.7 not started; 07.8 has 1 of 5 plans complete
last_updated: "2026-08-29T00:00:00Z"
last_activity: 2026-08-29
last_activity_desc: Phases 07.3-07.6 merged via direct agent dispatch without GSD plans; 07.8-01 plan complete; 4 detached tokio::spawns flagged for fava-runtime registration
state_head: HEAD
progress:
  total_phases: 22
  completed_phases: 14
  total_plans: 38
  completed_plans: 38
  percent: 63
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-08-21)

**Core value:** Applications can rely on coherent live queries and durable writes with exact, bounded, failure-isolated lifecycle and evidence semantics across replaceable provider compositions.
**Current focus:** Phase 07.7 — Facade Lifecycle and fava-session Signer Ownership

## Current Position

Phase: 07.7 (Facade Lifecycle and fava-session Signer Ownership) — NOT STARTED
Plan: none authored
Status: not_started
Last activity: 2026-08-29 — Phases 07.3-07.6 merged without GSD plans; 07.8-01 done; 07.7 not started

Progress: [██████░░░░] 63%

Phase progress is 14/22. Phases 07.3 (Architecture Gate Integrity), 07.4
(Neutral Contract Correction), 07.5 (fava-runtime Execution Owner), and 07.6
(Restore fava-observe Live-Query Ownership) were implemented by direct agent
dispatch without GSD plan/execute/verify cycles (merged 2026-08-23). Each phase
directory holds an EXECUTED-WITHOUT-PLAN.md with no retrospective SUMMARY;
Phase 07.9 carries the verification obligation.

Phase 07.7 (Facade Lifecycle and fava-session Signer Ownership) is NOT STARTED.
Phase 07.8 (Independent Correctness Defects) has 1 of 5 plans complete
(07.8-01: settled absence requires an answer). Phase 07.9 is NOT STARTED.

**Detached tokio::spawns requiring fava-runtime registration:** Four
`tokio::spawn` calls remain unregistered with fava-runtime:
- `fava-routing/chain.rs`
- `fava-publication` (two locations)
- `fava/src/query_source.rs`
These must be registered before Phase 07.5's no-detached-task success criterion
can be re-verified.

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
- [Phase 07.1.1]: SimpleGroup record access returns one bounded exact-host query per configured RelayUrl; typed decoders interpret one complete selected event.
- [Phase 07.1.1]: Disagreement compares complete optional typed records, so observed versus unobserved differs without turning an empty view into a negative claim.
- [Phase 07.1.1]: Content keeps QuerySnapshot order and each event exposes exact event-id-bound RelayOccurrences.
- [Phase 07.1.1]: Discovery counts total author or subject inputs before Query canonicalization and refuses exactly at bound plus one.
- [Phase 07.1.1]: groups_saved_by projects canonical authors from exact group-id and selected-host pairs without ValueSet or lifecycle state.
- [Phase 07.1.1]: Kind-10009 saved-list edits preserve opaque content and foreign order through target-local surgery.
- [Phase 07.1.1]: Kinds 9002 and 9010 remain ordinary author-bearing events; only kind 10009 uses ReplaceableEventEdit.
- [Phase 07.1.1]: The public simple-groups North Star composes pure preparation with ordinary facade to/publish/Write; Group owns no lifecycle.
- [Phase 07.1.1]: Capability production dependencies equal fava-query, fava-state, and fava-write; facade edges remain test/application-only and universal owners stay NIP-29 blind.
- [Phase 07.1.1]: Closed vocabulary contains exactly the issue-0019-approved implemented crate and public nominal values, checking direct declarations and re-exports.
- [Phase 07.1.1]: Two controlled Croissant children remain individually owned and both cleanup results are captured before failure propagation.
- [Phase 07.1.1]: Public multi-relay group behavior retains relay-local record authority while ordinary Fava owners handle observation and publication.
- [Phase 07.1.1]: Only twice-scanned complete author-sealed evidence is atomically promoted; pair verification requires exactly two runs and four distinct children.
- [Phase 07.1.1]: Controlled canary evidence is durably retained at the exact owner-private path named by 07.1.1-12-PAIR-ROOT.txt and remains excluded from Git by the canary evidence policy.
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
