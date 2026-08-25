# Issue 0027: `SimpleGroup` construction validates its complete finite input

**Status:** Approved by Pablo on 2026-08-25
**Owner:** Pablo
**Related:** `docs/issues/0019-simple-groups.md`

## Decision

`SimpleGroup` construction accepts the complete finite relay selection and owns
the two invariants required before the reusable value exists:

```rust
SimpleGroup::from_relays(id, relays: Vec<RelayUrl>)
    -> Result<SimpleGroup, SimpleGroupConstructionError>
```

`SimpleGroupConstructionError` is public and attributable:

- `EmptyId` rejects exactly a zero-length id. No trimming or normalization is
  performed, so every non-empty string remains an opaque NIP-29 id.
- `EmptyRelays` rejects exactly an empty vector.

Accepted relays are already parsed `RelayUrl` values. Construction preserves
the first occurrence of every relay and removes later duplicates without a
numeric domain limit. No string parser, arbitrary-iterator constructor,
compatibility overload, alias, or shared relay owner exists.

## Ownership

The constructor is the single owner of whether a reusable `SimpleGroup` can be
formed. Query and write operations retain their independent resource bounds and
typed refusals when the valid group is lowered; they do not repeat constructor
validation.

## Forcing requirement

Applications receive one reusable group value only after both its NIP-29 id and
application-selected relay sequence are usable. A complete owned `Vec` keeps
construction finite while allowing empty input to remain caller-attributable.

## Executable falsifiers

- Public and unit tests require exact `EmptyId` and `EmptyRelays` variants.
- A runtime test requires whitespace ids to remain valid and exact.
- A runtime test requires first-occurrence relay order and duplicate collapse.
- The public API canary requires the two-argument fallible signature.
- An arbitrary iterator does not type-check as the relay argument.
- Query and write tests retain their independent infinite-input bounds and
  exact errors.
