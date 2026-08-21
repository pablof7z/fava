---
status: resolved
trigger: "M8 parked unreachable delivery never retries after the relay becomes reachable. Reproduce: cargo test -p fava --test delivery_bounds offline_time_spends_no_attempt_budget_and_the_write_stays_open -- --exact"
created: 2026-08-21T18:33:10Z
updated: 2026-08-21T18:56:26Z
---

## Current Focus

hypothesis: Resolved — delayed retry, durable generation, spent budget, and Redb transition remain distinct and compose correctly.
test: Human confirmed the original M8 workflow is fixed end-to-end after all automated guardrails passed.
expecting: Archive the confirmed session and commit only the recorded fix/debug slice.
next_action: Move this record to resolved, commit the six recorded fix paths, and append the durable knowledge-base entry.
known_pattern_candidate: "NMP offline EVENT semantics — pre-handoff NotConnected spends no attempt; retry ownership remains with the shared relay session"
bug_class: Bohrbug
candidate_causes:
  - "code: WaitFor has no next effect, generation is derived from spent budget, and Redb omits the valid retry source state"
  - "config: retry interval and ceiling were tested and eliminated"
  - "environment: Tokio scheduling failure was tested and eliminated"
  - "data: terminalization and stale materialization were tested and eliminated"
and_gate: "yes: Memory needs WaitFor to authorize a delayed attempt AND publication to advance generation; Redb correctness additionally needs Unreachable -> Attempting"
reasoning_checkpoint:
  hypothesis: "Three code seams jointly block provider-independent retry: WaitFor redecides an unchanged stateless fact forever, publication passes spent budget as durable generation, and Redb omits Unreachable from retryable begin-attempt states."
  confirming_evidence:
    - "The exact public-Fava repro remains Open/Unreachable with attempts=1, spent=0, and zero new connections across twenty post-switch observations."
    - "The agent-authored Redb regression fails exactly at begin_attempt generation 2 with destination is not pending."
    - "WRITE-019 and the M8 attempt-ceiling gate require offline time to spend no attempt while later real failures advance the finite budget."
  falsification_test: "With WaitFor delaying one attempt, publication using receipt.attempts for generation and receipt.spent only for policy facts, and Redb accepting Unreachable, either unchanged regression still failing falsifies the proposed complete mechanism."
  fix_rationale: "The fix gives the timer an executable next effect while preserving store revalidation, restores the identity/budget ownership split, and restores provider parity; it does not weaken ceilings or stale-generation checks."
  blind_spots: "WaitFor currently has only one producer; custom future policies might want fact-change parking rather than delayed attempt and would need a distinct decision. Redb restart of Unreachable is covered only by adjacent recovery suites."
  candidate_causes:
    - "code: WaitFor lacks a state change, publication conflates monotonic generation with non-spending budget, and Redb omits a valid source state"
    - "config: retry interval and ceiling were tested and eliminated"
    - "environment: deterministic Tokio scheduling failure was tested and eliminated"
    - "data: receipt terminalization or stale materialization was tested and eliminated"
  and_gate: "yes: the named Memory repro requires WaitFor to authorize a delayed attempt AND publication to advance generation; Redb-backed correctness additionally requires the provider transition"
tdd_checkpoint: null

## Symptoms

expected: After an unreachable relay becomes reachable, the same durable obligation retries; offline time spends zero attempt budget and a real pre-handoff refusal reaches the configured ceiling.
actual: The receipt remains Open with an Unreachable destination, spent attempts stay zero, no new connection opens, and wait_terminal times out.
errors: "receipt settles once real attempts happen: Elapsed(())"
reproduction: "cargo test -p fava --test delivery_bounds offline_time_spends_no_attempt_budget_and_the_write_stays_open -- --exact"
started: Observed after rebasing the dirty M8 WIP onto M7-refactored main; a clean affected-package rebuild reproduces deterministically.

