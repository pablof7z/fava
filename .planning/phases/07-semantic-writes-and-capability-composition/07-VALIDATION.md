---
phase: 07
slug: semantic-writes-and-capability-composition
status: validated
nyquist_compliant: true
wave_0_complete: true
validated: 2026-08-21T17:25:04Z
validated_head: f97ecd8c0f8fd3793860cce95380ddcae9521aa3
phase_base: 6fe21f745297b4af414e52269c3ae1c813cbf28f
---

# Phase 07 — Final Validation

M7 passes its complete deterministic validation set. Raw `EventBuilder` input
preserves caller-owned `created_at`, kind, tags, content, author, and resulting
identity. Replaceable semantic edits separately use publication-engine-owned
monotonic revision time when reapplied to changing qualified source
state.

## Sensitivity experiments

| Experiment | Broken behavior | Restoration | Result |
|---|---|---|---|
| `DELIBERATE_BREAK_M7_STALE_COMPLETION` | Removing the sole current `RevisionId` predicate left the first-value tracer green but made the exact retired-completion test accept generation-one mutation against the successor event identity. | `fava-write-store/src/lib.rs` returned byte-identically; current publication target 19/19. | PASS |
| `DELIBERATE_BREAK_M7_PROTOCOL_DEPENDENCY` | Adding one `fava_signer` import to NIP-02 failed `cargo check -p fava-nip02 --lib` with E0432, `no external crate fava_signer`. | `fava-nip02/src/lib.rs` returned to SHA-256 `deefde7b77a75f8981c855c6dc46cae008dfeff79d5d527de56bbbda6156c0f2`; NIP-02 7+1. | PASS |
| `DELIBERATE_BREAK_M7_EVENT_BUILDER_BOUND` | Raising only `MAX_TAGS` from 2000 to 2001 made the exact hostile test accept 2001 raw tags instead of the typed refusal. | `fava-write/src/builder.rs` returned to SHA-256 `abaa77068de484d6b6b0cca7677414aaa263a35a0280af8288fb24533b0409e9`; raw builder target 2/2. | PASS |
| `DELIBERATE_BREAK_M7_ROUTE_READ_REVISION` | Disabling durable read reconciliation while dropping receipt notifications stranded generation two at the exact reapplication route boundary. | Causal RED `2b53b62`; GREEN `f97ecd8`; timing-free barrier test passed 95 repeated runs across executor and final verification. | PASS |

No broken source state was committed. All three restored source files have
zero diff from their pre-experiment bytes. Full transcripts and counterexamples are
in `docs/issues/0010-m7-semantic-writes-and-capability-composition.md`.

## Executable behavior matrix

| Evidence target | Exact count | Measured result |
|---|---:|---|
| `fava/semantic_write_contract` | 4 | PASS |
| `fava/semantic_write_store` | 11 | PASS |
| `fava/semantic_write_publication` | 19 | PASS |
| `fava/semantic_write_failures` | 14 | PASS |
| `fava-write-store-redb/semantic_write_store` | 16 | PASS |
| `fava-write-store-redb/process_kill` | 6 | PASS; includes all 3 exact semantic SIGKILL cases |
| `fava-nip02` | 7 unit + 1 external API | PASS |
| `fava-bookmarks` | 9 unit + 1 external API | PASS |
| external capability workspace | 3 library + 3 public lifecycle | PASS |
| `fava-write/event_builder` | 2 | PASS; exact raw fields/order/ID and shared tag/byte bounds |
| `fava/semantic_write_capabilities` | 4 | PASS; both protocol rows use one public-Fava corpus |
| canary library | 18 total; 4 exact M7 scenario tests | PASS |
| Bazel | 36 test targets | PASS |

The three required semantic process-kill names were discovered exactly once:

- `semantic::semantic_first_generation_survives_sigkill`
- `semantic::semantic_successor_and_failed_source_resume_once`
- `semantic::semantic_retired_and_terminal_work_stays_inert_after_sigkill`

