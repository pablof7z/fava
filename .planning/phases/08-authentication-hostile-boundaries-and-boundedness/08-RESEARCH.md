# Phase 08: Authentication, Hostile Boundaries, and Boundedness - Research

**Researched:** 2026-08-21  
**Domain:** NIP-42 relay authentication, hostile WebSocket ingress, NIP-11 limits, durable publication truth, provider isolation, and real-process boundedness  
**Confidence:** HIGH for repository state and remaining work; MEDIUM for external protocol/library documentation

## User Constraints

- Research and planning input only. Do not implement code, stage, commit, reset, stash, apply, or drop anything. [VERIFIED: orchestrator task]
- Plan only unfinished Phase 08 work. Inventory `HARD-01` through `HARD-10` from the live checkout, distinguish committed work from dirty WIP, and replace rather than trust the stale monolithic plan. [VERIFIED: orchestrator task]
- Byte-preserve the dirty worktree and `stash@{0}`. [VERIFIED: orchestrator task]
- Validation must prove public Fava behavior, hostile/boundary falsifiers, deliberate breaks, boundedness, restart, process, network, interoperability, and the complete M8 exit gate. [VERIFIED: orchestrator task]

<phase_requirements>
## Phase Requirements

The authoritative requirement values are quoted verbatim below. [VERIFIED: .planning/REQUIREMENTS.md:110-121]

| ID | Description | Research Support |
|---|---|---|
| HARD-01 | “Relay NIP-42 authentication is explicit, generation-scoped, and separate from event authorship and query filter identity.” | Committed owner/public scripted evidence exists; real-relay reconnect proof remains. [VERIFIED: .planning/REQUIREMENTS.md:112-112; git show ed6a76c; features/relay-authentication.feature:3-16] |
| HARD-02 | “Denial or failure of one account's authentication policy terminates only the exact affected operation and cannot block another account.” | Committed scripted isolation exists; separate-process account isolation remains. [VERIFIED: .planning/REQUIREMENTS.md:113-113; git show ed6a76c; features/relay-authentication.feature:18-30] |
| HARD-03 | “Invalid, malformed, oversized, off-filter, stale, post-CLOSED, never-EOSE, truncated, silent-limit, and disconnected relay behavior remains scoped and attributable.” | Dirty scripted hostile-ingress and frame-size evidence covers only a subset; wire/process coverage remains. [VERIFIED: .planning/REQUIREMENTS.md:114-114; crates/fava/tests/hostile_ingress.rs:1-250; crates/fava-transport-websocket/src/lib.rs:49-86] |
| HARD-04 | “NIP-11 limits produce a valid plan or exact shortfall before knowingly invalid work is sent.” | Committed typed projection and scripted public tests exist; advertised real NIP-11, exact no-wire witness, and second-relay proof remain. [VERIFIED: .planning/REQUIREMENTS.md:115-115; git show 94e04cd; features/relay-limits.feature:1-42] |
| HARD-05 | “Offline or unreachable time is distinct from a failed delivery attempt and does not consume the attempt budget.” | The focused fix is committed, but its public test currently compiles only with dirty receipt/outcome work; durable provider/restart closure remains. [VERIFIED: .planning/REQUIREMENTS.md:116-116; git show 197c278; crates/fava-write/src/lib.rs:345-362,511-535] |
| HARD-06 | “Real retryable attempts reach the configured terminal give-up policy within declared ceilings.” | Combined HEAD plus dirty WIP passes scripted evidence; real attempted-failure process proof remains. [VERIFIED: .planning/REQUIREMENTS.md:117-117; crates/fava-delivery-standard/src/lib.rs:23-63; cargo test -p fava --test delivery_bounds this session] |
| HARD-07 | “A completed handoff without a received relay outcome remains ambiguous and is never rewritten as acknowledged, rejected, or never sent.” | Scripted transport evidence exists; transparent-proxy crossing, restart, and public receipt proof remain. [VERIFIED: .planning/REQUIREMENTS.md:118-118; crates/fava/tests/delivery_bounds.rs:78-111; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:833-837] |
| HARD-08 | “Every externally influenced input, queue, set, fan-out, retained history, diagnostic stream, and artifact has an explicit bound, backpressure rule, refusal, or shortfall.” | Some bounds exist, but no exhaustive owner/resource inventory or phase resource envelope exists. [VERIFIED: .planning/REQUIREMENTS.md:119-119; docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376; repository-wide rg inspection this session] |
| HARD-09 | “Provider panic, blocking, late result, malformed result, or ignored cancellation cannot block unrelated queries, relays, writes, or shutdown.” | No Phase 08 provider-failure implementation/evidence was found outside specifications and older semantic-provider tests. [VERIFIED: .planning/REQUIREMENTS.md:120-120; repository-wide rg inspection this session] |
| HARD-10 | “Deterministic hostile scenarios use real sockets and separate processes, and publish resource envelopes and failure evidence for every run.” | No M8 scenario is registered or dispatched; the committed Khatru fixture is not integrated. [VERIFIED: .planning/REQUIREMENTS.md:121-121; apps/canary/scenarios.json:112-166; apps/canary/src/lib.rs:146-174; apps/canary/src/main.rs:66-124] |
</phase_requirements>

## Summary

Phase 08 is materially started but not complete: `HARD-01` through `HARD-08` are **partial**, `HARD-09` and `HARD-10` are **absent**, and no HARD requirement satisfies the repository Definition of Done because every one still lacks at least one required public, mutation, process, resource, or milestone-exit proof. [VERIFIED: requirement inventory above; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:792-859; AGENTS.md:30-38]

The branch contains three behavioral commits plus two resolved-debug documentation commits above its merge base: `ed6a76c`, `94e04cd`, `197c278`, `729565b`, and `aa2d901`; live branch `HEAD` is planning-document commit `5bf02b1ab72c281f564b3be67579aef7ab3be0d7`, live `main` has independently advanced through planning-only work to `7081ff3daf9d6e6b7f7af27783aa24163452ff06`, and their merge base remains `caeee9e73f2b3919934bcb70043491d33c200daa`. Branch-tip `1cdb31e`/`5bf02b1` and main `f77ccce`/`ad9137e`/`7081ff3` are planning artifacts, so the behavioral source baseline is unchanged and later main work is not silently incorporated. [VERIFIED: git log/rev-parse/merge-base during reset-loop revision two] Twelve tracked source files remain modified with 272 insertions and 83 deletions, and `crates/fava/tests/hostile_ingress.rs` remains untracked; the tracked source patch SHA-256 is `e7710b21f0fb81300ae136ec31d062c5aeff2e3b08b449b96ccdf4bb5e8b19c` and the untracked hostile-test blob is `7b9270a3c255a00a8a42e5d1d90294bd662e82ae`. `stash@{0}` remains the preserved autostash `5faecf42c0ec903507e3faeb04962f4680a9cb44`; no plan may apply, drop, rewrite, or supersede it. [VERIFIED: git status/diff/hash-object/stash during revision]

The first execution slice must reconcile the existing dirty WIP with committed `197c278`, because that commit's public `delivery_bounds.rs` names `RelayDeliveryOutcome::Unreachable` and `Receipt::spent`, while the corresponding definitions are present only in the current dirty tree. [VERIFIED: git show 197c278:crates/fava/tests/delivery_bounds.rs and git show 197c278:crates/fava-write/src/lib.rs this session; crates/fava-write/src/lib.rs:345-362,511-535] Planning from a clean-commit fiction would lose the live dependency that currently makes the regression pass. [VERIFIED: cargo test -p fava --test delivery_bounds this session]

