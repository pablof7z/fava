# `SimpleGroup` relay input uses a required head and finite owned tail

**Status:** Approved by Pablo on 2026-08-24; Option B
**Owner:** Pablo
**Related:** `docs/issues/0019-simple-groups.md`

## Contradiction

All four current constraints are required, but the current constructor surface
cannot satisfy them together:

1. `SimpleGroup` retains the application-selected relays for later query and
   publication composition.
2. `IntoIterator` permits non-terminating input, so collecting an arbitrary
   iterator can fail to return or allocate without bound.
3. `fava-simple-groups` may not invent a numeric relay limit, and
   `fava-state` does not own application relay selection.
4. Query and write operations own different bounds and exact refusals. Neither
   operation owns construction of the reusable `SimpleGroup` value.

`fava-state::RelaySequence` and `RelaySequenceError` are rejected because they
move application selection and a shared numeric policy into an unrelated state
owner. Mapping those errors into `QueryError` or `WriteIntentError`, or
revalidating that value in either consumer, also duplicates ownership.

Keeping arbitrary iterator constructors is therefore not a valid option. A
repeated infinite iterator is the executable counterexample.

## Rejected Option A — finite parsed collection; empty is operation-owned

Accept only an already finite owned collection of `RelayUrl` values. Remove the
string constructor and let callers use `RelayUrl::parse` directly. Preserve
first occurrences, but allow the retained collection to be empty; query and
write lowering then return their own exact empty-input errors.

Rejected shape:

```rust
SimpleGroup::from_relays(id, Vec<RelayUrl>) -> SimpleGroup
```

**Forcing requirement:** construction must terminate without a shared bound,
new nominal owner, or new refusal, while query and write keep their distinct
operation limits.

**Executable falsifiers:** a compile-fail consumer proves an arbitrary iterator
and `std::iter::repeat` are not accepted; construction preserves first order
and collapses duplicates; an empty value reaches
`QueryError::EmptyExplicitRelays` from query lowering and
`WriteIntentError::EmptyExplicitRelays` from write lowering; the existing query
and write infinite-input tests still stop at their different owner bounds.

**Cost:** the reusable value can exist without a usable relay. The invalidity is
reported only when an operation is requested.

## Approved Option B — finite head plus tail; non-empty by construction

Accept one parsed `RelayUrl` plus an already finite owned tail. Remove the
string constructor and let callers use `RelayUrl::parse` directly. Preserve
first occurrences while making empty selection unrepresentable.

Approved signature:

```rust
SimpleGroup::from_relays(id, first: RelayUrl, rest: Vec<RelayUrl>) -> SimpleGroup
```

**Forcing requirement:** the domain value must always retain at least one relay
without a new error type, numeric limit, shared owner, or operation-owned
validation during construction.

**Executable falsifiers:** compile-fail consumers prove zero-relay and arbitrary
iterator construction are unavailable; a runtime test proves first-occurrence
order and duplicate collapse across head and tail; query and write tests prove
their independent infinite-input bounds and exact errors remain unchanged.

**Cost:** callers must split a collection into head and tail, and string parsing
has no group-specific convenience surface.

## Decision

Pablo approved Option B and its forcing requirement and falsifiers. There is no
string convenience constructor: callers parse with `RelayUrl::parse`, retaining
that owner's exact error. `RelaySequence` and `RelaySequenceError` are removed;
no compatibility path, replacement owner, or simple-group numeric limit exists.

The 4,096 query cap and 256 write cap remain separately owned provisional
implementation shortcuts for resource safety. They are not Nostr limits or
domain semantics and do not constrain `SimpleGroup` construction.
