# Fava DX Review — August 2026

Reviewed against two real apps built on the current checkout (post phase A–G):

1. **`~/src/nip29-test/`** — a diagnostic REPL that exercises Fava's NIP-29 surface against a live relay. Every Fava capability it uses is the real public API; the contract is to expose limitations honestly.
2. **`examples/simple-groups/src/main.rs` + `support.rs`** — a straight-line CLI that exercises the full group lifecycle: create, metadata, invite, join, put/remove user, content publish, observation, saved lists, delete. Moved from `crates/fava-simple-groups/examples/demo.rs` in the most recent commit.

---

## Assembly

### What you write today (both apps)

```rust
// examples/simple-groups/src/support.rs — the cleaner of the two
Fava::builder()
    .event_cache_ephemeral()
    .write_store(Arc::new(MemoryWriteStore::default()))
    .query_evaluator(Arc::new(StandardQueryEvaluator))
    .subscription_planner(Arc::new(planner()))
    .transport(Arc::new(WebSocketTransport::new()))
    .publisher(Arc::new(Nip01Publisher))
    .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
    .signer(Arc::new(LocalSigner::new(alice.clone())))
    .signer(Arc::new(LocalSigner::new(bob.clone())))
    .signer(Arc::new(LocalSigner::new(carol.clone())))
    .materializers([saved_group_list_materializer()])
    .build()?
```

And in `Cargo.toml`, 9 provider crates for this app, 10 for nip29-test (which also needs `fava-state`; see below):

```toml
fava                          = { path = "..." }
fava-simple-groups            = { path = "..." }
fava-signer-local             = { path = "..." }
fava-write-store-memory       = { path = "..." }
fava-query-standard           = { path = "..." }
fava-subscriptions-no-grouping = { path = "..." }  # demo uses this
fava-subscriptions-standard   = { path = "..." }   # nip29-test uses this
fava-transport-websocket      = { path = "..." }
fava-publisher-nip01          = { path = "..." }
fava-delivery-standard        = { path = "..." }
```

**Verdict**: The verbosity is real. Every new Fava app rewrites this assembly block identically. The builder approach is correct and `BuildError` naming which provider is missing is genuinely excellent. But there are no preset profiles. `event_cache_ephemeral()` exists as a builder convenience and is the only example. Nothing comparable exists for the other seven roles.

The subscription planner situation is actively confusing: there are at least two planner crates (`fava-subscriptions-standard` and `fava-subscriptions-no-grouping`) and the two apps in this repo pick different ones. There is no guidance for a new developer on which to choose or what the tradeoff is.

---

## SimpleGroup construction

```rust
let url   = fava::RelayUrl::parse("wss://relay.example")?;
let group = SimpleGroup::new("photos", vec![url])?;
```

Clean. The two-variant `SimpleGroupConstructionError` (`EmptyId`, `EmptyRelays`) is ergonomic. The demo tests duplicate-relay deduplication explicitly:

```rust
// examples/simple-groups/src/main.rs
let group = SimpleGroup::new(&group_id, vec![relay.clone(), relay.clone()])?;
println!("normalized_relays={}", group.relays().count());  // prints 1
```

No friction here.

---

## Management constructors

The nine typed constructors (`create_group`, `edit_metadata`, `invite`, `join_request`, `put_user`, `remove_user`, `delete_event`, `delete_group`, `leave_group`) are clean. `MetadataEdit` with `Default` is idiomatic. Kind numbers 9000–9022 are private constants in `management.rs` and appear nowhere else in the workspace. This is the right design.

### Group content composes on `EventBuilder`

The caller chooses the ordinary event kind and payload, then composes one or
more group contexts directly on the builder:

```rust
// examples/simple-groups/src/main.rs
let content = EventBuilder::new(alice.public_key(), Kind::TextNote)
    .content("Hello from the runnable Fava simple-groups demo")
    .simple_group(&group)?;
let write = fava.publish(content)?;
```

`simple_group()` adds the exact `h` tag and accumulates the group's finite relay
route without taking ownership of content semantics. Repeating it publishes the
same event to several groups while deduplicating shared relays. The chosen kind
remains explicit caller input; the group capability does not silently rewrite
it or introduce a parallel content-event constructor.

### Friction 2: `invite()` takes a redundant relay argument

```rust
pub fn invite(
    author: PublicKey,
    group: &SimpleGroup,
    invitee: &PublicKey,
    relay: &RelayUrl,          // ← caller must supply; group already owns its relays
) -> Result<UnsignedEvent, EventBuildError>
```

The NIP-29 `relay` tag tells the invitee where to find the group. In practice callers always pick `group.relays().next()`. nip29-test writes the inevitable workaround:

```rust
// nip29-test/src/management.rs
let relay = group
    .relays()
    .next()
    .expect("SimpleGroup guarantees at least one relay");
let event = invite(author, group, &invitee, &relay).map_err(build_error)?;
```