**Primary recommendation:** replace `08-PLAN.md` with focused plans that first preserve and close the existing dirty delivery/hostile work, then complete the resource/provider boundary, then add all seven real-process canaries and run the full M8 exit gate. [VERIFIED: live inventory and remaining-slice analysis in this research]

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|---|---|---|---|
| NIP-42 challenge and authorization lifecycle | API / Backend (`fava-auth`) | Transport session owner | `fava-auth` owns relay-access identity, challenge, policy, signer, generation, and outcomes; the caller supplies the exact session. [VERIFIED: docs/spec/ARCHITECTURE.md:2116-2163] |
| Hostile frame admission | API / Backend (relay/query owner) | WebSocket transport | Transport enforces byte/frame bounds; the relay/query owner validates attribution, filter, generation, and terminal subscription state before cache mutation. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:794-803; crates/fava-transport-websocket/src/lib.rs:49-86; crates/fava/src/relay.rs:250-385] |
| NIP-11 acquisition and projection | API / Backend (`fava-nip11-http`, `fava-nip11`) | Subscription/publication planners | Acquisition remains separate from typed limits; planners own exact valid-plan/shortfall decisions. [VERIFIED: docs/spec/ARCHITECTURE.md:1843-1843; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:783-798] |
| Delivery attempts, budget, and ambiguity | Database / Storage (write store) | API / Backend (publication/delivery owners) | The store owns durable generation and receipt facts; policy uses spent real attempts, and transport supplies handoff evidence. [VERIFIED: crates/fava-write/src/lib.rs:345-375,511-535; .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30] |
| Provider execution and shutdown | API / Backend runtime boundary | Each universal owner | Provider work must execute outside owner locks/store transactions with bounded completion and shutdown influence. [VERIFIED: docs/spec/ARCHITECTURE.md:2201-2225] |
| Canary evidence and resource envelopes | External process / test lab | Public `fava` facade | Claims about sockets, persistence, and effects require separate processes plus independent witnesses. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-350; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855] |

## Project Constraints (from AGENTS.md)

- Treat `docs/spec/` as authoritative in the declared order; preserve behavior and ownership when illustrative names differ, and record a real contradiction before choosing. [VERIFIED: AGENTS.md:1-17]
- Use one focused local issue, branch, validation set, and commit series per slice. Write observable behavior, then failing executable evidence and its named deliberate break, then production code. Prove vertical behavior through public `fava`; do not stabilize an empty framework or claim unfinished behavior. [VERIFIED: AGENTS.md:30-38]
- Pass ownership, dependency direction, replaceability, failure isolation, boundedness, and behavioral-proof gates in proportion to scope. [VERIFIED: AGENTS.md:40-49]
- Architectural vocabulary is closed. New public/cross-crate nominal types, provider contracts, persisted entities, configuration concepts, owners, synonyms, and wrappers require a separate approved architecture change; run the vocabulary checker and its unit tests for public/API work. [VERIFIED: AGENTS.md:51-60]
- Code files have a 500-line soft and 800-line hard limit; use owner-local values, exact operation/generation identity, no hidden compatibility behavior, and an early contract/implementation split carrying its first real implementation. [VERIFIED: AGENTS.md:62-75]
- Do not add a remote or push without explicit authorization. [VERIFIED: AGENTS.md:38-38]

## Current-State Inventory

### Git and preservation boundary

| State | Exact inventory | Planning consequence |
|---|---|---|
| Branch/head | `milestone/m8-auth-hostile-limits` at planning-doc `5bf02b1ab72c281f564b3be67579aef7ab3be0d7`; live `main` at planning-doc `7081ff3daf9d6e6b7f7af27783aa24163452ff06`; merge base at `caeee9e73f2b3919934bcb70043491d33c200daa`. Branch-tip `1cdb31e`/`5bf02b1` and main-tip `f77ccce`/`ad9137e`/`7081ff3` are planning artifacts only; later main work is distinct. [VERIFIED: git branch/rev-parse/log/merge-base during reset-loop revision two] | Every plan starts from this exact branch/source truth, not the removed monolith's concurrency model or unmerged main changes. |
| Behavioral commits | `ed6a76c feat(m8): explicit generation-scoped NIP-42 relay authentication`; `94e04cd feat(m8): declared relay limits reach planning and publication`; `197c278 fix(m8): retry unreachable delivery without spending budget`. [VERIFIED: git log this session] | Do not re-plan these scripted owner/public implementations; plan only missing closure and evidence. |
| Debug commits | `729565b resolved debug record`; `aa2d901 debug knowledge-base entry`. [VERIFIED: git log this session] | Use the resolved causal record as evidence; it is not an M8 completion verdict. |
| Modified tracked files | `fava-delivery-standard`, `fava-publisher-nip01`, `fava-publisher`, `fava-transport-websocket` lib/conformance, memory write-store lib/lifecycle/semantic, redb semantic, neutral write-store receipt, `fava-write`, and `fava` relay. [VERIFIED: git status --short this session] | First plans must adopt these exact bytes and split by behavior without reverting or regenerating them. |
| Untracked | `crates/fava/tests/hostile_ingress.rs`. [VERIFIED: git status --short this session] | Treat as live WIP input, not a Wave 0 file to recreate. |
| Stash | `stash@{0}` = `5faecf42c0ec903507e3faeb04962f4680a9cb44`, pre-rebase autostash. [VERIFIED: git stash list this session] | Never apply/drop it; use only read-only inspection if a later executor needs ancestry evidence. |
| Patch hygiene | Dirty diff is 272 insertions/83 deletions over 12 tracked files; `git diff --check` is clean. [VERIFIED: git diff --stat and git diff --check this session] | Preserve patch bytes until the owning focused plan deliberately validates and commits them. |

### Live test observations

These commands passed against the combined committed-plus-dirty checkout, not a clean committed tree. [VERIFIED: commands executed this session]

| Command | Result | Interpretation |
|---|---:|---|
| `cargo test -p fava-auth --test authentication` | 6 passed | Owner-level authentication behavior is green in the live tree. [VERIFIED: command output this session] |
| `cargo test -p fava --test authentication` | 2 passed | Public scripted auth behavior is green, but not the required real-relay canaries. [VERIFIED: command output this session] |
| `cargo test -p fava-nip11 -p fava-nip11-http` | 8 passed | Typed parsing/acquisition behavior is green. [VERIFIED: command output this session] |
| `cargo test -p fava-subscriptions-standard --test relay_limits` | 5 passed | Planner-limit behavior is green. [VERIFIED: command output this session] |
| `cargo test -p fava --test relay_limits` | 4 passed | Public scripted limit behavior is green, not independent wire evidence. [VERIFIED: command output this session] |
| `cargo test -p fava --test delivery_bounds` | 4 passed | Offline, finite failure, and scripted ambiguity behavior is green only with dirty schema/outcome support. [VERIFIED: command output and git show comparison this session] |
| `cargo test -p fava --test hostile_ingress` | 2 passed | The untracked in-process hostile corpus is green; this is not a real-socket/separate-process proof. [VERIFIED: command output; crates/fava/tests/hostile_ingress.rs:66-137] |
| `cargo test -p fava-transport-websocket --test conformance` | 6 passed | Dirty transport size configuration is green. [VERIFIED: command output; crates/fava-transport-websocket/src/lib.rs:72-86] |

### Requirement disposition

