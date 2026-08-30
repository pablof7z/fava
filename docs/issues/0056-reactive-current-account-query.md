# 0056 — Bind current account behind one stable observation

**Status:** implemented; focused gates pass
**Authority:** ID-002, QUERY-001, QUERY-002, QUERY-010
**Parent:** `0054-current-account-reactive-root.md`

## Defect

`Query` could retain only literal authors and tag strings. `Observer` opened one
concrete query for life and had no session input. Changing current account could
therefore affect neither local evaluation nor relay demand without application
query reconstruction and observation reopening.

## Decision

`fava-query` adds one focused declarative dependency on current account:

```text
Query::authors_current_account
Query::tag_value_current_account
Query::depends_on_current_account
Query::bind_current_account
Query::matches_nothing
```

Binding intersects existing literals, uses canonical raw public-key hex for tag
values, and turns no current account into a present empty axis. Unbound and
present-empty queries match nothing.

`fava-observe` receives the existing `Session` during facade assembly. A current-
account query returns one stable application `Observation` and internally owns
successive exact concrete observations. Session change signals are subscribed
before the initial atomic account snapshot, so no switch can fall between
snapshot and watch. Each replacement opens fully before the old concrete owner
is dropped; distinct observation, demand, wire, plan, and operation identities
make every retired completion inert. Snapshot revision remains monotonic on the
stable outer handle.

The stable application observation id also owns diagnostics. Concrete
generations publish their current route, demand, and wire facts under that id;
retired internal ids are not exposed as extra open application queries.

No-current binding is forced cache-only and concrete open independently
suppresses relay work for every match-nothing query. Both protections are
required because Nostr wire filters treat present-empty author/id/kind sets as
unconstrained.

## Evidence

- Red: query-domain tests failed for all missing declarative methods; the first
  public observation switch timed out before session wiring.
- Green: binding tests prove author/tag binding, literal intersection, and empty
  semantics. Public cache tests prove A→B, clear, A→B→A coalescing, stable handle
  identity, and old-source isolation.
- Wire: the fake transport observes exact Alice `REQ`, distinct Bob `REQ`, Alice
  `CLOSE`, Bob `CLOSE`, stable public diagnostics, and no broad `REQ` after clear.
  An event delivered under Alice's retired subscription cannot enter Bob's view.
- Mutation: bypassing reactive detection makes the initial current query empty.
  Disabling both match-nothing relay protections emits a third broad `REQ` and
  fails the live proof.
