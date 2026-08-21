# Literal case-sensitive tag-value query semantics

**Status:** in progress
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

### Controlled-relay artifact

- Green command: `cargo fmt --manifest-path apps/canary/Cargo.toml -- --check && runs_dir=$(mktemp -d /tmp/fava-tag-values-06-1.XXXXXX) && cargo run --manifest-path apps/canary/Cargo.toml -- run subscription-grouping-equivalence --relay-bin /Users/pablofernandez/.cargo/bin/nostr-rs-relay --seed tag-values-06-1 --runs-dir "$runs_dir"`
- Green result: exit 0; `passed subscription-grouping-equivalence`.
- Preserved run: `/tmp/fava-tag-values-06-1.vqT3jj/subscription-grouping-equivalence-a4a09b5efe5d6565`
- Manifest SHA-256: `452230c195affa5c107f9f147727ee8967616827a24be5d4f194657b2e717d53`
- Relay: `nostr-rs-relay 0.8.12`; process and wire facts are retained in the hashed bundle.
- Case-isolation preflight: lowercase `#d` and uppercase `#D` remained two standard-planner messages.
- Workload result: 300 exact public tag-value queries produced one grouped REQ versus 300 no-grouping REQs; every query returned exactly its seeded event ID with equal exact relay-session evidence.
- No-grouping execution: the concurrent-first 300-REQ attempt received the exact refusal `NOTICE: Subscription error: Maximum concurrent subscription count reached`; deterministic batches of 32 then completed all 300 REQs against the same seeded relay.
- Witnesses: `wire/proxy.jsonl`, `results.json`, `evidence.jsonl`, `report.md`, relay process facts/logs, and `manifest.json`.

## Exclusions

- Reactive `ValueSet` projections and query algebra.
- Time-range and coordinate query axes.
- A general encoded-request byte or term bound, owned by the hostile-boundary
  milestone.
- A mandatory physical tag index for every event-cache provider.
