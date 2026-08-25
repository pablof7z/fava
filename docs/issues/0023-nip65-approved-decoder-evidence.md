# 0023 — Approved NIP-65 decoder evidence closure

**Status:** implemented, uncommitted; focused gates green; independent reviewer
execution blocked by sandbox `EPERM`
**Authority:** pad session `fava/2026-08-cross-crate-cleanup-proposals`,
`fava-nip65/README.md`

## Problem

The approved NIP-65-owned decoder behavior had executable tests, but its causal
RED and deliberate-break executions were not retained in a focused repository
artifact. The approved Bazel command named only `:unit_tests`, which did not
execute the new external decoder and public-API tests. The crate also retained
an unused `nostr` development dependency, and the crate README API gate had no
compiler-truthful current document to inspect.

## Scope

- Retain causal RED executions for the independently approved decoder and error
  surface.
- Retain the two independently implementable decoder mutant executions.
- Make `//crates/fava-nip65:unit_tests` execute the library, decoder, and public
  API proofs as one Bazel `test_suite`.
- Remove the unused Cargo development dependency and its generated Bazel graph
  edge.
- Inventory only the current compiler-visible API in the crate README.

## Explicit foundation blockers

This issue does not implement `relay_lists`, remove `event_id`, `created_at`,
`supersedes`, or `MissingEventId`, remove the normal `fava-state` dependency,
or replace router-owned `KnownLists`/`OutboxRouter::remember`. Those changes
depend on the explicitly unapproved state/query observation and universal
replaceable-winner foundation.

## Causal RED before implementation

### Tolerant tag-local decoding

Command:

```sh
cargo test -p fava-nip65 --test decoder
```

Exit: `101`.

The pre-change behavior produced exactly two owning failures while the bound
controls passed:

```text
hostile_tags_stay_local_and_valid_siblings_survive:
relay list: InvalidRelay("not a relay URL")

present_empty_marker_is_unknown_not_omitted:
assertion failed: list.read_relays().is_empty()

test result: FAILED. 2 passed; 2 failed
```

The temporary pre-change decoder source SHA-256 was
`8fa3cb59a8abf7f6534c279c949828904daa7d29ff5f3814ae68b49a47d7b5e7`.

### Named wrong-kind field

Command:

```sh
cargo test -p fava-nip65 --test public_api
```

Exit: `101` with `E0559`: `RelayListError::WrongKind` had no field named
`actual`; the compiler identified the tuple variant and the exact external
assertion. The temporary tuple-variant source SHA-256 was
`23559c6a46ded4b7a25b4fb99b12faf9194c7f3de2598cb29ea02fb75e22941b`.

## Deliberate decoder breaks

### Mutant 1: invalid URL becomes event-level refusal

Mutation: restore `InvalidRelay(String)` and return it from URL parsing while
leaving the empty-marker fix intact.

Command:

```sh
cargo test -p fava-nip65 --test decoder \
  hostile_tags_stay_local_and_valid_siblings_survive -- --exact
```

Exit: `101`; the only selected test failed with
`InvalidRelay("not a relay URL")`. Mutant source SHA-256:
`2e58564849dd46f2e2f37ef292b11ae7b3e70c38f2ac8d3c89438789215064cf`.

### Mutant 2: accepted occurrences consume the result bound

Mutation: count each accepted tag instead of distinct parsed relay identities.

Command:

```sh
cargo test -p fava-nip65 --test decoder \
  repeated_relay_does_not_consume_distinct_result_bound -- --exact
```

Exit: `101`; the only selected test failed because 300 identical tags returned
`TooManyRelays { actual: 257, maximum: 256 }`. Mutant source SHA-256:
`279d242e2a6a3045feb53715b603573a175717f78b80ffa121852657391edf4e`.

Both mutations were directly reversed. No mutation switch, compatibility path,
or alternate decoder remains. The restored production decoder source SHA-256
is `d72dc9dcf56a518b5a279688cf3eeb8443998c9fc8b21dfc75aee7743634fd28`.

## README/API closure decision

A truthful current README is possible without presenting the approved final
pad API as implemented. The crate README describes only the decoder behavior
that exists now, explicitly inventories the current compiler surface, and
labels the retained source identity, winner comparison, `MissingEventId`, and
normal `fava-state` edge as foundation-dependent current state. It does not
list `relay_lists` or claim that blocked subtraction has happened.

`python3 tools/crate_readme_api.py check fava-nip65` exits `0` against 16 exact
rustdoc items. This closes the README/API gate for the current slice; it does
not close the separately blocked final API.

