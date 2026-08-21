# Literal case-sensitive tag-value query semantics

**Status:** complete
**Authority:** `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`, QUERY-001, QUERY-002, RELAY-002, RELAY-003

## Problem

The completed query and subscription slices expose ids, authors, and kinds but
omit NIP-01 tag-value filters. This leaves required QUERY-001 behavior absent
and means the RELAY-003 grouping canary does not exercise its specified 300
tag-value-query case.

## Product result

Applications can constrain a `Query` by any ASCII one-letter Nostr tag key.
Keys are case-sensitive, so `#e` and `#E` are independent axes. Values are
exact strings: values within one key are alternatives, while distinct keys and
the existing id, author, and kind axes are conjunctive.

Repeated construction in any order produces the same canonical query identity.
An absent tag key is unconstrained; a present key with no values matches
nothing rather than widening the query.

The standard local evaluator, relay-demand conversion, and subscription
planner preserve those semantics. Compatible logical tag-value queries may be
coalesced into one wire request only when exact local attribution and
refiltering reproduce each original query.

## Architecture

- `fava-query` owns the declarative tag-value selection meaning using Nostr's
  existing `SingleLetterTag`; this change introduces no new architectural noun.
- Query sources evaluate the canonical selection without acquiring ownership
  of query semantics. Memory-backed sources may scan; physical indexing remains
  provider-owned.
- `fava-subscriptions` translates the selection into exact case-sensitive
  NIP-01 filter keys.
- `fava-subscriptions-standard` may group only one safely unionable tag axis at
  a time and retains wire-to-logical attribution.
- Tag cell zero identifies the exact one-letter key, tag cell one is the exact
  value, and later cells do not alter tag-filter matching.

## Evidence

- Cover all 52 ASCII one-letter keys and prove each opposite-case key does not
  match.
- Prove exact UTF-8 value matching, OR within an axis, AND across axes, and
  present-empty matching nothing through the public query path.
- Prove construction-order-independent query identity and exact `#e`/`#E`
  relay JSON.
- Replace the author-only grouping canary with 300 compatible tag-value
  logical queries that share one wire request and retain exact per-query
  results and evidence.
- Confirm the new evidence fails before implementation and under a named
  deliberate case-folding or empty-axis break.

### Red before implementation

- Plan 01 owner RED: `cargo test -p fava-query --test query_identity` and `cargo test -p fava-query-standard --test source_merge` exited 101 because `SingleLetterTag` and `Query::tag_values` were unresolved before commit `2ff9b9e`.
- Plan 01 facade RED: `cargo test -p fava --test local_source_merge` exited 101 with unresolved `fava::SingleLetterTag` before commit `22dc7fe`.
- Plan 02 wire RED: `cargo test -p fava-subscriptions --test tag_values` exited 101 with `{}` instead of exact `#e`, `#E`, UTF-8, and empty-array filters before commit `ecac1d5`.
- Plan 02 planner RED: `cargo test -p fava-subscriptions-standard --test grouping` exited 101 because 300 tag demands remained 300 subscriptions and exceeded the 64-subscription planner bound before commit `b2eebbd`.

### Green focused evidence

- Query identity: 5/5 passed, including all 52 keys and present-empty distinction.
- Standard evaluator: 7/7 passed, including exact signed/unsigned cell matching and all opposite-case keys.
- Public facade: 7/7 passed, including exact result and source evidence through `Fava::observe`.
- Exact wire translation: 3/3 passed for case, UTF-8, canonical values, and present-empty arrays.
- Standard grouping: 6/6 passed, including 300-to-1 grouping, complete logical attribution, unsafe-axis refusal, and bounds.

### Deliberate break: case-folded grouping-axis

Planner mutation exit: 101
Canary mutation exit: 1
Pre-mutation source SHA-256: afd96ad4b09b8f124aadf06c076b4466d47dd90df99e32e056cd4134b16cbc5d
Post-restoration source SHA-256: afd96ad4b09b8f124aadf06c076b4466d47dd90df99e32e056cd4134b16cbc5d