| Requirement | Disposition | Committed truth | Dirty/WIP truth | Exact unfinished work |
|---|---|---|---|---|
| HARD-01 | PARTIAL | Auth owner, exact generation identity, NIP-42 frame construction, and public scripted read integration are committed. [VERIFIED: git show ed6a76c; crates/fava-auth/src/lib.rs:23-36,98-145,177-197,255-304] | None required for current owner tests. [VERIFIED: git status --short this session] | Run `nip42-write-and-reconnect` against a real third-party relay, restart/reconnect, prove a fresh challenge and persistent publish through public Fava, and publish independent process/wire evidence. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-814,850-855] |
| HARD-02 | PARTIAL | Policy-choice and scripted two-account isolation are committed. [VERIFIED: git show ed6a76c; features/relay-authentication.feature:18-30] | None required for current tests. [VERIFIED: git status --short this session] | Run `auth-account-isolation` through real sockets and prove one denial cannot delay or alter the other account. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:815-819] |
| HARD-03 | PARTIAL | Earlier admission/parsing and diagnostics paths exist. [VERIFIED: git show main:crates/fava/src/relay.rs inspected this session] | Dirty terminal subscription tracking, dirty WebSocket frame/message bounds, and untracked scripted hostile tests pass. [VERIFIED: crates/fava/src/relay.rs:250-385; crates/fava-transport-websocket/src/lib.rs:72-86; crates/fava/tests/hostile_ingress.rs:157-250] | Add real-socket/separate-process invalid id/signature, off-filter, malformed, oversized, stale-generation, post-CLOSED, never-EOSE, mid-frame truncation, silent-limit, NOTICE, and disconnect scenarios with scoped diagnostics and a healthy concurrent witness. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:794-803,821-825,850-859] |
| HARD-04 | PARTIAL | NIP-11 values/acquisition and subscription/publication refusal are committed. [VERIFIED: git show 94e04cd; features/relay-limits.feature:1-42] | No relevant dirty files. [VERIFIED: git status --short this session] | Wire real NIP-11 HTTP from the lab relay, independently prove no invalid REQ/EVENT crosses the wire, run exact shortfall against Khatru, and keep M9 fetch caching out of scope. [VERIFIED: apps/canary/relays/khatru/main.go:20-86; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:827-831] |
| HARD-05 | PARTIAL | `197c278` fixes delayed retry/generation/Redb transition and records causal mutation evidence. [VERIFIED: .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30,39-45,85-99] | Dirty public outcome/store schema separates `attempts` from `spent_attempts`; combined test passes. [VERIFIED: crates/fava-write/src/lib.rs:345-362,511-535; cargo test -p fava --test delivery_bounds this session] | Make the commit series self-contained, prove memory/Redb parity and real process restart, then run `attempt-ceiling` with an offline interval followed by an actual attempt. [VERIFIED: current git show comparison; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:839-843] |
| HARD-06 | PARTIAL | The committed policy/public regression establishes the intended finite path. [VERIFIED: git show 197c278] | Dirty `Unreachable`, `spent_attempts`, and retry policy wiring currently make the public test compile/pass. [VERIFIED: crates/fava-delivery-standard/src/lib.rs:23-63; crates/fava-write/src/lib.rs:345-362,511-535] | Prove repeated real relay failures reach terminal `GivenUp` within the configured ceiling through public Fava, with resource/time envelope and named break. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:839-843] |
| HARD-07 | PARTIAL | The public scripted test is committed inside `197c278`. [VERIFIED: git show 197c278:crates/fava/tests/delivery_bounds.rs] | Its current compilation depends on dirty receipt/outcome definitions. [VERIFIED: git show comparison and cargo test this session] | Use a transparent proxy to witness the full EVENT crossing, cut before OK, assert exact durable ambiguity through public receipt, kill/reopen, and prove it is never rewritten. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:833-837; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-350] |
| HARD-08 | PARTIAL | Several pre-M8 owners already have declared bounds, and committed auth/NIP-11 work adds bounded challenge/acquisition/plan surfaces. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376; crates/fava-auth/src/lib.rs:23-36] | Dirty WebSocket inbound bounds and delivery budget separation pass focused tests. [VERIFIED: crates/fava-transport-websocket/src/lib.rs:72-86; crates/fava-write/src/lib.rs:511-535] | Produce an exhaustive owner/resource ledger for all OPS-004 categories, close active-session/wire-subscription/provider/diagnostic/artifact gaps with typed refusal/backpressure/shortfall, exceed every new limit, and publish measured envelopes. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376] |
| HARD-09 | ABSENT | No M8 provider-failure slice is committed. [VERIFIED: git log and repository-wide rg inspection this session] | No matching dirty file or test was found. [VERIFIED: git status and repository-wide rg inspection this session] | Add causal public tests for panic, block, late, malformed, and cancellation-ignore; execute provider calls outside owner locks/store transactions; prove unrelated query/relay/write progress and bounded shutdown. [VERIFIED: docs/spec/ARCHITECTURE.md:2201-2225; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:845-848] |
| HARD-10 | ABSENT | The Khatru relay fixture is committed, but no M8 scenario is registered or dispatched. [VERIFIED: apps/canary/relays/khatru/main.go:1-100; apps/canary/scenarios.json:112-166; apps/canary/src/lib.rs:146-174] | No M8 canary WIP exists in the dirty inventory. [VERIFIED: git status --short this session] | Implement and enable all seven named M8 scenarios; real sockets and separate processes; NIP-42 plus persistence on one real third-party relay; core read/publish on Khatru; resource and failure bundles every run. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-855] |

## Stale Assumptions in `08-PLAN.md`

| Stale assumption | Live correction | Planner action |
|---|---|---|
| `depends_on: [M3, M6]`; M7 is “concurrent” and not a dependency. [VERIFIED: .planning/phases/08-authentication-hostile-boundaries-and-boundedness/08-PLAN.md:1-21] | The current roadmap says Phase 08 “Depends on: Phase 7,” and Phase 07 passed 12/12; its stable receipt/generation behavior is now upstream truth. [VERIFIED: .planning/ROADMAP.md:150-161; .planning/phases/07-semantic-writes-and-capability-composition/07-VERIFICATION.md:1-24,45-60] | Set plan dependency to Phase 07 and preserve M7's exact current-generation and durable receipt semantics. |
| Six slices are described as future work. [VERIFIED: 08-PLAN.md:23-93] | Three behavioral commits and substantial dirty WIP already exist. [VERIFIED: git log/status this session] | Replace the monolith with remaining-only slices; do not recreate auth/NIP-11/scripted delivery work. |
| Numeric bounds `512 KiB`, `256` sessions, and `256` diagnostics are presented as introduced. [VERIFIED: 08-PLAN.md:95-105] | Only the auth values and dirty WebSocket configuration were found in live source; no session-pool or per-category diagnostic implementation was found. [VERIFIED: crates/fava-auth/src/lib.rs:23-36; crates/fava-transport-websocket/src/lib.rs:72-86; repository-wide rg inspection this session] | Treat unimplemented table values as stale proposals, not locked facts. Derive/approve each missing bound from its owner, workload profile, typed refusal, and exceeding-limit test. |
| Authentication and relay-limit feature scenarios cite enabled canaries. [VERIFIED: features/relay-authentication.feature:3-23; features/relay-limits.feature:3-15] | `nip42-write-and-reconnect`, `auth-account-isolation`, and `relay-limit-shortfall` are absent from registry and dispatch. [VERIFIED: apps/canary/scenarios.json:112-166; apps/canary/src/lib.rs:146-174; apps/canary/src/main.rs:66-124] | Until executors pass, mark those scenario claims as specified or remove the nonexistent canary evidence; restore `built` only with executable mapping. |
| The Khatru fixture is described as usable. [VERIFIED: 08-PLAN.md:84-90] | It is not integrated, and its checked-in module requires Go `1.25.0` while the machine has Go `1.23.3` with `GOTOOLCHAIN=local`. [VERIFIED: apps/canary/relays/khatru/go.mod:3-8; go version/go env/go list this session] | Add a Wave 0 Go 1.25 toolchain prerequisite before building or running the second relay. |

## Recommended Remaining Slices

These are planning boundaries, not implementation changes.

### Slice A — Reconcile the live delivery lifecycle WIP (`HARD-05`, `HARD-06`, `HARD-07`)

Own the exact dirty publisher/outcome/write-store/write/publication seams that make `197c278` self-contained. Preserve separate monotonic generation and spent attempt budget, Memory/Redb replaceability, ambiguity, and restart truth. Begin with the existing public regression plus clean-base compilation evidence; add missing Redb/memory/reopen coverage and a deliberate break that reconflates generation with spent budget. [VERIFIED: crates/fava-write/src/lib.rs:345-362,511-535; .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30]

### Slice B — Reconcile hostile ingress and inbound wire bounds (`HARD-03`, part of `HARD-08`)

