# M4: ordered asynchronous routing and subscription planning

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M4

## Product result

An automatic live `Query` opens work for immediately known relays, adds later
relay contributions without reopening the query, and withdraws a relay only
when the current merged plan no longer selects it. Explicit queries bypass the
automatic router chain. Route preview uses the same ordered derivation without
opening router sessions or relay work.

## Architecture

- `fava-routing` owns request targets, complete replacement contributions,
  ordered live composition, destination deduplication, reason/target
  attribution, current coverage, bounded shortfalls, and route preview. It
  refuses oversized router counts, destinations, targets, coverage, evidence
  text, and shortfalls at the provider boundary.
- `fava-router-app-relays` and `fava-router-fallback-relays` own their separate
  policies. The routing core contains neither policy nor protocol meaning.
- `fava-router-testkit` supplies a controllable delayed router.
- `fava` reconciles route destinations with exact relay tasks. Unchanged relay
  sessions stay live; added destinations open new work; retracted destinations
  receive exact CLOSE.
- `fava-subscriptions` receives logical demand already assigned to one
  `RelaySessionKey`. `fava-subscriptions-standard` groups only proven-compatible
  filters and retains wire-to-logical attribution.

## Exit-gate evidence

- Public-facade tests prove immediate progress, later expansion, explicit
  bypass, reactive fallback withdrawal, unchanged-relay continuity,
  side-effect-free preview, and deduplication with all reasons retained.
- Planner owner tests prove compatible author grouping and typed exact refusal
  when a relay subscription limit cannot represent all demand.
- Routing owner tests prove an oversized contribution is refused exactly rather
  than truncated or retained.
- `async-route-partial-read` uses two real relays and proves the first REQ and
  result exist before the delayed router contributes the second relay.
- `explicit-route-bypass` records zero automatic router sessions.
- `fallback-reacts` proves fallback CLOSE after upstream coverage arrives while
  the unrelated app-relay connection remains uninterrupted.
- `subscription-grouping-equivalence` proves one grouped REQ and three
  no-grouping REQs produce identical logical query results against one real
  relay.
- The `SubscriptionPlanner` remains selected through its existing provider
  contract; routing and observation code do not depend on either planner.

## Canary evidence

- `async-route-partial-read`
- `explicit-route-bypass`
- `fallback-reacts`
- `subscription-grouping-equivalence`

## Falsifier evidence

Forcing automatic query open to await the next router update made the immediate
route test fail at its 100 ms open deadline before the delayed router changed.
Restoring immediate-current-plan execution made the same test and real-relay
scenario pass.
