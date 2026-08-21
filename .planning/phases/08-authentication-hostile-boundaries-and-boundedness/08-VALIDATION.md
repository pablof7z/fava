---
phase: 08
slug: authentication-hostile-boundaries-and-boundedness
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-21
---

# Phase 08 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via Cargo 1.90, Tokio tests, Python `unittest`, canary CLI, and Bazel |
| **Config file** | `Cargo.toml`, `Cargo.lock`, `apps/canary/Cargo.toml`, `apps/canary/scenarios.json`, and crate `BUILD.bazel` files |
| **Quick run command** | Smallest affected exact test plus the matching focused owner/public bundle |
| **Full suite command** | `cargo test --workspace --all-targets` plus strict Clippy, canary, Bazel, vocabulary, feature-mapping, line, and diff gates |
| **Estimated runtime** | Focused: under 120 seconds; full suite and seven real-process canaries: measured during execution |

Go 1.25 is a Wave 0 prerequisite for the checked-in Khatru module. The current local Go 1.23.3 cannot load its `go.mod`.

---

## Sampling Rate

- **After every task commit:** Run the exact new test with `-- --exact`, then the smallest affected focused command.
- **After every plan wave:** Run all affected owner/public bundles, the plan's real-process scenario, `git diff --check`, and vocabulary checks for public/API changes.
- **Before `$gsd-verify-work`:** Full Cargo, strict Clippy, canary, Bazel, vocabulary, feature-mapping, line-limit, and final committed-tree gates must be green.
- **Max feedback latency:** 120 seconds for focused automated checks; real-process scenarios must declare and enforce their own bounded deadline.

Current focused commands:

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

---

## Per-Task Verification Map

The planner must reconcile these provisional task IDs with the final remaining-only PLAN files. No HARD requirement may lose its owner/public, deliberate-break, and process/network row.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | HARD-05, HARD-06, HARD-07 | T-08-DELIVERY | Dirty delivery/store WIP becomes a self-contained exact-generation, spent-budget, and ambiguity slice | public integration | `cargo test -p fava --test delivery_bounds` plus memory/Redb parity | ✅ mixed committed/WIP | ⬜ pending |
| 08-01-02 | 01 | 1 | HARD-03 | T-08-INGRESS | Adopt and extend the existing hostile-ingress WIP without bypassing admission | public integration | `cargo test -p fava --test hostile_ingress` | ✅ untracked WIP | ⬜ pending |
| 08-02-01 | 02 | 2 | HARD-03 | T-08-INGRESS | Invalid, oversized, stale, post-CLOSED, never-EOSE, truncated, silent-limit, and disconnect behavior remains scoped | hostile/process | exact hostile corpus plus `hostile-relay-ingress` | ❌ W0 process harness | ⬜ pending |
| 08-03-01 | 03 | 2 | HARD-08 | T-08-BOUNDS | Every OPS-004 owner/resource category has a bound, owner, refusal/backpressure rule, and exceed-limit test | contract/resource | resource-ledger checker plus affected owner tests | ❌ W0 ledger | ⬜ pending |
| 08-03-02 | 03 | 2 | HARD-09 | T-08-PROVIDER | Provider panic, block, late, malformed, and cancel-ignore cannot stall unrelated work or shutdown | conformance/public | provider exact tests plus `provider-failure-isolation` | ❌ W0 | ⬜ pending |
| 08-04-01 | 04 | 3 | HARD-01 | T-08-AUTH | Reconnect uses a fresh generation-scoped challenge and persists the write | real process/network | `nip42-write-and-reconnect` | ❌ W0 | ⬜ pending |
| 08-04-02 | 04 | 3 | HARD-02 | T-08-AUTH-ISO | One account's denial cannot block another account | real process/network | `auth-account-isolation` | ❌ W0 | ⬜ pending |
| 08-04-03 | 04 | 3 | HARD-04 | T-08-LIMITS | Real NIP-11 produces an exact shortfall before invalid REQ/EVENT bytes cross the wire | real process/network | `relay-limit-shortfall` plus independent no-wire witness | ❌ W0 | ⬜ pending |
| 08-05-01 | 05 | 4 | HARD-05, HARD-06 | T-08-ATTEMPT | Offline time spends no budget; real attempts reach terminal policy within the declared ceiling | restart/process | `attempt-ceiling` plus Redb reopen parity | ❌ W0 | ⬜ pending |
| 08-05-02 | 05 | 4 | HARD-07 | T-08-AMBIGUITY | Full handoff without OK remains durable ambiguity across process restart | proxy/restart | `ambiguous-handoff` | ❌ W0 | ⬜ pending |
| 08-05-03 | 05 | 4 | HARD-10 | T-08-EVIDENCE | Seven exact M8 scenarios use real sockets/processes and emit bounded resource/failure evidence | registry/CLI | canary registry tests and all seven CLI runs | ❌ W0 | ⬜ pending |
| 08-05-04 | 05 | 5 | HARD-01–10 | T-08-EXIT | Final committed tree satisfies every exit gate and deliberate break | milestone gate | full command matrix below | ❌ final | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Preserve `stash@{0}` and the exact dirty patch; never apply, drop, reset, or regenerate them during planning/execution.
- [ ] Make committed `197c278` plus dirty delivery/outcome/store definitions one self-contained testable slice.
- [ ] Adopt and extend `crates/fava/tests/hostile_ingress.rs`; do not recreate or discard it.
- [ ] Add provider-failure owner/public conformance tests for HARD-09.
- [ ] Add seven M8 registry entries, executor mappings, CLI dispatch paths, and evidence-schema checks.
- [ ] Add a separate-process adversarial relay/proxy harness if the existing canary supervisor cannot launch one.
- [ ] Provision Go 1.25; run `go mod verify`, `go test ./...`, and build `apps/canary/relays/khatru`.
- [ ] Add fail-closed feature mapping so `built` cannot name an absent registry/executor/dispatch path.
- [ ] Create the OPS-004 owner/resource ledger and evidence-envelope schema before claiming HARD-08 or HARD-10.