Own the dirty WebSocket and relay terminal-state changes plus the untracked hostile test. First commit accurate behavior text and turn each missing hostile class into a causal owner/public test; then add a separate adversarial relay process and transparent proxy for oversized, stale, truncated, silent-limit, disconnect, and post-CLOSED schedules. The named falsifier must bypass admission into cache mutation and make the hostile public scenario fail. [VERIFIED: crates/fava-transport-websocket/src/lib.rs:72-86; crates/fava/tests/hostile_ingress.rs:157-250; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:821-825,850-859]

### Slice C — Complete the OPS-004 resource ledger and provider isolation (`HARD-08`, `HARD-09`)

Inventory every owner against the exact quoted categories: “query structure and derived values,” “router contributions and route fan-out,” “active relay sessions,” “wire subscriptions,” “frame and message sizes,” “event-cache memory where bounded,” “write-store active work and retained receipts,” “provider operations,” “observation delivery,” “diagnostics,” “fetched service entries,” and “platform bridge queues.” [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1374] For each gap, assign one owner and one typed refusal/backpressure/shortfall plus an exceed-limit test. Implement the specified neutral `fava-runtime` contract with its first real provider and the FavaBuilder-before-standard selection surface; then one centralized graph slice must register every Runtime/standard/consumer Cargo and Bazel edge, update MODULE metadata, regenerate Cargo.lock once, and compile all affected crates in their pre-migration state. Only after that compile may owner tasks add and commit exact compiled behavioral RED tests and migrate source-observation polling, facade relay/transport sessions, publisher futures, signer and provider work, timers, cancellation, joins, panic isolation, and shutdown deadlines. Universal owners retain authorization and accept only exact operation/generation-correlated typed completions; repository-wide falsifiers prohibit parallel owner-local execution helpers outside runtime/provider implementation and explicitly approved harness code. [VERIFIED: AGENTS.md:40-57,68-75; docs/spec/ARCHITECTURE.md:2201-2225,2837,3476]

### Slice D — Wire authentication and relay-limit real-process evidence (`HARD-01`, `HARD-02`, `HARD-04`)

Register and dispatch `nip42-write-and-reconnect`, `auth-account-isolation`, and `relay-limit-shortfall`. Use a real third-party relay process for NIP-42 plus persistence/restart, integrate Khatru as the second implementation, and use a proxy/relay log as the independent witness that knowingly invalid work sent no bytes. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-831,850-855; apps/canary/relays/khatru/main.go:20-93]

### Slice E — Wire delivery/provider canaries and final M8 exit (`HARD-05` through `HARD-10`)

Register and dispatch `ambiguous-handoff`, `attempt-ceiling`, `provider-failure-isolation`, and `hostile-relay-ingress`. Each run must emit a bounded resource envelope and exact failure evidence, and final validation must execute all seven M8 scenarios, both relay implementations, every named deliberate break, Cargo/Bazel/vocabulary gates, and the hostile-admission falsifier. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:821-859]

## Standard Stack

### Core

Use the checked-in external dependency stack. Add only the authoritative internal `fava-runtime` contract, its first approved concrete provider, and the specified `fava-standard` assembly; register all three in Cargo/Bazel and regenerate the root lock once. [VERIFIED: docs/spec/ARCHITECTURE.md:2201-2306,2837,3476; AGENTS.md:68-75]

| Library/tool | Verified version | Purpose | Why standard here |
|---|---:|---|---|
| Rust workspace | `rust-version = "1.90"`, edition `"2024"` | Production and test code | Repository-owned toolchain contract. [VERIFIED: Cargo.toml:45-48] |
| `tokio` | `1.53.1` | async scheduling, controlled deadlines, process orchestration | Already pinned workspace runtime. [VERIFIED: Cargo.lock:1439-1444] |
| `tokio-tungstenite` / `tungstenite` | `0.30.0` / `0.30.0` | WebSocket transport with configured message/frame limits | Existing transport stack; `WebSocketConfig` owns the complete-message and frame limits. [VERIFIED: Cargo.lock:1475-1496; crates/fava-transport-websocket/src/lib.rs:72-86] |
| `nostr` | `0.45.3` | Nostr event/signature values and NIP-42 event construction | Existing pinned protocol implementation; do not hand-roll signature verification. [VERIFIED: Cargo.lock:1035-1040; crates/fava-auth/src/lib.rs:177-197] |
| `redb` | `4.2.0` | durable receipt/restart and transition parity | Existing persistent write-store implementation. [VERIFIED: Cargo.lock:1190-1195] |
| `serde` / `serde_json` | `1.0.229` / `1.0.151` | bounded typed wire/service parsing | Existing pinned serialization stack. [VERIFIED: Cargo.lock:1267-1272,1297-1302] |
| `nostr-rs-relay` | `0.8.12` installed | primary real third-party relay process | Existing lab dependency and required separate-process target. [VERIFIED: nostr-rs-relay --version this session; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855] |
| Khatru fixture | `khatru v0.19.1`, `eventstore v0.17.12`, `go-nostr v0.52.3`; Go module minimum `1.25.0` | second relay implementation and strict NIP-11/auth fixture | Already committed and pinned; executor must upgrade the local Go toolchain before use. [VERIFIED: apps/canary/relays/khatru/go.mod:3-8; apps/canary/relays/khatru/go.sum:33-54] |

### External protocol rules