The shared corpus discovered exactly:

- `nip02_passes_public_semantic_write_corpus`
- `bookmarks_pass_public_semantic_write_corpus`
- `capabilities_share_preview_bounds_and_failure_behavior`
- `capabilities_share_concurrency_and_retired_completion_behavior`

## Feature-to-test mapping

`python3 -m unittest tools.tests.test_semantic_write_feature` passed 10/10.
Every feature mapping resolves through Cargo metadata to one real test target
and exactly one `cargo test -- --list` name. Missing, non-test, empty,
zero-test, duplicate-name, duplicate-pending-comment, and malformed
module-qualified fixtures refuse.

| Feature scenario | Exact executable destination |
|---|---|
| first value | `fava/semantic_write_contract#first_value_receives_no_prior_and_exact_timestamp` |
| source-v2 reapplication | `fava/semantic_write_publication#newer_source_reapplies_once_and_preserves_unrelated_fields` |
| stable receipt/new revision | `fava/semantic_write_store#memory_generation_swap_is_compare_and_set` |
| retired completion | `fava/semantic_write_publication#interleavings::retired_completion_is_attributable_and_inert` |
| opposing operations | `fava/semantic_write_capabilities#nip02_passes_public_semantic_write_corpus` |
| external N+1 | `external-semantic-capability-proof/public_capability#external_capability_composes_through_public_fava` |
| raw future kinds | `external-semantic-capability-proof/public_capability#raw_future_event_kind_publishes_unchanged` |

## Fresh CLI evidence

All four enabled M7 IDs ran separately after the final implementation repair under
`/tmp/m7-f97ecd8.35TrgP`. Each run produced exactly one parseable
`manifest.json`, one `semantic.json`, seven total files, six artifact hashes,
one 64-character seed hash, bounded JSON shape, no raw caller seed, and at most
9,383 bytes.

| Scenario | Evidence directory | Files | Bytes |
|---|---|---:|---:|
| `replaceable-edit-first-value` | `/tmp/m7-f97ecd8.35TrgP/replaceable-edit-first-value-8b6b97e0f0e78651` | 7 | 3,569 |
| `replaceable-edit-reapplication` | `/tmp/m7-f97ecd8.35TrgP/replaceable-edit-reapplication-3f7d41f70a7b0894` | 7 | 5,570 |
| `replaceable-edit-opposing-operations` | `/tmp/m7-f97ecd8.35TrgP/replaceable-edit-opposing-operations-f290e2f2217ae709` | 7 | 9,430 |
| `protocol-crate-n-plus-one` | `/tmp/m7-f97ecd8.35TrgP/protocol-crate-n-plus-one-66d296b8a85b5516` | 7 | 4,113 |

All four are enabled in the canary registry and execute through the ordinary
CLI path. The semantic edit stores no inverse; follow/unfollow and
bookmark/unbookmark are distinct opposing edits.

## Dependency and vocabulary gates

- NIP-02 normal dependencies: exactly `fava-write`.
- Bookmarks normal dependencies: exactly `fava-state,fava-write`.
- Canary normal dependencies explicitly include both protocol crates.
- External capability normal dependencies: exactly `fava`.
- Cargo-tree forbidden provider/lifecycle paths from both protocols: empty.
- Bazel `somepath` from both protocols to publication, transport, concrete
  stores/cache, signer, routers, publisher, and delivery: empty.
- Universal-owner production Rust has no NIP-02/bookmark kind or crate switch.
  The validated scan uses repository-relative exclusions
  `!crates/fava-nip02/**` and `!crates/fava-bookmarks/**`.
- `python3 tools/check_vocabulary.py`: PASS.
- Vocabulary unit tests: 4/4 PASS.

Rustdoc JSON produced these exact consumer-visible allowlists:

```text
fava_nip02:module,follow:function,applier:function,unfollow:function
bookmark_coordinate:function,bookmark_event:function,fava_bookmarks:module,applier:function,unbookmark_coordinate:function,unbookmark_event:function
EventBuilder methods: build,content,created_at,from_parts,new,tag,tags
```