---

## Controlled Schedules and Deliberate Breaks

- Barriers, channels, controlled clocks, proxy gates, and process witness signals establish order. Sleeps may enforce only an outer deadline.
- Every slice records the exact production seam disabled and the exact test that fails before restoring the bytes and rerunning green.
- External-effect claims require a witness independent of Fava diagnostics: proxy transcript, relay log, filesystem/database state, PID/port state, or resource sampler.
- Restart tests must destroy runtime state, reopen through the supported construction path, observe the public result, and continue the operation.
- The final hostile-admission mutation bypasses admission before cache mutation and must fail `hostile-relay-ingress`.

---

## Manual-Only Verifications

All phase behaviors require automated evidence. Environment provisioning of Go 1.25 may be manual, but `go version`, `go mod verify`, `go test ./...`, and the Khatru process scenario are automated gates afterward.

---

## Final Milestone Exit Gate

Phase 08 exits only when all conditions are simultaneously true:

1. HARD-01 through HARD-10 have public-Fava evidence and no partial/absent disposition.
2. `nip42-write-and-reconnect`, `auth-account-isolation`, `hostile-relay-ingress`, `relay-limit-shortfall`, `ambiguous-handoff`, `attempt-ceiling`, and `provider-failure-isolation` are registered, dispatched, enabled, and passing.
3. Hostile scenarios use real sockets and a separate process; a third-party relay proves NIP-42 and persistence; Khatru proves the core subset as a second implementation.
4. Every scenario publishes bounded resource envelopes and exact failure evidence validated by an independent witness.
5. The hostile-admission bypass and every named deliberate break fail their causal evidence before restoration.
6. The final committed tree, not only the mixed dirty checkout, passes:

```bash
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --manifest-path apps/canary/Cargo.toml --all-targets
cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings
bazel test //...
python3 tools/check_vocabulary.py
python3 -m unittest discover -s tools/tests
git diff --check
```

The executor must also run all seven fresh CLI scenarios, feature-to-executor mapping, line-limit checks, Khatru verification, and every plan-specific exact/falsifier command.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verification or Wave 0 dependencies.
- [ ] Sampling continuity: no three consecutive tasks without automated verification.
- [ ] Wave 0 covers all missing references.
- [ ] No watch-mode flags.
- [ ] Focused feedback latency is under 120 seconds.
- [ ] All seven scenario deadlines and resource envelopes are measured and recorded.
- [ ] `nyquist_compliant: true` set in frontmatter.

**Approval:** pending