- NIP-42 uses relay `AUTH` challenge, client `AUTH` signed event, kind `22242`, exact `relay` and `challenge` tags, and relay `OK`; challenges/answers are connection-scoped. [CITED: https://github.com/nostr-protocol/nips/blob/master/42.md]
- NIP-11 limitation fields are optional; omitted claims remain unknown. Relevant fields include message/subscription/subscription-id/limit/tag/content/PoW/auth/payment/created-at constraints. [CITED: https://github.com/nostr-protocol/nips/blob/master/11.md]
- Tungstenite exposes distinct `max_message_size` and `max_frame_size`; configure both when the transport contract bounds complete inbound messages and individual frames. [CITED: https://docs.rs/tungstenite/0.30.0/tungstenite/protocol/struct.WebSocketConfig.html]

### Alternatives Considered

| Instead of | Could use | Tradeoff |
|---|---|---|
| Existing pinned Rust stack | New WebSocket/auth/limit crates | Rejected: this phase must close live owner behavior and cannot add compatibility paths or unnecessary vocabulary/dependencies. [VERIFIED: AGENTS.md:1-17,51-60] |
| Existing relay lab plus Khatru | A custom in-process-only mock | Rejected for exit evidence: scripted relays are useful for causal schedules but cannot prove real sockets, process restart, third-party interoperability, or persistence. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-350; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855] |

**Installation:** no new package installation is recommended. Upgrade/provision Go 1.25 for the already checked-in Khatru module, then run `go mod verify` and `go test ./...` in `apps/canary/relays/khatru`. [VERIFIED: apps/canary/relays/khatru/go.mod:3-8; go toolchain probe this session]

## Package Legitimacy Audit

No new external package is proposed. Rust packages are already pinned with registry checksums in `Cargo.lock`; Khatru dependencies are already pinned with module checksums in `go.mod`/`go.sum`. [VERIFIED: Cargo.lock:1035-1496; apps/canary/relays/khatru/go.mod:3-8; apps/canary/relays/khatru/go.sum:33-54] The GSD legitimacy seam has no Go ecosystem mode, so this research does not upgrade the existing Go modules to `[VERIFIED: npm registry]`-style legitimacy; the executable gate is checked-in identity plus `go mod verify`, build, and real-process behavior. [VERIFIED: package-legitimacy seam help/protocol available to this session]

## Architecture Patterns

### System Architecture Diagram

```text
App public Fava call
        |
        v
accepted query/write + exact RelaySessionKey/generation
        |
        +--> NIP-11 acquisition --> typed projection --> planner/publisher
        |                                      |             |
        |                                valid plan      exact shortfall
        |
        +--> transport open --> bounded WebSocket --> relay process
                                  |                    |
                                  |                    +--> AUTH challenge
                                  |                    +--> hostile/limit/outcome frames
                                  v
                         admission + attribution
                          /        |          \
                     accepted   scoped fault   ambiguous handoff
                        |           |                 |
                        v           v                 v
                    cache/query  diagnostics    durable write receipt
                                                       |
                                                       v
                                           bounded delivery policy
                                           /         |          \
                                      wait offline  retry real  give up

Provider calls --> isolated execution boundary --> exact-generation completion
       |                    |
       |                    +--> timeout/panic/malformed/cancel-ignore => scoped outcome
       +--------------------------------------------------------------> unrelated work continues

Canary parent process --> relay/adversary/proxy child processes --> evidence bundle
```

The flow and ownership are prescribed by the M8 required behavior, architecture auth/runtime owners, and process-proof rules. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:792-859; docs/spec/ARCHITECTURE.md:2116-2225; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-350]

### Recommended Project Structure

Keep behavior in existing owners and add evidence alongside the owning crate/public facade. Do not create a generic common bucket. [VERIFIED: AGENTS.md:68-75]

```text
crates/
├── fava-auth/                         # exact NIP-42 lifecycle owner
├── fava-nip11{,-http}/                # typed relay information and acquisition
├── fava-transport-websocket/          # frame/message boundary
├── fava-publication/                  # publication orchestration
├── fava-write{,-store-*}/             # durable delivery truth
└── fava/tests/                         # public-Fava capstones
apps/canary/
├── relays/khatru/                      # second third-party relay fixture
├── src/                                # seven M8 executors and evidence packaging
└── scenarios.json                     # enabled only when executor exists and passes
features/                               # accurate behavior status/evidence mapping
```

### Pattern 1: Separate generation identity from attempt budget

**What:** Keep monotonic attempt generation in `attempts`; count only actual relay-reaching work in `spent_attempts`; treat `Unreachable` as an open, non-spending state. [VERIFIED: crates/fava-write/src/lib.rs:345-362,511-535]

**When to use:** Every retry, store transition, stale completion, and restart path. [VERIFIED: .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30]

```rust
// Source: crates/fava-write/src/lib.rs:354-362,511-535
Unreachable { reason: String }
pub attempts: BTreeMap<RelaySessionKey, u32>,
pub spent_attempts: BTreeMap<RelaySessionKey, u32>,
```

The discrete values quoted above are verbatim from the live dirty source. [VERIFIED: crates/fava-write/src/lib.rs:354-362,511-535]

### Pattern 2: Bound before parsing/ownership handoff

**What:** Configure the WebSocket library for both complete-message and individual-frame bounds, then retain owner-level admission checks for attribution, generation, filter, and terminal subscription state. [VERIFIED: crates/fava-transport-websocket/src/lib.rs:72-86; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:794-803]

```rust
// Source: crates/fava-transport-websocket/src/lib.rs:72-75
let config = WebSocketConfig::default()
    .max_message_size(Some(self.max_frame_bytes.get()))
    .max_frame_size(Some(self.max_frame_bytes.get()));
```

### Pattern 3: Causal process proof with an independent witness

**What:** Parent canary controls process/port/proxy gates; the public Fava client performs the use case; proxy transcript, relay log, filesystem/process state, and public output jointly prove the claim. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-350]

**When to use:** Auth reconnect, no-wire shortfall, ambiguous handoff, process restart, hostile ingress, bounded shutdown, and resource envelope claims. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-855]

### Anti-Patterns to Avoid

- **Counting elapsed offline time as failed attempts:** it destroys the generation/budget distinction and can terminalize work without a relay attempt. [VERIFIED: .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30,39-45]
- **Calling a provider under a lock or store transaction:** one block/panic can hold unrelated owner progress and shutdown. [VERIFIED: docs/spec/ARCHITECTURE.md:2223-2225]
- **Treating a `built` feature comment as executable evidence:** three current feature scenarios cite nonexistent canary executors. [VERIFIED: features/relay-authentication.feature:3-23; features/relay-limits.feature:3-15; apps/canary/src/lib.rs:146-174]
- **Using sleeps as a liveness/order proof:** use controlled clocks, barriers, channels, proxy gates, or witness signals. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-325]
- **Inventing defaults for omitted NIP-11 fields:** absence is unknown, not permission or refusal. [CITED: https://github.com/nostr-protocol/nips/blob/master/11.md]
- **Claiming process durability by opening twice in one process:** runtime state must be destroyed and reopened through the supported construction path. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-338]

## Don't Hand-Roll

| Problem | Don't build | Use instead | Why |
|---|---|---|---|
| NIP-42 event crypto | Custom signature/event verifier | Existing `nostr` types plus `fava-auth` owner | Exact kind/tag/signature semantics and generation correlation already exist. [VERIFIED: Cargo.lock:1035-1040; crates/fava-auth/src/lib.rs:177-197,255-304] |
| WebSocket size enforcement | Post-hoc string-length-only guard | Tungstenite `WebSocketConfig` plus owner admission | Library bounds allocations/frames before owner parsing; owner still supplies semantic refusal. [CITED: https://docs.rs/tungstenite/0.30.0/tungstenite/protocol/struct.WebSocketConfig.html] |
| Retry ledger | Timer-local counters | Durable receipt generation and spent-attempt maps | Late completion, restart, ambiguity, and provider parity require one durable owner. [VERIFIED: crates/fava-write/src/lib.rs:511-535; .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30] |
| Real-process witness | In-process fake claiming socket/process truth | Existing canary supervisor/evidence model, real relays, child adversary, transparent proxy | Diagnostics cannot prove their own external-effect claim. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:340-350] |
| Provider execution isolation | Parallel owner-local spawn/timeout/panic/join helpers or an empty abstraction | The specified neutral `fava-runtime` contract plus its first approved concrete implementation; owners carry only authorization and typed correlated completions | Architecture assigns execution resources, provider isolation, cancellation, timers, and shutdown joins to `fava-runtime`; AGENTS requires the real contract/implementation split and forbids a private bypass. [VERIFIED: docs/spec/ARCHITECTURE.md:2201-2225,2837,3476; AGENTS.md:75-75] |

## Common Pitfalls

### Pitfall 1: Treating green combined-WIP tests as committed completion

**What goes wrong:** `197c278` appears complete, but its public test currently references discrete values defined only in dirty files. [VERIFIED: git show comparison this session; crates/fava-write/src/lib.rs:345-362,511-535]

**How to avoid:** the first plan records clean-base compile/failure truth, preserves the dirty patch, makes the focused slice self-contained, and reruns the exact test plus provider parity before any broader work. [VERIFIED: current repository state analysis]

### Pitfall 2: Proving protocol behavior only with scripted transports

**What goes wrong:** scripted tests can control schedules but do not prove WebSocket framing, process isolation, third-party relay behavior, persistence, or proxy-observed handoff. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-350; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855]

**How to avoid:** keep scripted owner tests for causality, then add public real-process capstones with independent witnesses. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:300-350]

### Pitfall 3: A partial boundedness checklist

**What goes wrong:** frame size and retry ceiling pass while session pools, wire subscriptions, provider work, diagnostics, fetched services, or evidence artifacts remain unbounded/silent. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376]

**How to avoid:** require a ledger row and exceed-limit test for every quoted OPS-004 category; absence of a row blocks HARD-08. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376]

### Pitfall 4: Allowing stale connection work to authenticate or mutate current state

**What goes wrong:** comparing relay identity without generation lets a retired AUTH answer/frame complete current work. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1060-1076; features/relay-authentication.feature:32-40]

**How to avoid:** exact session key plus generation at challenge, handoff, completion, admission, reconnect, and store mutation boundaries. [VERIFIED: crates/fava-auth/src/lib.rs:255-304; AGENTS.md:73-73]

### Pitfall 5: Letting the canary registry outrun executors