The demo passes `&relay` directly because it already has the variable, but both callers are expressing the same intent: use the group's relay. `invite()` should default to the first group relay, accepting an `Option<&RelayUrl>` for the rare override case.

### Friction 3: no `.with_tag()` on `UnsignedEvent`; both apps write their own `append_tags`

Both apps independently implement the same workaround to add optional tags to an already-built event. The demo:

```rust
// examples/simple-groups/src/main.rs
fn append_tags(
    event: fava::UnsignedEvent,
    extra: impl IntoIterator<Item = Tag>,
) -> DemoResult<fava::UnsignedEvent> {
    let mut tags = event.tags.to_vec();
    tags.extend(extra);
    Ok(EventBuilder::from_parts(
        event.pubkey,
        event.kind,
        event.created_at,
        tags,
        event.content,
    )
    .build()?)
}
```

nip29-test has an identical `append_code` function, 9 lines for the same operation. Two independent apps, same workaround. `EventBuilder::from_parts` is the only escape hatch; callers have to know it exists. A `.with_tag()` / `.with_tags()` method on `UnsignedEvent` would eliminate both functions.

---

## Observation / query DX

### What's good

`observe(query).await` + `obs.current()` + `obs.changed().await` is a clean reactive model. `obs.close()` is explicit. `SimpleGroupStateEventKind::ALL` as a slice constant is immediately usable. The demo opens three simultaneous observations without friction:

```rust
// examples/simple-groups/src/main.rs
let mut group_events = fava.observe(group.events(Query::events().limit(128)?)?).await?;
let mut group_state  = fava.observe(group.meta_events(SimpleGroupStateEventKind::ALL)?).await?;
let mut saved_lists  = fava.observe(saved_query).await?;
```

Good.

### Friction 4: both apps independently implement the same observation polling helper

The demo has two helpers: `wait_for` (blocks until predicate) and `wait_for_optional` (times out gracefully). nip29-test has `cmd_read` (waits for non-empty or timeout). All three implement the same pattern:

```rust
// examples/simple-groups/src/support.rs
pub(super) async fn wait_for(
    observation: &mut Observation,
    predicate: impl Fn(&QuerySnapshot) -> bool,
) -> DemoResult<Arc<QuerySnapshot>> {
    tokio::time::timeout(OPERATION_TIMEOUT, async {
        loop {
            let current = observation.current();
            if predicate(&current) {
                return Ok::<_, Box<dyn Error + Send + Sync>>(current);
            }
            observation.changed().await?;
        }
    })
    .await?
}
```

Two apps, two helpers, same loop. This should be on `Observation` as `obs.wait_until(predicate, timeout)` or similar. Every app that wants to observe a state and wait for a condition will write this.

### Friction 5: no round-trip from kind number to `SimpleGroupStateEventKind`

Any app that receives a `QuerySnapshot` and wants to dispatch by kind must embed its own numeric table. nip29-test does:

```rust
// nip29-test/src/display.rs
fn decode_state_event(event: &fava::EventValue) -> String {
    match event.kind().as_u16() {
        39_000 => match SimpleGroupMetadata::from_event(event) { ... }
        39_001 => match SimpleGroupAdmins::from_event(event) { ... }
        39_002 => match SimpleGroupMembers::from_event(event) { ... }
        39_003 => match SimpleGroupRoles::from_event(event) { ... }
        39_004 => match SimpleGroupLivekitParticipants::from_event(event) { ... }
        39_005 => match SimpleGroupPins::from_event(event) { ... }
        kind   => format!("unselected state kind {kind}"),
    }
}
```

The demo avoids this by trying all decoders on every record (checking `is_ok()`), which is safe but wasteful. Both approaches exist only because there's no `TryFrom<Kind> for SimpleGroupStateEventKind`. Adding it would let callers write a clean match without embedding the numeric table.

---

## Publication DX

### What's good

```rust
let write   = fava.to([relay.clone()])?.publish(event)?;
let receipt = tokio::time::timeout(TIMEOUT, write.settled(at_least(1)?)).await??;
```

The chain is natural. `PublishError::NotReached { receipt }` carries the terminal receipt in the error — no second lookup needed. The demo uses `receipt.acknowledged()` and `receipt.rejected()` directly, which are the right counters.

### Terminal completion and publication success are explicit

The facade now exposes separate predicates for the two user-visible meanings:

```rust
let complete = write.settled(fava::all_terminal()).await?;
let successful = write.settled(fava::all_acknowledged()).await?;
```

`all_terminal()` waits for routing to settle and for every currently desired
destination to have an exact terminal fact. Rejection, exhausted delivery, and
ambiguous handoff satisfy it, so it is completion evidence rather than a success
claim.

`all_acknowledged()` requires a nonempty settled current route and exact
acknowledgement evidence for every desired destination. Historical
acknowledgements retained for withdrawn destinations cannot mask a current
rejection. If terminality makes the requested acknowledgement condition
impossible, `PublishError::NotReached { receipt }` returns the complete evidence.

