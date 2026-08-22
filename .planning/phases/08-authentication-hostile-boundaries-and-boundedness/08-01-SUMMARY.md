---
phase: 08-authentication-hostile-boundaries-and-boundedness
plan: 01
subsystem: testing
tags: [go, khatru, nip-11, real-process, boundedness]
requires: []
provides:
  - Go 1.25 qualification for the pinned Khatru relay fixture
  - bounded real-process NIP-11 readiness and exact reap evidence
  - preserved-stash and dirty-WIP continuity witnesses
affects: [08-15-hostile-ingress-canaries, 08-16-relay-limit-canaries, 08-17-delivery-process-canaries, 08-18-m8-exit-gate, HARD-04, HARD-10]
actuals:
  tokens: 3021
  tasks: 2
  commits: 2
tech-stack:
  added: []
  patterns:
    - temporary-output real-process probes with bounded readiness and reap ceilings
    - exact NIP-11 document validation before readiness is accepted
key-files:
  created:
    - apps/canary/relays/khatru/probe.sh
    - docs/issues/0018-m8-go25-khatru-readiness.md
    - .planning/phases/08-authentication-hostile-boundaries-and-boundedness/08-01-SUMMARY.md
  modified: []
key-decisions:
  - "The post-08-02 Rust WIP fingerprint 5c6da3a98c5dbbc074e3f33ff33f1bfc6fae77a52658bf3e4024e93c1bbbc604 is the current preservation witness; the older plan fingerprint remains historical pre-08-02 evidence."
patterns-established:
  - "A process is ready only after its bounded live NIP-11 response advertises every configured limit exactly."
  - "Relay teardown proves TERM, wait status, elapsed ceiling, and absence of the exact PID."
requirements-completed: [HARD-04, HARD-10]
coverage:
  - id: D1
    description: "Go 1.25.14 verifies, tests, and builds the unchanged go 1.25.0 Khatru module with GOTOOLCHAIN=local."
    requirement: HARD-04
    verification:
      - kind: integration
        ref: "apps/canary/relays/khatru/probe.sh#module verification"
        status: pass
    human_judgment: false
  - id: D2
    description: "One real loopback Khatru process publishes exact nonzero NIP-11 limits within ten seconds and is reaped within five seconds."
    requirement: HARD-10
    verification:
      - kind: e2e
        ref: "apps/canary/relays/khatru/probe.sh#GO25_KHATRU_READINESS"
        status: pass
      - kind: other
        ref: "docs/issues/0018-m8-go25-khatru-readiness.md#named-deliberate-break"
        status: pass
    human_judgment: false
  - id: D3
    description: "The probe leaves module pins, current Rust WIP, hostile-ingress evidence, planning config, and the preserved stash unchanged."
    verification:
      - kind: other
        ref: "08-01 final preservation hash checks"
        status: pass
    human_judgment: false
duration: 3min
completed: 2026-08-22
status: complete
---

# Phase 08 Plan 01: Go 1.25 Khatru Readiness Summary

**Go 1.25.14 now qualifies the unchanged Khatru module through exact NIP-11 readiness and bounded real-process teardown.**

## Performance

- **Duration:** 3 min for continuation verification and close-out; prior implementation time was not recorded
- **Started:** 2026-08-22T05:25:16Z
- **Completed:** 2026-08-22T05:27:44Z
- **Tasks:** 2
- **Files modified:** 3 including this summary

## Accomplishments

- Selected Go 1.25.14 outside the repository while retaining the checked-in `go 1.25.0` directive and dependency checksums.
- Added an executable witness that verifies the module, runs all Go tests/builds, validates a live bounded NIP-11 document, and reaps the exact relay PID.
- Recorded causal RED, a type-correct NIP-11 limit mismatch break, exact restoration, and preservation of the current M8 WIP and stash.

## Task Commits

1. **Task 1: Provision Go 1.25.x outside the repository** — host-only prerequisite; no repository commit
2. **Task 2: Prove the checked-in Khatru module through one real process** — `94e117b` (test)

**Plan metadata:** this commit.

## Files Created/Modified

- `apps/canary/relays/khatru/probe.sh` — bounded Go module, NIP-11 readiness, and process-reap witness.
- `docs/issues/0018-m8-go25-khatru-readiness.md` — focused readiness evidence, preservation hashes, causal RED, and named deliberate break.
- `.planning/phases/08-authentication-hostile-boundaries-and-boundedness/08-01-SUMMARY.md` — plan results and coverage classification.

## Decisions Made

- Used the current post-08-02 Rust patch fingerprint `5c6da3a98c5dbbc074e3f33ff33f1bfc6fae77a52658bf3e4024e93c1bbbc604` for preservation. The plan's `e7710b...` value predates the already-completed Plan 08-02 and cannot describe this worktree.
- Kept every relay binary and process artifact in a guarded temporary directory; no generated module or binary output is retained.

## Deviations from Plan

None - the two planned task artifacts and behaviors were delivered without source, module, stash, or configuration changes.

## Verification

- `go version` and `GOTOOLCHAIN=local go version` — PASS (`go1.25.14 darwin/arm64`).
- `GOTOOLCHAIN=local go mod verify`, `go test ./...`, and build — PASS.
- `apps/canary/relays/khatru/probe.sh` — PASS with a 12 ms live NIP-11 response and 24 ms exact child reap on the final run.
- Exact configured limits — PASS: subscriptions 3, message bytes 4096, result limit 17, subscription-ID bytes 64.
- Named `max_limit=18` deliberate break and checksum restoration — PASS.
- Current Rust patch `5c6da3...`, hostile-ingress blob `7b9270...`, `stash@{0}` object `5faecf...`, `.planning/config.json`, `.gsd/`, and `.planning/milestone.lock` — unchanged.
- `git diff --check` and generated-residue scan — PASS.

## Known Stubs

None.

## Threat Flags

None. The planned probe opens only an operating-system-selected loopback listener, bounds its NIP-11 response, and reaps its exact child process.

## Issues Encountered

- The plan's preservation command names the pre-08-02 Rust patch fingerprint `e7710b...`. Plan 08-02 had already committed its owned delivery files before this continuation, so the byte-preserved remaining WIP correctly hashes to `5c6da3...`; the issue record and final checks use that current witness.
- The plan's literal `go build ./...` verification emitted `apps/canary/relays/khatru/canary-khatru-relay`. It was identified as verification residue and removed without touching any pre-existing untracked file; the committed probe itself always builds into its guarded temporary directory.

## User Setup Required

None. Go 1.25.14 is active on the host.

## Next Phase Readiness

- Plans 08-15 through 08-18 can use the qualified Khatru fixture for second-relay, limit-shortfall, and M8 exit evidence.
- This plan does not claim those later canary scenarios are complete.

## Self-Check: PASSED

The task commit and all three created artifacts exist. The final live-process gate, current WIP fingerprints, stash identity, configuration hash, whitespace check, and generated-residue check pass; unrelated M8 WIP remains present and unstaged.

---
*Phase: 08-authentication-hostile-boundaries-and-boundedness*
*Completed: 2026-08-22*
