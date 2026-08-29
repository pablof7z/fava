# Complete initial-route idempotence

**Status:** implemented
**Branch:** `fix/memory-route-idempotence`
**Authority:** WRITE-024, M7
**Approved by:** Pablo, 2026-08-26

## Defect

Reserved semantic admission may repeat after the exact same revision and
initial automatic route effect are already durable. The replay decision must
compare the whole persisted effect of that route plan. Comparing only revision,
settlement, and destination identities can accept a different shortfall or a
different coverage-derived shortfall as the already-committed operation.

The route-effect reconstruction was also copied into the memory and redb
providers. That made parity depend on two implementations even though the
neutral write-store receipt transition already owns how a `RoutePlan` becomes
durable receipt state.

## Decision

`apply_route_to_receipt` remains the sole route-plan-to-receipt transition. It
now accepts a same-revision plan only when reconstructing that plan produces
the complete persisted route effect already present on the receipt, including
outcome, settlement, shortfalls, desired destinations, attempts, and current
publication destinations. A different effect at the same revision refuses.

Memory and redb admission retain their own atomic custody transactions and
exact revision checks. Both delegate only the route-effect comparison to
the neutral owner. No state, provider contract, public symbol, persisted field,
or compatibility path is added.

## Causal evidence

The public memory-store and durable redb suites each persist a direct
shortfall and a coverage-derived shortfall, replay the exact plan without a
receipt mutation or notification, then replay the same event and generation
with a different shortfall. The mismatch must refuse without changing the
retained receipt or publishing a notification.

Deleting the `route_shortfalls` comparison from the neutral reconstruction is
the deliberate falsifier: both suites accept the changed shortfall as
idempotent. The redb case reopens no alternate comparator, so the same break
proves provider parity depends on the shared receipt transition.

## Exit gates

- Focused memory and redb tests pass through Cargo and Bazel.
- The complete affected Cargo suites pass.
- Focused Clippy, rustfmt, vocabulary validation, and diff checks pass or retain
  an independently identified baseline disposition.

## Validation

Green:

- the deliberate removal of only shared shortfall equality makes both named
  tests fail with `shortfall mismatch was accepted as idempotent`;
- all 19 public memory semantic-store tests and all 29 redb semantic-store
  tests through Cargo;
- `fava-write-store` and `fava-write-store-memory` Cargo package tests;
- `//crates/fava:semantic_write_store` and
  `//crates/fava-write-store-redb:semantic_write_store` through Bazel;
- focused Clippy with warnings denied, rustfmt, the 36 vocabulary-tool unit
  tests, and diff checks.

The repository vocabulary command remains red on the pre-existing public
inventory and approval backlog. This slice adds no public symbol, crate, or
architectural vocabulary term and does not modify the vocabulary registry.

## Public apply-route review closure

Review found a remaining provider bypass after initial admission was fixed.
Memory and redb each returned early for a same-revision route when revision,
destination identities, raw `plan.shortfalls`, and settlement matched. That
partial check did not own coverage-to-shortfall derivation and could therefore
disagree with the neutral complete-effect comparator.

The causal public `WriteStore::apply_route` proof first persists a
coverage-derived shortfall for one `SettledAbsent` target while an unresolved
target keeps the route open. A same-revision candidate then copies that
persisted string into its raw shortfalls and names a different
`SettledAbsent` target. The inherited redb fast path accepted the candidate;
the inherited memory path separately notified on an exact replay because its
partial check could not recognize the derived shortfall.

Both provider-local equality checks are removed. Every route replay now calls
`apply_route_to_receipt`; the provider transaction publishes only when the
shared transition changes the receipt. Exact replay remains mutation-free,
and the changed derived effect refuses without receipt mutation or
notification in memory and redb.

The two new proofs pass through their complete Cargo and Bazel semantic-store
targets. Focused Clippy with warnings denied, rustfmt, vocabulary-tool unit
tests, and diff checks remain green. The unchanged repository vocabulary
inventory backlog remains the only red validation command.