## Eliminated

- hypothesis: The configured unreachable retry interval or attempt ceiling suppresses the wake-up.
  evidence: The policy unit tests pass, the test configures a 10 ms interval, and repeated post-switch reads retain the exact generation-1 state predicted by a rejected begin-attempt rather than advancing toward the ceiling.
  timestamp: 2026-08-21T18:37:03Z
- hypothesis: Tokio scheduling or test timing prevents the lane from waking.
  evidence: The deterministic failure persists for more than 100 configured retry intervals and the state remains stable; the blocking condition is the exact durable generation check, independent of wall-clock scheduling.
  timestamp: 2026-08-21T18:37:03Z
- hypothesis: Terminalization or stale materialization invalidates the lane.
  evidence: Every observation shows ReceiptOutcome::Open, the same desired destination, and the same current materialization; only the distinct attempt-generation and budget counters diverge.
  timestamp: 2026-08-21T18:37:03Z
- hypothesis: Separating generation from spent budget and fixing Redb parity is sufficient.
  evidence: Redb regression passed after those two changes, but the public regression still made zero new connections because WaitFor repeatedly redecided the unchanged Unreachable fact.
  timestamp: 2026-08-21T18:40:52Z

## Evidence

- timestamp: 2026-08-21T18:33:10Z
  checked: Clean rebuild and two exact reruns of the named public-Fava test.
  found: The test fails deterministically after the source APIs compile; hostile ingress 2/2 and delivery-policy unit tests 3/3 pass.
  implication: The defect is in integrated publication/store lifecycle behavior, not missing source reapplication or the pure policy decision.
- timestamp: 2026-08-21T18:35:31Z
  checked: Prior durable memory for offline EVENT delivery semantics.
  found: A prior implementation established that pre-handoff unreachability consumes no delivery attempt, while a relay-session-owned reconnect loop is responsible for presenting the same durable event again.
  implication: This is a candidate contract pattern only; the Fava implementation must still be traced and tested independently.
- timestamp: 2026-08-21T18:35:31Z
  checked: Debug knowledge-base fallback and current worktree inventory.
  found: No matching local knowledge-base entry surfaced; the worktree has fifteen intentional modified M8 source files plus two untracked M8 integration-test files and this debug directory.
  implication: Preserve the dirty WIP exactly and use path-scoped diffs; there is no prior Fava resolution to assume.
- timestamp: 2026-08-21T18:37:03Z
  checked: Exact delivery-bounds reproduction with post-switch receipt diagnostics.
  found: Across twenty observations after the transport became reachable, the receipt remained Open/Unreachable with durable attempt generation Some(1), spent budget 0, and opened-session count 0; the test then timed out exactly as predicted.
  implication: No second publisher call can begin because generation is recomputed as spent+1 == 1 and the store already owns generation 1.
- timestamp: 2026-08-21T18:37:03Z
  checked: Spectrum-based fault-localization prerequisites.
  found: A deterministic failing integration test and passing neighbors exist, but no per-test coverage/Ochiai pipeline is configured for this Rust workspace.
  implication: SBFL is skipped explicitly; direct state-transition evidence localizes this Bohrbug.
- timestamp: 2026-08-21T18:38:13Z
  checked: Existing Memory and Redb write-store tests plus both `begin_attempt` implementations.
  found: No test covers retry after Unreachable. Memory accepts Unreachable as a source state for the next generation; Redb accepts only Pending or Retryable and therefore violates provider replaceability even after publication generation is corrected.
  implication: Add a Redb regression before changing production code; the complete fix must address both sides of the AND-gate.
- timestamp: 2026-08-21T18:39:37Z
  checked: Agent-authored Redb unreachable-generation regression before production changes.
  found: Generation 1 records Unreachable with spent budget zero, then generation 2 fails deterministically with Refused("destination is not pending").
  implication: The Redb source-state omission is independently confirmed rather than inferred from source comparison.