## Validation outcome

### Green

- `cargo test -p fava-nip65 --doc --locked`: exit `0`, one negative API
  doctest passed.
- `cargo test -p fava-nip65 --all-targets --locked`: exit `0`, six tests
  passed across library, decoder, and public API targets.
- `cargo test -p fava-router-outbox --all-targets --locked`: exit `0`, five
  tests passed.
- Focused Cargo Clippy and formatting: exit `0`.
- `cargo build --workspace --all-targets --locked`: exit `0`.
- `cargo test --workspace --doc --locked`: exit `0`.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: exit `0`.
- External semantic-capability test and Clippy gates: exit `0`.
- Canary Clippy, nine Python tooling tests, and `canary list`: exit `0`.
- README inventory check, its 11 unit tests, vocabulary checker's 36 unit
  tests, and `git diff --check`: exit `0`.
- The prescribed focused Bazel command selected four concrete tests and passed
  `decoder`, `lib_unit_tests`, `public_api`, and router `outbox`. The same four
  passed under Bazel `--config=clippy` and `--config=fmt-check`.
- Cargo metadata has no direct `nostr` dependency for `fava-nip65`; `Cargo.lock`
  removed it from that package; the regenerated Bazel `_NORMAL_DEV_DEPENDENCIES`
  entry is empty.

### Independent review execution accounting

- A fresh-context source/diff review found no actionable correctness, scope,
  evidence, README/API-truthfulness, Bazel-label, or dependency-closure defect.
- During that earlier review, a separate Bazel output-root attempt ended with
  `ENOSPC`. The reviewer removed its 467 MB temporary cache. This is historical
  environment evidence, not the current blocker.
- A 2026-08-25 recheck reports `/tmp` on a 926 GiB filesystem with 68 GiB
  available. Current capacity does not reproduce or support an ENOSPC blocker.
- The final reviewer's fresh executable attempt did not launch: its sandbox
  refused execution with `EPERM` (`Operation not permitted`). No test result
  came from that attempt. The fresh review verdict is therefore source/diff
  review only; the completed focused Cargo and Bazel executions above remain
  the executable evidence.
- Readiness: the requested implementation and evidence-accounting corrections
  are complete and focused gates are green, but a fresh independent executable
  review is not closed in the current reviewer sandbox.

### Independent repository/environment failures

- `cargo test --workspace --all-targets --locked`: exit `101`; all reached
  behavior passed except the two baseline tests
  `vocabulary_terminal_names_match_term_names` and
  `vocabulary_gate_requires_all_terms_approved`.
- `python3 tools/check_vocabulary.py`: exit `1`. Its output is byte-identical to
  the untouched original checkout, SHA-256
  `f89e1f4857668544e65c4c9b16baed2a1c9a270528d9f25822c4c6847113828c`.
- `cargo fmt --all -- --check`: exit `1` only in existing `fava-simple-groups`
  and facade files.
- The repository hard file-size gate exits `1` for existing
  `apps/canary/src/flows.rs:1210`.
- External null-cache test and Clippy gates exit `101`: its `src/lib.rs`
  independently defines `transact` twice, at lines 23 and 47 (`E0201`).
- Canary Rust tests exit `101`: 68 passed; three NIP-02 and one environment
  test require the unavailable Croissant fixture executable, while the
  simple-groups public-flow test cannot launch `go build` (`ENOENT`).
- `bazel test //...` exits `1` before executing its 70 tests because
  `fava-publisher-nip01` imports undeclared `fava_state`. Bazel Clippy exits `1`
  on undeclared first-party imports in `fava-observe`; Bazel fmt-check reaches
  the same publisher dependency failure.
- Regenerating `MODULE.bazel.lock` also reconciles its pre-existing stale input
  hashes and crate-universe repository output. That generated churn is broader
  than the one removed dev edge, but leaving the old lock would not mirror the
  current manifests and would make ordinary Bazel invocations rewrite it.

## Exit gates

- Focused Cargo decoder, public API, and doctest proofs pass.
- The prescribed Bazel `//crates/fava-nip65:unit_tests` label executes all
  three NIP-65 proof targets.
- Cargo metadata and the generated Bazel module lock contain no `fava-nip65`
  development edge to `nostr`.
- The README inventory equals the current rustdoc public API without claiming
  blocked final APIs.
- Writable-environment workspace, architecture, vocabulary, README, Bazel, and
  canary outcomes are recorded before handoff.
