# M3: multi-relay reactivity and bounded observation

**Status:** complete
**Authority:** `docs/spec/FAVA_REWRITE_IMPLEMENTATION_PLAN.md`, M3

## Product result

One explicit live `Query` can ask several relays. Fava merges the same signed
event into one `EventRecord`, credits only relays that actually served it,
restores a disconnected relay with fresh session and subscription identity,
and refuses frames attributed to the prior subscription. `Observation` retains
one latest state rather than an update backlog and reports superseded revisions
through bounded diagnostics.

## Architecture

- One explicit relay set opens one independently cancellable relay task per
  exact `RelaySessionKey`; no operating-system thread is assigned per query.
- `EventCache` remains the merge authority for signed relay events and
  `QueryEvaluator` remains the authority for deduplication and evidence merge.
- Reconnect replaces only the disconnected relay session and its exact
  subscription identity. No EOSE fact exists without an actual EOSE frame.
- Query sources and application observations use Tokio watch channels: each
  boundary retains one current value. Superseded revisions increment
  `coalesced_query_updates`.
- Causal write and receipt facts remain outside the current-state observation
  channel.

## Exit-gate evidence

- A current-thread Tokio test opens 1,000 simultaneous idle observations and
  confirms they remain on the same operating-system thread.
- A burst of 256 committed events, after 128 cancelled pulls, produces one
  exact latest snapshot containing all 256 events and a positive coalescing
  count.
- Scripted public-facade tests prove multi-relay evidence merge, non-serving
  relay exclusion, reconnect identity replacement, and old-subscription frame
  refusal.
- `multi-relay-dedup-provenance` passes with three independent
  `nostr-rs-relay 0.8.12` processes and three wire proxies.
- `reconnect-generation` kills and restarts a real relay, observes a fresh REQ
  without application resubscription, and uses the proxy to inject old and
  current subscription frames.
- A second relay implementation remains an M8 exit requirement, as specified.

## Canary evidence

- `multi-relay-dedup-provenance`
- `reconnect-generation`
- `slow-consumer-latest-state`

## Falsifier evidence

Replacing current-subscription attribution with the first available filter made
`reconnect-generation` admit the injected old-subscription event and fail.
Restoring exact attribution made the same real-relay scenario pass.