- timestamp: 2026-08-21T18:39:37Z
  checked: Authoritative WRITE-019, M8 attempt-ceiling, architecture delivery-policy, and TDD guidance.
  found: Offline transport time must not spend failed-attempt budget; real failures must eventually terminate under the ceiling; durable facts remain store-owned and retries require stale-generation rejection evidence.
  implication: The regression oracle is specified, and generation cannot be collapsed into budget without violating exact late-completion identity.
- timestamp: 2026-08-21T18:40:52Z
  checked: Both driving regressions after the initial two-site production fix.
  found: The Redb generation regression passes, but the public-Fava test still shows attempts=1, spent=0, connections=0 for twenty observations and times out.
  implication: Generation separation and Redb parity were real defects but are not sufficient; the unchanged Unreachable fact causes the policy timer branch to repeat without authorizing an attempt.
- timestamp: 2026-08-21T18:41:49Z
  checked: Complete delivery-decision contract and all WaitFor producers/consumers.
  found: StandardDeliveryPolicy is the only producer and is deliberately stateless; Publication is the only consumer and loops back to the same durable Unreachable fact after sleeping, so no observation can ever change the next decision.
  implication: WaitFor must delay one next attempt (revalidated by the store) or the contract would need a new elapsed/wake fact; the former is the minimal existing-vocabulary fix.
- timestamp: 2026-08-21T18:42:59Z
  checked: Both driving regressions after the complete three-seam fix.
  found: Public-Fava passes with Complete/GivenUp, spent=1, durable generation=17, and one successfully opened session; the Redb generation-2 transition test also passes with spent=0.
  implication: Repeated offline generations preserve exact identity without spending policy budget, and the first real refusal reaches the configured ceiling across both stores.
- timestamp: 2026-08-21T18:43:33Z
  checked: Complete delivery-bounds suite after removing temporary diagnostics.
  found: All four cases pass: offline retry, exact retryable ceiling, ambiguous handoff, and acknowledgment.
  implication: The fix preserves adjacent delivery outcomes and does not depend on diagnostic timing.
- timestamp: 2026-08-21T18:44:40Z
  checked: Affected Cargo suites and registered Bazel targets.
  found: All tests for fava, fava-delivery, fava-delivery-standard, fava-publication, and fava-write-store-redb pass, including Redb process-kill/recovery; Bazel delivery-policy and Redb lifecycle targets pass 2/2.
  implication: Adjacent behavior, durable recovery, and both supported build graphs accept the fix.
- timestamp: 2026-08-21T18:45:20Z
  checked: Static gates before revert/reconfirm.
  found: Strict Clippy passes for all affected packages/targets, vocabulary check passes, and git diff whitespace check passes. Workspace fmt reports unrelated existing WIP plus relevant indentation in Redb ops and line wrapping in the new test.
  implication: Correct only the two owned formatting findings and verify exact touched files; do not mechanically rewrite unrelated dirty WIP.
- timestamp: 2026-08-21T18:47:11Z
  checked: Exact-file formatting, vocabulary unit tests, mutation tooling, and Bazel lock side effects.
  found: All five touched Rust files pass rustfmt; vocabulary checker unit tests pass 4/4; no Stryker configuration exists for this Rust workspace; the Bazel-regenerated lockfile was restored to its clean pre-run state.
  implication: Mutation tooling degrades explicitly to the named deliberate break, and no generated or unrelated file remains from verification.
- timestamp: 2026-08-21T18:48:17Z
  checked: Path-scoped deliberate break and exact reapplication.
  found: With all three production corrections removed, the public test returned its original timeout and the Redb test returned destination-is-not-pending; after exact reapplication both tests pass.
  implication: The regression tests kill the behavioral mutant and causally attribute the fix; revert-and-reconfirm passes without stashing or touching unrelated WIP.
