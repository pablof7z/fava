# Issue 0028: exact tag-axis composition belongs to `fava-query`

**Status:** Option A approved by Pablo on 2026-08-24; implemented with focused evidence
**Owner:** `fava-query`
**Related:** GROUP-02; `docs/issues/0019-simple-groups.md`

This issue records an architecture decision. It is not vocabulary approval and
does not establish merge readiness.

## Verified contradiction

`Query::tag_values` owns union composition. Union cannot add an AND constraint
to an already-present tag axis: composing group `photos` with `{other}` would
produce `{other, photos}` and broaden `SimpleGroup::events` beyond the group.
`fava-simple-groups` cannot repair that by inspecting `FilterSelection` or by
inventing a group-owned validation or error.

## Approved operation

`fava-query` owns this exact public operation:

```rust
impl Query {
    pub fn intersect_tag_values<I, S>(
        self,
        key: SingleLetterTag,
        values: I,
    ) -> Result<Self, QueryError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>;
}
```

It consumes the complete supplied iterator under the query owner's existing
provisional 4,096-input resource-safety cap. Exceeding that cap returns the
existing exact `QueryError::TooManyTagValues`; no conflict error exists.

| Existing axis | Supplied `{photos}` | Resulting axis |
|---|---|---|
| absent | `{photos}` | `{photos}` |
| `{photos}` | `{photos}` | `{photos}` |
| `{other}` | `{photos}` | present-empty |
| `{other, photos}` | `{photos}` | `{photos}` |
| present-empty | `{photos}` | present-empty |

An absent axis becomes the supplied set. A present axis becomes its set
intersection with the supplied set. A supplied empty set also produces a
present-empty axis. Present-empty is a valid match-nothing query, not a domain
failure. All unrelated selection, source, access, ordering, freshness, and
limit state remains unchanged.

## Forcing requirement

Protocol helpers must add an AND constraint to one literal filter axis without
learning query representation, widening caller selection, or owning generic
query failure.

## Executable falsifiers

- `cargo test -p fava-query --test query_identity`
- `cargo test -p fava-simple-groups content_query_intersects_the_h_axis`

The first test covers absent, matching, disjoint, partially overlapping,
present-empty, and supplied-empty axes; unrelated query state; query-owned
bounds; and exact `TooManyTagValues`. Replacing intersection with union fails
the disjoint and partially overlapping assertions. The second test proves
`SimpleGroup::events` delegates to that operation and returns an ordinary
explicit-relay query without a group-owned error.

The focused validation record passes. Requirement checkboxes remain unchecked
until the fresh full matrix runs. GROUP-12 also remains blocked on
cryptographic vocabulary approval.