**What goes wrong:** feature mapping says `built` even though the named executor is absent, creating false milestone evidence. [VERIFIED: features/relay-authentication.feature:3-23; features/relay-limits.feature:3-15; apps/canary/src/lib.rs:146-174]

**How to avoid:** a validation test must require every enabled/built canary ID to exist in the registry, `has_executor`, CLI dispatch, and a passing evidence bundle. [VERIFIED: current mapping gap]

## State of the Art

| Old/stale approach | Current required approach | Impact |
|---|---|---|
| M8 planned independently of concurrent M7. [VERIFIED: 08-PLAN.md:20-21] | Phase 07 is complete and an explicit Phase 08 dependency. [VERIFIED: .planning/ROADMAP.md:150-155; Phase 07 verification:1-24] | Plan against stable M7 receipt/generation semantics; do not reconstruct pre-M7 APIs. |
| One monolithic six-slice plan. [VERIFIED: 08-PLAN.md:23-93] | Remaining-only focused slices starting from three commits and dirty WIP. [VERIFIED: git log/status this session] | Prevent duplicate implementation and preserve debugged work. |
| WebSocket text checked after receipt. [VERIFIED: pre-dirty git diff inspected this session] | Configure both complete-message and frame limits before parsing, then retain semantic admission. [VERIFIED: crates/fava-transport-websocket/src/lib.rs:72-86] | Bounds transport allocation and owner mutation separately. |
| Attempt generation inferred from spent budget. [VERIFIED: .planning/debug/resolved/m8-unreachable-delivery-retry.md:21-30] | Durable generation and actual spent budget are distinct. [VERIFIED: crates/fava-write/src/lib.rs:511-535] | Offline retry remains open while stale completions remain exactly rejectable. |

## Assumptions Log

All implementation-status claims in this research were checked against live source, history, status, tests, or authoritative documents. No `[ASSUMED]` claim is used to lock a planning decision.

## Open Questions (RESOLVED)

1. **Which exact numeric values own the missing session-pool, diagnostic, provider, shutdown, and artifact bounds? — RESOLVED**
   - Decision: keep every existing source-owned bound unchanged; `Diagnostics::default()` remains 256 facts per category and the checked-in process-output cap remains 1,048,576 bytes. Close the missing rows with owner-private reversible constants: 256 active relay sessions per Fava instance; 64 in-flight runtime provider operations and 64 command/completion slots per runtime instance; a 5-second provider-operation deadline and 5-second shutdown-join deadline; 1,048,576 bytes per canary evidence stream and 8,388,608 bytes per complete run. Exceeding any row returns an existing typed refusal/shortfall or a scoped runtime outcome; no silent truncation counts. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1359-1376; crates/fava-diagnostics/src/lib.rs:76-85; apps/canary/src/semantic_process.rs:13,241-258]
   - Approval path: these are private owner/profile policy values, not vocabulary or persisted schema. The owning tests and OPS-004 ledger approve them through causal boundary/max+1 evidence; no human architecture checkpoint is required. Any executor that needs a new public/cross-crate noun or configuration surface must stop for the separate runtime architecture checkpoint rather than widening this decision.

2. **Does the already-added public authentication vocabulary require a new approval record? — RESOLVED**
   - Decision: no new approval is required for `AuthorizationDecision` or `RelayChallenge`. Both are exact specified public contracts in authoritative `ARCHITECTURE.md` and are registered as `spec_symbols` under the existing Authentication vocabulary; commits ed6a76c/94e04cd implement specified vocabulary rather than proposing a feature-owned synonym. The canary plan verifies the registry and implementation but contains no ratification/correction branch. [VERIFIED: docs/spec/ARCHITECTURE.md:2116-2163; docs/internals/vocabulary.toml:722-759; AGENTS.md authority order]
   - Boundary: any symbol or crate not already specified/registered remains a vocabulary change and requires its own blocking architecture decision before dependent implementation.

3. **Which installed relay is the NIP-42-plus-persistence witness? — RESOLVED**
   - Decision: `nostr-rs-relay` 0.8.12 is the primary NIP-42 plus persistent-restart witness, using its generated isolated SQLite directory and `authorization.nip42_auth = true`; the canary must observe the challenge/AUTH/write/restart sequence and durable database/log/public receipt evidence. Khatru remains the second implementation for core read/publish and real NIP-11 limits only; its in-memory store is never cited as persistence evidence. [VERIFIED: apps/canary/README.md:8-12; apps/canary/src/relay.rs:74,149-193,222-223; apps/canary/relays/khatru/main.go:27-64; docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855]

## Environment Availability

| Dependency | Required by | Available | Version | Fallback/action |
|---|---|---:|---|---|
| Rust/Cargo | all owner/public tests | ✓ | `rustc 1.90.0`, `cargo 1.90.0` | — [VERIFIED: rustc/cargo version this session] |
| Bazel | registered cross-build exit gate | ✓ | `9.2.0` | — [VERIFIED: bazel --version this session] |
| `nostr-rs-relay` | primary real relay | ✓ | `0.8.12` | Configure and prove NIP-42/persistence in scenario. [VERIFIED: nostr-rs-relay --version this session] |
| Go | Khatru second relay | ✗ wrong version | installed `1.23.3`; module requires `1.25.0`; `GOTOOLCHAIN=local` | Blocking: provision Go 1.25 or change environment policy before build. [VERIFIED: apps/canary/relays/khatru/go.mod:3-3; go version/go env/go list this session] |
| Python | vocabulary checks | ✓ | `3.14.6` | — [VERIFIED: python3 --version this session] |
| Git | preservation/history checks | ✓ | `2.50.1 (Apple Git-155)` | — [VERIFIED: git --version this session] |
| `timeout` | bounded canary/process gates | ✓ | `/opt/homebrew/bin/timeout` | Tokio/process deadlines remain the behavioral proof; shell timeout is a harness failsafe. [VERIFIED: command -v timeout this session] |

**Missing dependency with no current fallback:** Go 1.25 for the checked-in Khatru module; local Go refuses the module with `go.mod requires go >= 1.25.0`. [VERIFIED: go list invocation this session]

## Validation Architecture

The repository has Nyquist validation and security enforcement enabled. [VERIFIED: .planning/config.json:20-49]

### Test Framework

| Property | Value |
|---|---|
| Framework | Rust built-in test harness via Cargo 1.90; Tokio tests; Python `unittest` for vocabulary/feature gates; canary CLI for real processes. [VERIFIED: current test files and tool versions this session] |
| Config | Workspace `Cargo.toml`, `Cargo.lock`, Bazel BUILD files, `apps/canary/Cargo.toml`, `apps/canary/scenarios.json`. [VERIFIED: repository file inspection this session] |
| Fast owner/public run | The eight focused commands below; each passed in the live combined tree this session. [VERIFIED: command outputs this session] |
| Full suite | `cargo test --workspace --all-targets`; `cargo clippy --workspace --all-targets -- -D warnings`; canary fmt/check/test/clippy; `bazel test //...`; vocabulary and feature-evidence gates. [VERIFIED: .planning/phases/07-semantic-writes-and-capability-composition/07-VALIDATION.md:136-142; AGENTS.md:60-60] |

### Fast feedback commands

Run the smallest relevant set after every task; these are the current executable commands. [VERIFIED: commands executed this session]

```bash
cargo test -p fava-auth --test authentication
cargo test -p fava --test authentication
cargo test -p fava-nip11 -p fava-nip11-http
cargo test -p fava-subscriptions-standard --test relay_limits
cargo test -p fava --test relay_limits
cargo test -p fava --test delivery_bounds
cargo test -p fava --test hostile_ingress
cargo test -p fava-transport-websocket --test conformance
```

After adding each new owner-level test, run its exact test name with `-- --exact` before the package/public bundle. New evidence must first fail causally, then pass after implementation, then fail under the named mechanism-disable break. [VERIFIED: AGENTS.md:32-36; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:415-432]

### Requirements → Test Map