- Mutation: the standard planner temporarily treated one otherwise-identical opposite-case tag axis as merge-compatible; no production flag or retained mutation path was added.
- Planner causal failure: `unsafe_multi_axis_limit_case_and_empty_axis_candidates_remain_separate` observed one message instead of two for `opposite-case-keys`; 5 tests passed and the owning assertion failed.
- Canary causal failure: the independent preflight reported `opposite-case tag axes were not isolated: messages=1, attribution=1` before publishing the workload.
- Controls under mutation all exited 0: query identity, standard evaluator, public facade, and exact wire translation.
- Restoration: the mutation hunk was reversed directly; the source SHA-256 matched and the focused matrix plus controlled-relay canary returned green.

### Controlled-relay artifact

- Final green command: `cargo run --manifest-path apps/canary/Cargo.toml -- run subscription-grouping-equivalence --relay-bin /Users/pablofernandez/.cargo/bin/nostr-rs-relay --seed tag-values-06-1-final2 --runs-dir /tmp/fava-tag-values-final2.FcLarU`
- Green result: exit 0; `passed subscription-grouping-equivalence`.
- Latest preserved run: `/tmp/fava-tag-values-final2.FcLarU/subscription-grouping-equivalence-6ff11616eb65ddd4`
- Latest manifest SHA-256: `6aa5bb56cd1d19e9d4758b5021a654ed18782cd329aed5cc41b4d4d12142c94c`
- Initial Task 1 run: `/tmp/fava-tag-values-06-1.vqT3jj/subscription-grouping-equivalence-a4a09b5efe5d6565`; manifest SHA-256 `452230c195affa5c107f9f147727ee8967616827a24be5d4f194657b2e717d53`.
- Relay: `nostr-rs-relay 0.8.12`; process and wire facts are retained in the hashed bundle.
- Case-isolation preflight: lowercase `#d` and uppercase `#D` remained two standard-planner messages.
- Workload result: 300 exact public tag-value queries produced one grouped REQ versus 300 no-grouping REQs; every query returned exactly its seeded event ID with equal exact relay-session evidence.
- No-grouping execution: the concurrent-first 300-REQ attempt received the exact refusal `NOTICE: Subscription error: Maximum concurrent subscription count reached`; deterministic batches of 32 then completed all 300 REQs against the same seeded relay.
- Witnesses: `wire/proxy.jsonl`, `results.json`, `evidence.jsonl`, `report.md`, relay process facts/logs, and `manifest.json`.
- Code-size cohesion: `apps/canary/src/grouping.rs` is 540 lines, above the 500-line soft limit and below the 800-line hard limit; the single module deliberately keeps one bounded scenario's relay process, wire attribution, concurrent-first fallback, public refiltering, differential comparison, and hashed evidence finalization together so the acceptance witness cannot bypass a private helper owner.

### Full validation

- `cargo test --workspace --all-targets` — exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` — exit 0.
- `cargo fmt --all -- --check` — exit 0.
- `cargo test --manifest-path apps/canary/Cargo.toml` — exit 0; 7/7 tests passed.
- `cargo clippy --manifest-path apps/canary/Cargo.toml --all-targets -- -D warnings` — exit 0.
- `cargo fmt --manifest-path apps/canary/Cargo.toml -- --check` — exit 0.
- `bazel test //...` — exit 0; 25/25 tests passed.
- `python3 tools/check_vocabulary.py` — exit 0.
- `python3 -m unittest tools/tests/test_vocabulary_check.py` — exit 0; 5/5 tests passed.
- Final fresh controlled-relay scenario — exit 0 with the latest artifact above.
- Boundary diff: `Cargo.toml`, `Cargo.lock`, `crates/fava-ingest/src/lib.rs`, and `crates/fava/src/relay.rs` are unchanged.

Full validation exit: 0

## Exclusions

- Reactive `ValueSet` projections and query algebra.
- Time-range and coordinate query axes.
- A general encoded-request byte or term bound, owned by the hostile-boundary
  milestone.
- A mandatory physical tag index for every event-cache provider.
