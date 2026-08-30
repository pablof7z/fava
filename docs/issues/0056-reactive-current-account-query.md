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

`fava-observe` receives the existing `Session` during facade assembly and
refuses a current-account query when no session owner is configured. A current-
account query returns one stable application `Observation` and internally owns
successive exact concrete observations. Session change signals are subscribed
before the initial atomic account snapshot. Every source open is rechecked
against the exact account plus selection revision before activation; signer and
unrelated-account churn cannot invalidate it. At most 64 unstable selection
attempts are admitted before typed refusal. Activation and stable delivery
linearize under the session owner lock. Each replacement activates fully before
synchronously retiring the prior concrete owner. Distinct observation, demand,
wire, plan, and operation identities make every retired completion inert.
Snapshot revision remains monotonic on the stable handle.

The stable application observation id directly owns every provisional child and
its one active child. Provisional children cannot contribute desired relay
demand. Active-owner validation, diagnostic commit, and stable delivery share
one registry critical section. Query diagnostics, relay evidence, and relay
subscription ownership translate child identities to the public observation.
Outer close synchronously withdraws every child, so retired or pending children
cannot overwrite, resurrect, own wire demand, or expose private generation ids.

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
  `CLOSE`, Bob `CLOSE`, stable query and relay diagnostic ownership, and no broad
  `REQ` after clear. An event delivered under Alice's retired subscription cannot
  enter Bob's view.
- Races: controlled blocking sources prove B cannot publish after C becomes
  current and close during B open synchronously retires A without allowing B to
  resurrect demand or diagnostics.
- Mutation: bypassing reactive detection makes the initial current query empty.
  Disabling both match-nothing relay protections emits a third broad `REQ` and
  fails the live proof.