`at_least(n)` remains the right predicate when success means a positive relay
threshold rather than every current destination. There is no compatibility
alias joining the two meanings.

### Friction 6: `at_least(n)` returns `Result` for guaranteed-positive n

```rust
// examples/simple-groups/src/support.rs
write.settled(at_least(1)?).await?
```

`at_least(0)` returns `Err`, `at_least(1)` always succeeds. The `?` in `at_least(1)?` is safe but unexpected in a predicate position. `at_least` could accept `NonZeroUsize` to eliminate the `Result`; `all_acknowledged()` already covers the distinct every-current-destination requirement.

### Friction 7: `wait_next_second()` workaround for replaceable event timestamps

The demo sprinkles `support::wait_next_second()` (1100ms sleep) between consecutive replaceable event publishes:

```rust
// examples/simple-groups/src/main.rs
support::wait_next_second().await;
support::publish_edit(&fava, &saved_relay, alice.public_key(), "rename...", ...).await?;
support::wait_next_second().await;
support::publish_edit(&fava, &saved_relay, alice.public_key(), "save_relay...", ...).await?;
```

This is because replaceable events require strictly increasing `created_at` timestamps, and the relay enforces it. The demo sleeps to guarantee the next timestamp is strictly larger. If `EventBuilder` (or the `ReplaceableEventEdit` path) automatically advanced the timestamp to at least `previous + 1`, this workaround would disappear. Fava owns the signing path and could enforce this invariant.

---

## `RelaySessionKey` not re-exported from `fava`

`Receipt::destinations()` returns `(RelaySessionKey, RelayDeliveryOutcome)` pairs. `RelaySessionKey` has a `relay` field every app that displays delivery outcomes needs. But `RelaySessionKey` is not re-exported from the `fava` façade. nip29-test adds `fava-state` as a direct dependency:

```toml
# nip29-test/Cargo.toml
fava-state = { path = "..." }  # needed for RelaySessionKey display in diagnostics
```

And accesses `.relay.as_str()` without naming the type (the compiler infers it, but the crate is still required to access the field):

```rust
// nip29-test/src/display.rs
for (session_key, outcome) in receipt.destinations() {
    lines.push(delivery_line(session_key.relay.as_str(), outcome));
}
```

Any app that displays per-relay delivery outcomes needs `fava-state` in its Cargo.toml. `RelaySessionKey` should be re-exported from `fava`.

---

## What's genuinely good

- `SimpleGroup::new(id, vec![relay])` — minimal args, clear error variants, silent deduplication.
- `EventBuilder::simple_group(&group)` — composes exact group tags and finite routes without a parallel publication path.
- Management constructor encapsulation — kind numbers 9000–9022 in one place only.
- `MetadataEdit` with `Default` — idiomatic partial updates.
- `SimpleGroupStateEventKind::ALL` — immediately usable constant.
- `observe(query).await` → `obs.current()` → `obs.changed().await` — the reactive model is correct and natural.
- `fava.to(relays).publish(event)` → `write.settled(predicate)` — chains naturally.
- `PublishError::NotReached { receipt }` — evidence in the error, not a separate lookup.
- `all_terminal()` / `all_acknowledged()` — completion and success are explicit and non-overlapping.
- `BuildError` names the missing provider — you know exactly what to add.
- `event_cache_ephemeral()` builder shortcut — shows the pattern for more presets.
- `fava.diagnostics()` — snapshot-based, bounded, honest about what it measures.
- `fava.preview_routes(&query)` — zero-side-effect diagnostic.

---

## Remaining friction

| Friction | Evidence | Possible boundary |
|----------|----------|-------------------|
| Both apps independently write `append_tags` / `append_code` | Two identical `EventBuilder::from_parts` workarounds | Builder-level tag composition without rebuilding `UnsignedEvent` |
| Both apps independently implement observation polling | `wait_for` in the demo and `cmd_read` in nip29-test | A bounded `Observation` predicate wait |
| 9–10 provider crates and a long assembly block per app | Every app repeats the same role selection | A documented standard profile without bypassing provider contracts |
| `RelaySessionKey` is not in `fava` re-exports | nip29-test needs `fava-state` for receipt display | Re-export the receipt-facing type from `fava` |
| `invite()` takes a relay already available from the group | nip29-test selects the first group relay manually | Make the ordinary group-relay choice explicit without hiding overrides |
| No `TryFrom<Kind>` for `SimpleGroupStateEventKind` | nip29-test embeds a raw kind-number match | Add the typed conversion |
| `at_least(n)` returns `Result` for known-positive literals | `at_least(1)?` in predicate position | Accept `NonZeroUsize` or another positive-by-construction input |
| Replaceable events need manual timestamp spacing | demo calls `wait_next_second()` repeatedly | Own monotonic replacement time at the replaceable-edit lifecycle |

The repeated observation loop and tag-rebuilding helper remain the strongest
cross-application evidence: both independent consumers still implement them.
