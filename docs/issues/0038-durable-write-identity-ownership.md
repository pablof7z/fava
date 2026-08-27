# Durable write identity ownership

## Defect

`WriteId`, `ReceiptId`, and `MaterializationId` are opaque types, but their
infallible numeric constructors admit zero even though durable recovery treats
zero next identity and zero materialization generation as incoherent. A Redb
row whose key and every nested write/receipt identity are coherently rewritten
to zero can therefore reenter custody.

Materialization successor arithmetic is also repeated by both write-store
providers instead of being owned by the shared generation value.

## Required outcome

All three identities have a nonzero representation and public fallible numeric
reconstruction for durable import, external provider implementations, tests,
and receipt reattachment. Constructing a value does not mint custody or grant
mutation authority: only a successful `WriteStore` commit makes an identity
live, and every later mutation still requires exact current identity.

`WriteId` and `ReceiptId` are minted together by initial acceptance and never
advance. `MaterializationId` begins at `FIRST` inside the store and advances
only through checked successor construction during a currentness-qualified
store mutation. No caller supplies an initial generation, identity allocator,
owner token, or wrapper.

## Recovery and exhaustion

Redb restores the nonzero next identity from metadata and rejects zero receipt
row keys before reconstructed state becomes visible. The next identity and the
new receipt row remain one atomic durable commit. Numeric exhaustion refuses
without advancing metadata, publishing a receipt change, or leaving a row.

The memory provider owns the same nonzero next-identity fact for its process
lifetime and refuses exhaustion without changing revision, receipts, snapshot,
or notifications. Materialization advancement returns `None` at `u64::MAX` and
never wraps.

## Falsifiers

- Change any identity backing value to `u64` and allow zero deserialization;
  `durable_write_identities_are_nonzero_and_round_trip_exactly` and
  `zero_write_and_receipt_identity_cannot_reenter_through_recovery` must fail.
- Replace checked next-identity advancement with wrapping arithmetic;
  `exhausted_write_identity_refuses_without_state_or_notification` and
  `exhausted_durable_identity_refuses_acceptance_atomically` must fail.
- Replace `MaterializationId::checked_next` with wrapping arithmetic;
  `materialization_generation_advancement_is_checked` must fail.
- Remove public fallible `ReceiptId` reconstruction;
  `ordered_explicit_route_survives_reopen_with_one_lane_per_identity` must stop
  compiling or fail to reattach from its stored numeric identity.
- Remove a store-side exact currentness check; the existing memory and Redb
  stale-generation/current-guard suites must accept a stale mutation and fail.

## Validation disposition

Implemented and validated. The `fava-write`, memory-store, Redb-store, public
semantic-write, query-standard, publisher, and external-consumer suites pass;
the affected crates pass strict Clippy; workspace all-target checking passes;
README inventory and diff checks pass. Workspace-wide strict Clippy remains
blocked by pre-existing `fava-fetch-cache` `map_unwrap_or` and unchecked
duration-subtraction findings. The vocabulary checker retains the repository's
pre-existing review backlog and adds no durable-write identity finding.
