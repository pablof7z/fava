# M6: automatic write routing and partial delivery

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M6

## Product result

An automatically routed write opens the configured ordered `Router` chain after
durable acceptance. Known relay destinations begin delivery immediately.
Unresolved NIP-65 discovery remains visible and can add destinations later
under the same `Receipt`. A relay selected by multiple reasons still receives
the event once.

## Architecture

- `RouteRequest` has read and write forms over ordinary `Query` and event facts.
  There is no second query representation.
- `RoutePlan` carries current destinations, target coverage, unresolved targets,
  exact shortfalls, revision, and settlement. Coverage and unresolved work may
  coexist for one target.
- `Publication` opens automatic routing after `WriteStore` custody, applies each
  route revision atomically, and starts signing and routing independently.
- `Receipt` records the applied route revision, settlement, bounded shortfalls,
  and the destinations currently desired by the route. Delivery facts remain
  attached after a possible handoff even if a later route withdraws that relay.
- `WriteStore` removes a withdrawn pending lane, retires a retryable pre-handoff
  lane, and preserves attempting or terminal historical facts.
- `fava-nip65` owns only NIP-65 `RelayList` parsing. It does not own routing.
- `fava-router-outbox` maps author write relays and recipient read relays. Missing
  relay lists use an ordinary `Query` explicitly routed to configured indexers.
- `fava-router-hints` uses Nostr reference hints and actual admitted relay
  evidence. It does not teach `EventBuilder` reply or reaction meaning.
- App-relay and fallback routers can independently select read and write scope.
- `Fava` implements the existing `QuerySource` contract so a router can reuse
  ordinary explicit query machinery without private sockets or routing
  recursion.
- Preview and live publication call the same ordered routing derivation.
  Preview opens no router query, signer, receipt, store entry, or relay work.

## Bounds

- At most 32 configured routers enter one chain.
- Each router contribution has at most 256 distinct relay sessions and 256
  shortfalls, and covers every target the query named.
- One write receipt accepts at most 256 current desired destinations.
- Route reason and shortfall text is bounded to 4,096 bytes.
- Bounds return exact actual and maximum values and commit no partial receipt
  mutation.

## Exit-gate evidence

- `automatic_publication` proves immediate known-lane delivery, later route
  expansion under one receipt, preview parity, and duplicate suppression.
- `outbox` proves locally known NIP-65 lists are immediate and a missing list
  opens one exact explicit kind:10002 indexer query.
- `hints` proves pointer relay hints and actual admitted relay evidence select
  the referenced event relay independently of outbox routing.
- `async-recipient-routing` uses five disposable relay processes and independent
  wire transcripts. Three relays receive the event before the third recipient's
  relay list is served by the real indexer relay; the fourth destination is then
  added under the same receipt with no duplicate sends.
- `hint-routing`, `route-preview-parity`, and
  `app-relay-versus-fallback-profile` prove their public application behaviors
  against disposable third-party relay processes.
- Dependency checks keep `fava-routing` free of concrete router crates and
  protocol-specific routing semantics.

## Falsifier evidence

The milestone falsifier changes publication to wait for route settlement before
starting any known delivery lane. Under that change,
`async-recipient-routing` cannot observe the first three EVENT handoffs before
the final relay list is served and fails. The production owner starts eligible
known lanes from every applied partial plan.
