---
phase: 07
slug: semantic-writes-and-capability-composition
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-21
---

# Phase 07 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness through Cargo 1.90; Bazel `rules_rust` 0.73 authoritative build graph |
| **Config file** | `Cargo.toml`, `Cargo.lock`, `MODULE.bazel`, per-crate `BUILD.bazel`, and separate canary/falsifier manifests |
| **Quick run command** | `cargo test -p fava-write -p fava-write-store-memory -p fava-write-store-redb -p fava-publication -p fava --all-targets` plus new protocol crates once present |
| **Full suite command** | `cargo test --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test --manifest-path apps/canary/Cargo.toml && cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings && cargo test --manifest-path falsifiers/external-protocol-capability/Cargo.toml && bazel test //... && python3 tools/check_vocabulary.py && python3 -m unittest tools.tests.test_vocabulary_check` |
| **Estimated runtime** | quick ≤30 seconds warm; full suite measured at execution |

---

## Sampling Rate

- **After every task commit:** Run the smallest owning crate test and
  `python3 tools/check_vocabulary.py` for architectural/public API changes.
- **After every plan wave:** Run the quick M7 package command, plus canary or
  external-falsifier tests when that wave changes their contracts.
- **Before phase verification:** Full suite and the named deliberate break must
  be recorded green/red respectively.
- **Max feedback latency:** 30 seconds for a focused task loop.

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-W0-01 | TBD | 0 | CAP-01 | T-07-01 | Opaque edit bounds/version refuse without residue | unit + compile-negative | `cargo test -p fava-write -p fava-nip02 -p fava-bookmarks` | ❌ W0 | ⬜ pending |
| 07-W0-02 | TBD | 0 | CAP-02 | T-07-02 | Wrong actor/coordinate output cannot commit | unit + facade | `cargo test -p fava --test semantic_writes actor` | ❌ W0 | ⬜ pending |
| 07-W0-03 | TBD | 0 | CAP-03 | T-07-03 | Empty source produces one bounded current materialization | facade + canary | `cargo test -p fava --test semantic_writes first_value` | ❌ W0 | ⬜ pending |
| 07-W0-04 | TBD | 0 | CAP-04 | T-07-04 | Own local value is excluded and source v2 replaces atomically | model + facade | `cargo test -p fava --test semantic_writes rematerialization` | ❌ W0 | ⬜ pending |
| 07-W0-05 | TBD | 0 | CAP-05 | T-07-05 | Write/receipt stay stable across materialization and restart | model + SIGKILL | `cargo test -p fava-write-store-redb --test process_kill semantic` | ⚠️ cases missing | ⬜ pending |
| 07-W0-06 | TBD | 0 | CAP-06 | T-07-06 | Retired completions are attributable and inert | controlled schedule + deliberate break | `cargo test -p fava --test semantic_writes retired_generation` | ❌ W0 | ⬜ pending |
| 07-W0-07 | TBD | 0 | CAP-07 | T-07-07 | Two protocols pass one bounded public corpus | shared corpus + canary | `cargo test --manifest-path apps/canary/Cargo.toml semantic_capability_corpus` | ❌ W0 | ⬜ pending |
| 07-W0-08 | TBD | 0 | CAP-08 | T-07-08 | External N+1 uses only public contracts | external compile + behavior | `cargo test --manifest-path falsifiers/external-protocol-capability/Cargo.toml` | ❌ W0 | ⬜ pending |
| 07-W0-09 | TBD | 0 | CAP-09 | — | Raw future kinds require no capability/core switch | facade | `cargo test -p fava --test semantic_writes raw_future_kind` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/fava/tests/semantic_writes.rs` — public first-value,
  rematerialization, identity, stale-completion, inverse, and raw-kind evidence.
- [ ] Pure edit/codec tests in `fava-nip02` and `fava-bookmarks`.
- [ ] Memory-store semantic state-machine corpus.
- [ ] Redb semantic cases in `crates/fava-write-store-redb/tests/process_kill.rs`.
- [ ] Shared selected-product capability corpus and four named canary scenarios.
- [ ] External N+1 falsifier workspace and dependency/source allowlist checks.
- [ ] Bazel targets and vocabulary-gate coverage for every new crate/symbol.
- [ ] Deliberate-break procedure recorded in issue 0010.

---

## Manual-Only Verifications

All Phase 7 behavior is deterministic and must have automated evidence. Live
public-relay availability is not required to prove semantic generation safety.

---

## Validation Sign-Off

- [ ] All tasks have automated verification or Wave 0 dependencies.
- [ ] Sampling continuity: no three consecutive tasks without automated proof.
- [ ] Wave 0 covers all missing references.
- [ ] No watch-mode flags or sleep-based race assertions.
- [ ] Focused feedback latency remains below 30 seconds.
- [ ] `nyquist_compliant: true` and `wave_0_complete: true` set after evidence exists.

**Approval:** pending plan verification