- timestamp: 2026-08-21T18:49:02Z
  checked: Final exact-file formatting, whitespace, and scoped worktree status.
  found: All touched Rust files pass rustfmt, git diff --check passes, MODULE.bazel.lock is clean, and only the intended M8/debug paths remain changed among the scoped inventory.
  implication: Verification left no generated lockfile or unrelated mutation and the fix is ready for human workflow confirmation.
- timestamp: 2026-08-21T18:56:26Z
  checked: Human verification checkpoint for the original M8 workflow/environment.
  found: The user reported "confirmed fixed" with no remaining failure.
  implication: End-to-end verification is complete and the session may be archived.

## Resolution

root_cause: "`WaitFor` slept then redecided an unchanged Unreachable fact forever; publication also used `Receipt::spent` as the monotonic generation predecessor and for give-up identity; Redb independently refused `Unreachable -> Attempting`. Together these prevented a reachable relay from receiving a fresh exact attempt while keeping offline budget at zero."
fix: "Make `WaitFor` delay one store-revalidated attempt; use `Receipt::attempts` for monotonic store generation while passing only `Receipt::spent` into delivery policy; permit Redb to begin the next generation from an Unreachable lane; add a focused Redb transition regression."
verification:
  target_test: { result: pass }
  mutation_check: { result: skipped, reason_if_skipped: "Stryker is not configured for this Rust workspace; the path-scoped deliberate break was killed by both driving regressions", mutant_killed: true }
  no_op_deletion: { result: pass, deletion_justified_by_rca: false }
  adjacent_tests:
    result: pass
    suites_run:
      - "cargo test -p fava-delivery -p fava-delivery-standard -p fava-publication -p fava-write-store-redb -p fava"
      - "bazel test //crates/fava-write-store-redb:delivery_lifecycle //crates/fava-delivery-standard:all //crates/fava-publication:all"
      - "cargo clippy for all affected packages and targets with -D warnings"
      - "vocabulary check and 4 checker unit tests"
      - "exact touched-file rustfmt and git diff --check"
  revert_and_reconfirm: { result: pass, bug_returned_on_revert: true, fixed_on_reapply: true }
  human_workflow: { result: pass, confirmation: "confirmed fixed" }
  guardrail_verdict: accepted
oracle_type: specified
files_changed:
  - crates/fava-delivery/src/lib.rs
  - crates/fava-publication/src/delivery.rs
  - crates/fava-write-store-redb/src/ops.rs
  - crates/fava-write-store-redb/BUILD.bazel
  - crates/fava-write-store-redb/tests/delivery_lifecycle.rs
  - crates/fava/tests/delivery_bounds.rs

## Prevention

causal_branches:
  code:
    - "A stateless policy returned WaitFor, but Publication interpreted the wake-up as permission only to reconsider the same unchanged Unreachable fact."
    - "Publication used spent budget as the next durable generation, so offline attempts repeatedly requested an already-owned generation."
    - "Redb's transition table omitted Unreachable as a valid source for the next Attempting generation."
  data_model:
    - "Durable attempt identity and policy-spent budget were represented as related counters without an integration test proving that they diverge while a relay is offline."
    - "Memory and Redb encoded the same destination lifecycle independently without a provider-parity regression for Unreachable -> Attempting."
and_gate: "The failure required the delayed wake-up and generation identity defects in the Memory-backed workflow; Redb-backed retry additionally required the missing provider transition."
why_not_caught: "No existing committed integration gate combined unreachable-then-reachable retry, zero offline budget spend, monotonic attempt generation, and Memory/Redb provider parity."
recurrence_guard: "The specified public regression crates/fava/tests/delivery_bounds.rs:offline_time_spends_no_attempt_budget_and_the_write_stays_open and provider regression crates/fava-write-store-redb/tests/delivery_lifecycle.rs:unreachable_generation_can_retry_without_spending_attempt_budget both pass and kill the deliberate three-seam break."