Public nominal declarations and re-exports contain no protocol-owned
`ContactList`, `BookmarkList`, decoded-list synonym, descriptor, registry,
factory, profile, compatibility, or migration value. Established Nostr
`Kind::ContactList` remains ordinary domain vocabulary.

## Build and hygiene gates

The following all passed on the restored tree:

```text
cargo build --workspace
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt/check/test/clippy --manifest-path apps/canary/Cargo.toml
cargo fmt/check/test/clippy --manifest-path falsifiers/external-semantic-capability/Cargo.toml
bazel test //...
python3 tools/check_vocabulary.py
python3 -m unittest tools.tests.test_vocabulary_check
```

All Rust code files are at most 500 lines; therefore the 501-800 cohesion
ledger is empty and the 800-line hard gate passes. The phase-range whitespace
gate passes after removing one Plan 07-03 trailing blank line from
`deferred-items.md`; no behavioral content changed. Temporary evidence lives
outside the repository. Restored deliberate-break files have zero diff.

## CAP-01 through CAP-09

| Requirement | Final evidence | Result |
|---|---|---|
| CAP-01 | pure protocol edits/opposing operations; two-row shared corpus; opposing-operation CLI | PASS |
| CAP-02 | author resolved and persisted by accepted custody; authorless edit; exact public event pubkey across generations | PASS |
| CAP-03 | no-source first value; write-store visibility; first-value CLI | PASS |
| CAP-04 | qualified source successor; unrelated state preserved; equal-time lower-ID winner; reapplication CLI | PASS |
| CAP-05 | stable write/receipt; redb recovery and 3 semantic SIGKILL cases | PASS |
| CAP-06 | exact stale signer/route/delivery refusal; durable custody reconciles dropped notifications and transient reads | PASS |
| CAP-07 | NIP-02 and bookmarks share one neutral public-Fava corpus | PASS |
| CAP-08 | independent public-only N+1 workspace; dependency-negative break and graphs | PASS |
| CAP-09 | raw kind 50001 preserves caller `created_at = 42`, three arbitrary tags in exact order, content, and accepted/signed/published identity | PASS |

## ASVS L1 dispositions

| Threat | Mitigation evidence | Disposition |
|---|---|---|
| T-07-33 stale completion repudiation | exact predicate/test, checksum, state counterexample, 19/19 restored rerun | mitigated |
| T-07-34 mapping tampering | Cargo metadata, exact `--list`, duplicate-pending refusal, negative fixtures, positive counts | mitigated |
| T-07-35 evidence/source growth | exact 2000-tag and 131,072-byte builder refusals, causal bound break, fixed seven-file bundles, all-file line gate | mitigated |
| T-07-36 protocol privilege escalation | E0432 compile break, exact metadata, Cargo tree, Bazel paths | mitigated |
| T-07-37 public vocabulary escalation | rustdoc allowlists, declaration/re-export scans, vocabulary checker | mitigated |
| T-07-38 milestone repudiation | corrected CAP map, four canary paths, three breaks, phase-range gate | mitigated |
| T-07-SC supply chain | no new third-party package; locked root/canary/external graphs pass | accepted low |

## Sign-off

- Sampling continuity: PASS.
- Deterministic barriers/deadlines; no correctness sleeps: PASS.
- Restart/SIGKILL evidence: PASS.
- Public facade and standalone consumer evidence: PASS.
- Architecture, dependency, boundedness, vocabulary, security, Cargo, Bazel,
  line, and clean-range gates: PASS.
- Nyquist state: complete.

## Validation Audit 2026-08-21

| Metric | Count |
|--------|------:|
| CAP requirements audited | 9 |
| Behavioral gaps found in final audit cycle | 4 |
| Behavioral gaps resolved with causal RED/GREEN evidence | 4 |
| Escalated/manual-only gaps | 0 |