| Req | Required public behavior test | Hostile/boundary falsifier | Process/network/restart gate | Current file state |
|---|---|---|---|---|
| HARD-01 | Existing auth unit/public tests plus `nip42-write-and-reconnect` | Remove generation comparison or reuse old challenge; retired challenge test and reconnect canary must fail. [VERIFIED: features/relay-authentication.feature:32-40] | Real relay, write, kill/restart/reconnect, fresh AUTH, public receipt, relay log/persistence witness. [VERIFIED: M8 plan:807-814,850-855] | Unit/public files exist; canary absent. [VERIFIED: cargo tests and canary registry this session] |
| HARD-02 | Existing two-account public test plus `auth-account-isolation` | Ignore policy denial or share auth state; denied account incorrectly succeeds or authorized account stalls. [VERIFIED: features/relay-authentication.feature:18-30] | Two exact relay accesses over real sockets; deny one; bounded completion for the other with relay-auth identity witness. [VERIFIED: M8 plan:815-819] | Public test exists; canary absent. [VERIFIED: feature/canary inspection this session] |
| HARD-03 | Commit/extend `hostile_ingress.rs` into a complete hostile corpus | Route malformed input directly to cache mutation; hostile scenario must expose the bad event/fail. [VERIFIED: M8 plan:857-859] | Separate adversary process and healthy relay; proxy-controlled malformed/oversized/truncated/stale/post-CLOSED/silent/disconnect schedules. [VERIFIED: M8 plan:821-825,850-852] | Scripted file untracked; process test absent. [VERIFIED: git status/canary inspection this session] |
| HARD-04 | Existing NIP-11 owner/public tests plus `relay-limit-shortfall` | Ignore relay claim; over-limit plan/publish crosses wire and no-wire assertion fails. [VERIFIED: features/relay-limits.feature:3-15] | Real HTTP NIP-11 from Khatru, proxy/relay witness of zero invalid REQ/EVENT, exact public shortfall. [VERIFIED: M8 plan:827-831] | Unit/public files exist; canary absent. [VERIFIED: cargo tests/canary inspection this session] |
| HARD-05 | `offline_time_spends_no_attempt_budget_and_the_write_stays_open` plus memory/Redb reopen parity | Count `Unreachable` as spent or derive generation from spent budget; focused test/restart must fail. [VERIFIED: resolved debug:21-30,39-45] | Start offline, cross multiple retry intervals, start relay, actual attempt occurs, kill/reopen same receipt. [VERIFIED: M8 plan:839-843; TDD guide:327-338] | Public test committed but depends on dirty schema/outcome; reopen capstone absent. [VERIFIED: git show comparison this session] |
| HARD-06 | Finite retry/give-up public test | Remove/raise ceiling or stop incrementing spent real attempts; terminal assertion/resource ceiling fails. [VERIFIED: crates/fava-delivery-standard/src/lib.rs:47-63] | Real relay produces controlled retryable failures; public receipt reaches `GivenUp` within declared attempt/time/resource envelope. [VERIFIED: M8 plan:839-843] | Scripted test exists; process canary absent. [VERIFIED: cargo/canary inspection this session] |
| HARD-07 | Scripted ambiguity plus durable receipt reopen | Convert post-handoff disconnect to retryable, acknowledged, rejected, or never-sent; exact ambiguity assertion fails. [VERIFIED: M8 plan:833-837] | Transparent proxy witnesses complete EVENT then drops relay OK; kill/reopen; same exact ambiguity remains public. [VERIFIED: M8 plan:833-837; TDD guide:327-350] | Scripted test exists; proxy/restart canary absent. [VERIFIED: delivery test/canary inspection this session] |
| HARD-08 | One exceed-limit test for every OPS-004 ledger row | Remove each bound/refusal/backpressure path individually; the matching explicit shortfall/high-water assertion fails. [VERIFIED: GOALS:1359-1376] | Load/process run publishes peak RSS/FD/tasks/queues/subscriptions/diagnostics/artifact sizes and exact refusal/loss counters. [VERIFIED: M8 plan:850-855] | Partial tests only; ledger/envelope absent. [VERIFIED: repository inspection this session] |
| HARD-09 | Provider conformance/public isolation tests for panic/block/late/malformed/cancel-ignore | Run provider under owner lock, omit generation check, or wait unboundedly at shutdown; unrelated-progress/deadline assertion fails. [VERIFIED: ARCHITECTURE:2201-2225] | Provider child/task controlled by barriers; independent query/relay/write completes; shutdown joins/refuses within declared bound. [VERIFIED: M8 plan:845-848] | Wave 0: tests and implementation absent. [VERIFIED: repository-wide rg inspection this session] |
| HARD-10 | Registry/executor/evidence-schema tests for all seven M8 IDs | Remove executor/evidence artifact, run adversary in-process, or omit resource/failure bundle; mapping/bundle validation fails. [VERIFIED: M8 plan:805-855] | Seven CLI scenarios, real sockets, separate processes, primary persistent NIP-42 relay, second relay core subset, resource/failure evidence every run. [VERIFIED: M8 plan:850-855] | Wave 0: M8 registry/executors absent; Khatru fixture exists but Go toolchain blocks build. [VERIFIED: canary files and environment probe this session] |

### Controlled schedules and deliberate breaks

- Use barriers/channels/proxy gates for reconnect, stale completion, cancellation around handoff, provider stall, and process kill. Sleeps may enforce an outer deadline but cannot prove ordering or liveness. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:313-325]
- Every plan names the exact production seam temporarily disabled and the exact test expected to fail; retain the before/failing, after/passing, and deliberate-break/failing command/output in the focused issue or validation artifact. [VERIFIED: AGENTS.md:32-36; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:415-432]
- External-effect claims require a witness not owned by Fava diagnostics: proxy transcript, relay process log, filesystem/database state, PID/port state, or resource sampler. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:340-350]
- Restart proof must kill/stop the process, destroy runtime state, reopen through the supported construction path, observe the public result, and continue the operation. [VERIFIED: docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:327-338]

### Sampling Rate

- **Per task commit:** exact new test with `-- --exact`, then the smallest affected command from the fast-feedback list. [VERIFIED: repository TDD rules]
- **Per plan/wave:** all eight fast-feedback commands plus affected provider/store tests, `git diff --check`, vocabulary checks for public/API changes, and the plan's real-process scenario. [VERIFIED: AGENTS.md:32-36,51-60]
- **Per phase gate:** full Cargo/Bazel/lint checks, all seven enabled M8 canaries against required relay implementations, every deliberate break, hostile-admission falsifier, feature-to-executor mapping, and evidence-bundle/resource validation. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-859; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:415-432]

### Wave 0 Gaps

- [ ] Make `197c278` plus dirty delivery/outcome/store code a self-contained testable slice without altering the preserved patch unexpectedly. [VERIFIED: git show/diff comparison this session]
- [ ] Commit/adopt rather than recreate `crates/fava/tests/hostile_ingress.rs`; extend it to the missing hostile classes. [VERIFIED: git status and file inspection this session]
- [ ] Add provider-failure owner/public conformance tests for HARD-09. [VERIFIED: repository-wide rg inspection this session]
- [ ] Add M8 scenario registry entries, `has_executor` mapping, CLI dispatch, evidence schema checks, and seven executors. [VERIFIED: apps/canary/scenarios.json:112-166; apps/canary/src/lib.rs:146-174; apps/canary/src/main.rs:66-124]
- [ ] Add a generic separate-process adversarial relay/proxy harness if the existing canary supervisor cannot launch it, without adding public architectural vocabulary. [VERIFIED: M8 process requirements and current canary inspection]
- [ ] Provision Go 1.25, then `go mod verify`, `go test ./...`, and build the checked-in Khatru fixture. [VERIFIED: apps/canary/relays/khatru/go.mod:3-8; environment probe this session]
- [ ] Add a fail-closed feature mapping check so `built` canary evidence cannot name an absent registry/executor/dispatch path. [VERIFIED: current feature/canary mismatch]
- [ ] Create the OPS-004 owner/resource ledger and evidence-envelope schema before claiming HARD-08/10. [VERIFIED: GOALS:1359-1376; M8 plan:850-855]

