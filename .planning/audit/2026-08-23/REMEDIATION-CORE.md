# Live-query ownership remediation — core design (draft, pre-audit)

Status: draft. Contract shapes marked `TBD-audit` await the subscriptions,
transport, and query auditors.

## The decisive authority clauses

- `ARCHITECTURE.md:3001` — Query opening ordering owned by `fava-observe`:
  `source boundary -> initial evaluation -> handle release -> later updates`.
  Relay work is *after* handle release.
- `ARCHITECTURE.md:2638` — "Router and relay work may continue while local
  source snapshots are assembled. No relay result is required to produce the
  local initial snapshot."
- `GOALS.md:313` (QUERY-004) — "The initial query value MUST be produced from
  the configured local query sources without waiting for any relay response."
- `ARCHITECTURE.md:2978-2979` — `fava-observe` owns query demand for one relay
  and owns the desired wire subscription plan; the planner only computes it and
  transport only performs it.
- `ARCHITECTURE.md:2990` — `fava-runtime` owns execution resources and joins.
- `ARCHITECTURE.md:2372` — the facade owns no retry algorithm and no socket state.

## What must move

| Fact / lifecycle | Today | Target owner |
|---|---|---|
| Relay session establishment | `fava::relay::establish` | `fava-transport`, executed by `fava-runtime` |
| Reconnect policy + generation | `fava::relay::OpenedRelay::reconnect` (fixed 50ms) | `fava-transport` |
| Subscription identity allocation | `Fava::next_subscription: AtomicU64` | `fava-observe` desired plan |
| Logical per-relay demand | not retained anywhere | `fava-observe` |
| Desired subscription plan + diff | not retained anywhere | `fava-observe` |
| Shared-work identity + refcount | absent | `fava-observe` registry |
| Route session for a live query | `fava::routes::run` task | `fava-observe` |
| Observation identity | absent | `fava-observe` |
| Cancellation of relay work | ad-hoc `watch::Sender<bool>` vec on `Observation` | `fava-observe` registry + `fava-runtime` |
| Task spawning / joins / deadlines | bare `tokio::spawn` in facade | `fava-runtime` |

## Target open sequence

`Observer::open` stays **synchronous and total**. It never awaits a provider.

1. validate query (`fava-query`)
2. open EventCache + WriteStore sources (local, non-blocking)
3. establish derived dependencies
4. bind explicit plan, or open the router chain and take its immediate contribution
5. compile logical per-relay demand for this observation
6. evaluate one complete initial `QuerySnapshot`
7. install the observation in the registry under a fresh `ObservationId`
8. **return the handle** — initial snapshot readable
9. submit a demand-change command to the relay engine (fire-and-forget, bounded channel)

Steps 1-8 contain no `.await` on any provider. Step 9 enqueues; it does not
execute. All network effects happen on the runtime, after release.

## Shared work and the sharing key

Sharing is at the level of **logical demand**, not observation:

```
DemandKey = (RelaySessionKey, semantic identity of the query's relay demand)
```

Two equivalent observations produce the same `DemandKey`, so the registry holds
one entry with refcount 2, the planner sees one demand, and exactly one `REQ`
goes out. Withdrawal is refcounted: the `CLOSE` and the session teardown happen
when the last holder drops.

`TBD-audit`: does `fava_query::Query` expose a stable semantic identity today,
or must one be derived from the compiled `RelayDemand`? Prefer deriving it from
the demand — that is the value the planner already consumes and it avoids a new
vocabulary noun.

## Relay engine (owned by `fava-observe`, executed by `fava-runtime`)

Single reconciliation owner per Fava instance:

```
commands (bounded)  ->  registry: aggregate demand per RelaySessionKey
                    ->  desired plan = planner.plan(key, all demands for key)
                    ->  diff against installed plan
                    ->  transport executes only the delta
```

Properties this buys, each of which is a falsifier:
- unchanged subscriptions survive a replan untouched
- a new observation on a busy relay adds only its own `REQ`
- withdrawal sends exactly the `CLOSE`s that lost their last holder
- a blocked `open_session` for relay B cannot delay relay A or any handle
- relay-limit shortfall is a typed fact attributable to a demand

## `fava-runtime`

Already approved vocabulary (`docs/internals/vocabulary.toml:270`, `spec_crates`),
never implemented. Minimum first slice needed by this remediation:

- spawn with a join registry, so shutdown can join outstanding work
- bounded command channels
- Fava-owned deadlines for provider calls (establishment, handoff, close)
- provider-call isolation: a panicking or stalled provider is scoped and attributable
- cancellation propagation tied to owner-held tokens, not detached `watch` senders

## The three existing falsifiers

- `relay_establishment_does_not_delay_the_coherent_local_observation` — correct
  as written; keep verbatim.
- `equivalent_observations_share_relay_work_until_the_last_handle_closes` —
  correct intent, but asserts synchronously on effects that become asynchronous.
  Must await a deterministic condition (engine quiescence or an observable
  diagnostic fact) rather than reading `script.opens` immediately.
- `cancelling_observe_while_another_relay_opens_closes_provisional_work` —
  **encodes the defective architecture.** It asserts `observe()` still times out.
  Under the correct model `observe()` returns immediately, so this test must be
  rewritten: open the observation, let relay B stall, drop the handle, then
  assert relay A's session closed and A's `CLOSE` was sent. Flagged so it is not
  mistaken for a regression during remediation.