### Final Milestone Exit Gate

Phase 08 exits only when all of the following are simultaneously true:

1. All `HARD-01` through `HARD-10` requirement rows have public-Fava evidence and no partial/absent disposition. [VERIFIED: .planning/ROADMAP.md:150-161; AGENTS.md:32-37]
2. The seven exact scenarios—`nip42-write-and-reconnect`, `auth-account-isolation`, `hostile-relay-ingress`, `relay-limit-shortfall`, `ambiguous-handoff`, `attempt-ceiling`, `provider-failure-isolation`—are registered, dispatched, enabled, and pass. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:805-848,1301-1307]
3. Hostile scenarios use real sockets and a separate process; one real third-party relay proves NIP-42 and persistence; Khatru or another second implementation passes the core read/publish subset. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855]
4. Every run publishes bounded resource envelopes and exact failure evidence, validated by an independent witness rather than diagnostics alone. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:850-855; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:340-350]
5. The hostile-admission bypass mutation fails `hostile-relay-ingress`, and every slice's named deliberate break fails its causal evidence. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:857-859; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:415-432]
6. Full Cargo tests/clippy, canary checks, Bazel tests, vocabulary checks/unit tests, feature mapping, line limits, and `git diff --check` pass from the final committed tree, not only from mixed dirty WIP. [VERIFIED: AGENTS.md:32-38,51-66; Phase 07 validation:136-142]

## Security Domain

Security enforcement is enabled at ASVS Level 1. [VERIFIED: .planning/config.json:47-49] OWASP's current stable ASVS is 5.0.0; the repository template's V2–V6 labels match ASVS 4.x, so the table below retains those requested labels while applying the equivalent current concerns to Nostr/WebSocket behavior. [CITED: https://owasp.org/www-project-application-security-verification-standard/]

### Applicable ASVS Categories

| ASVS category | Applies | Standard control for this phase |
|---|---:|---|
| V2 Authentication | yes | NIP-42 signed AUTH, exact relay/challenge tags, policy-selected identity, connection generation, refusal isolation; no password/session invention. [CITED: https://github.com/nostr-protocol/nips/blob/master/42.md] |
| V3 Session Management | yes | Fresh auth state on reconnect, retired-generation refusal, bounded challenge deadline, exact session key/generation. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1060-1076; crates/fava-auth/src/lib.rs:255-304] |
| V4 Access Control | yes | Application policy authorizes one exact relay access; denial affects only that destination/session operation. [VERIFIED: docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1064-1076] |
| V5 Input Validation | yes | Size limit before parse; JSON/protocol parse; signature/id/filter/subscription/generation/terminal-state admission before cache mutation. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:794-803; crates/fava-transport-websocket/src/lib.rs:72-86] |
| V6 Cryptography | yes | Use pinned Nostr event/signature implementation; no custom cryptography. [VERIFIED: Cargo.lock:1035-1040; crates/fava-auth/src/lib.rs:177-197] |
| V7 Error Handling/Logging | yes | Bounded, attributable diagnostic facts; preserve exact bounded relay evidence without leaking signer secrets. [VERIFIED: docs/spec/ARCHITECTURE.md:2167-2197; docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1078-1080] |
| V9 Communication / current V4 WebSocket | yes | Real WebSocket size/frame limits, malformed/truncated/disconnect isolation, independent wire witness. [CITED: https://cheatsheetseries.owasp.org/IndexASVS.html] |

### Known Threat Patterns

| Pattern | STRIDE | Required mitigation/evidence |
|---|---|---|
| Stale AUTH challenge replay | Spoofing | Bind challenge and answer to exact relay session generation; reconnect requires fresh state; deliberate generation-break test. [VERIFIED: GOALS:1060-1076; features/relay-authentication.feature:32-40] |
| Cross-account auth state leakage | Elevation of privilege | `RelayAccess`/session isolation and concurrent deny/allow public scenario. [VERIFIED: features/relay-authentication.feature:18-30] |
| Forged/off-filter/post-CLOSED event mutation | Tampering | Validate id/signature/filter/attribution/terminal state before cache mutation; bypass-admission falsifier. [VERIFIED: M8 plan:821-825,857-859] |
| Oversized/malformed/truncated WebSocket input | Denial of service | Pre-parse message/frame bounds, scoped session failure, healthy-relay liveness witness. [VERIFIED: crates/fava-transport-websocket/src/lib.rs:72-86; M8 plan:794-803] |
| Blocking/panicking provider | Denial of service | Execute outside locks/transactions; bounded deadline/join; unrelated query/relay/write/shutdown witness. [VERIFIED: docs/spec/ARCHITECTURE.md:2223-2225; M8 plan:845-848] |
| Outcome rewritten after ambiguous handoff | Repudiation | Proxy witness plus durable exact ambiguous receipt across restart. [VERIFIED: M8 plan:833-837; TDD guide:327-350] |
| Silent NIP-11 clamp/omission | Tampering / repudiation | Exact plan or typed shortfall before wire work, with no-wire independent witness. [VERIFIED: M8 plan:827-831] |

## Sources

### Primary (HIGH confidence)

- `AGENTS.md` — authority, workflow, architecture, vocabulary, and Rust gates. [VERIFIED: AGENTS.md:1-75]
- `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/config.json` — exact requirement text, dependency, Nyquist/security configuration. [VERIFIED: REQUIREMENTS:110-121; ROADMAP:150-161; config:20-49]
- Authoritative Fava specs — M8 behavior/scenarios/exit/falsifier, owners, OPS-004, and proof model. [VERIFIED: docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md:774-859; docs/spec/ARCHITECTURE.md:2116-2225; docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1060-1090,1359-1376; docs/spec/FAVA_TDD_BDD_TESTING_GUIDE.md:300-350,415-432]
- Live Git/source/test inspection — exact committed/dirty/stash boundary and passing combined-tree tests. [VERIFIED: git and cargo commands this session]

### Secondary (MEDIUM confidence)

- [Official NIP-42](https://github.com/nostr-protocol/nips/blob/master/42.md) — wire/auth lifecycle. [CITED: github.com/nostr-protocol/nips/blob/master/42.md]
- [Official NIP-11](https://github.com/nostr-protocol/nips/blob/master/11.md) — optional limitation fields. [CITED: github.com/nostr-protocol/nips/blob/master/11.md]
- [Tungstenite 0.30 `WebSocketConfig`](https://docs.rs/tungstenite/0.30.0/tungstenite/protocol/struct.WebSocketConfig.html) — frame/message limit semantics. [CITED: docs.rs/tungstenite/0.30.0]
- [OWASP ASVS](https://owasp.org/www-project-application-security-verification-standard/) and [ASVS cheat-sheet index](https://cheatsheetseries.owasp.org/IndexASVS.html) — current stable version and applicable authentication/session/access/input/WebSocket categories. [CITED: owasp.org; cheatsheetseries.owasp.org]

### Tertiary (LOW confidence)

- None used to lock a decision.

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — exact lockfile/toolchain/module values and live versions inspected. [VERIFIED: Cargo.lock, Cargo.toml, go.mod, version commands this session]
- Architecture: HIGH — authoritative Fava specifications and current owners inspected. [VERIFIED: spec line ranges cited above]
- Current status: HIGH — Git history/status/stash, dirty diff, source, and targeted tests inspected live. [VERIFIED: commands this session]
- External protocol/library details: MEDIUM — official NIP, docs.rs, and OWASP sources checked through the research seam/web lookup. [CITED: source links above]
- Pitfalls and remaining slices: HIGH — derived directly from the authoritative exit gates and live gaps. [VERIFIED: requirement disposition and validation map above]

**Research date:** 2026-08-21  
**Valid until:** 2026-08-28, or immediately stale after any branch-head, worktree, stash, requirement, or authoritative-spec change.
