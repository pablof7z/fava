# A Fava NIP-29 simple-groups app

This is a runnable application, not part of the `fava-simple-groups` protocol
crate. It assembles a concrete Fava engine, chooses relays and identities,
opens live views, publishes events, and decides how to present NIP-29 data.
`fava-simple-groups` turns a `SimpleGroup` into ordinary Fava queries and
extends the generic `EventBuilder` with the exact group `h` tag and local route.

The example deliberately makes the ownership boundary visible:

| The application owns | Fava owns | `fava-simple-groups` owns |
| --- | --- | --- |
| group ids, selected relay URLs, identities, provider selection, UI policy, and which relay-local state to show | observations, sockets, cache/store, signing, exact-route delivery, receipts, cancellation, and event provenance | NIP-29 `h`/`d` query lowering, fluent group context composition, event-local state decoding, and kind-10009 edits |

## The shape of an app

An app first obtains a group id and a non-empty set of group relays through its
own product flow. It makes that selection explicit:

```rust
let group = SimpleGroup::new("photos", vec![relay_a, relay_b])?;
```

It opens normal Fava observations. Content is constrained to `h = "photos"`
and acquired from the selected relays; group state is constrained to `d =
"photos"` and the NIP-29 state kinds. Both results are ordinary
`QuerySnapshot`s containing `EventRecord`s, including each relay's evidence.

```rust
let content = fava
    .observe(group.events(Query::events().kinds([Kind::TextNote])?)?)
    .await?;
let state = fava
    .observe(group.meta_events(SimpleGroupStateEventKind::ALL)?)
    .await?;
```

The application can decode each state event it chooses to render. It must keep
relay-local disagreement visible or apply its own presentation policy; the
decoder does not pronounce one relay authoritative.

```rust
for record in &state.current().events {
    if let Ok(metadata) = SimpleGroupMetadata::from_event(record.event()) {
        println!("{:?}", metadata.name());
    }
}
```

To send content, the app composes the selected group into the concrete builder.
That adds its exact `h` tag and local route; Fava signs with the
application-selected account and tracks the resulting write receipt.

```rust
let builder = EventBuilder::new(me, Kind::TextNote)
    .content("hello")
    .simple_group(&group)?;
let write = fava.publish(builder)?;
let receipt = write.settled(at_least(1)?).await?;
```

NIP-29 group-management events and kind-10009 saved-group-list edits are also
ordinary unsigned values. The example uses the typed constructors, publishes
them through the same Fava path, and observes the resulting relay state. It
does not create a second NIP-29 runtime, socket, or publication lifecycle.

## Management is checked, not just called

Publishing a management event and printing the receipt proves the bytes left
the process. The example goes further for all nine typed constructors, and
fails loudly rather than printing a value nobody reads:

- **Acknowledged.** Each publication settles on `all_terminal()` and then
  requires relay acknowledgement evidence. A rejection therefore surfaces the
  relay's own message instead of an unexplained timeout.
- **Read back.** Each management event is waited for in the live
  `SimpleGroup::events` observation and checked there — kind and the tags that
  carry its meaning (`h`, `p`, `e`, `name`, `code`). Presence alone is not
  accepted: the record must carry at least one relay occurrence, so the
  application's own local write cannot masquerade as relay confirmation.
- **Effect on group state.** After the sequence, the example states the
  membership and metadata it expects and waits for exactly that: the name and
  closed flag `edit_metadata` set, Alice holding the primary role in the derived
  admin list, Alice and Bob in the derived member list, and Carol no longer in
  it. Waiting for the *expected* state rather than the first state that decodes
  is what makes `remove_user` and `leave_group` verifiable at all.
- **Refusals.** Each constructor is then called again without the authority to
  make it stick — `edit_metadata`, `invite` and `leave_group` by an outsider, a
  `join_request` to the closed group with no invite code, `delete_group` by a
  plain member, a duplicate `create_group`, a `put_user` that changes nothing, a
  `remove_user` targeting someone already gone, and a `delete_event` for an
  event the relay never stored. Each prints the expectation next to the relay's
  verbatim refusal.
- **Deletion.** `delete_event` is confirmed by the relay refusing to store the
  deleted event again; `delete_group` by the relay no longer knowing the group.

A generic relay stores management events without deriving NIP-29 state, and
does not enforce NIP-29 authority either — it would acknowledge every refusal
above. The example detects that from the absent derived state and prints each
check it did not run, by name, under a `SKIPPED` heading, finishing with
`PASS (partial)`. A relay that *does* derive state but derives the wrong state
is a failure, not a skip.

## Run it

Build from the repository root:

```sh
cargo build --manifest-path examples/simple-groups/Cargo.toml --locked
```

Against an already-running relay, the example proves management-event and
content publication. A generic relay normally stores those events but does not
derive NIP-29 state events:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml -- \
  --relay ws://127.0.0.1:8080
```

For the full lifecycle, point it at a Croissant binary. The example starts an
isolated NIP-29 group relay whose owner is its generated Alice key. Pass a
regular user relay for kind-10009 saved lists, because a group relay can reject
non-group preference events:

```sh
cargo run --manifest-path examples/simple-groups/Cargo.toml -- \
  --spawn-croissant /path/to/croissant \
  --saved-relay ws://127.0.0.1:8080
```

The app uses disposable Alice, Bob, Carol, and Dave keys — Alice administers the
group, Bob is invited into it, Carol joins, leaves, is re-added and removed, and
Dave never joins so his calls exercise the refusal paths. It creates a unique
group id, opens the content/state/saved-list observations before writing, then
creates and edits the group, sends content, asserts the derived group state,
exercises every management refusal, runs the saved-list edits, and removes its
state. It stops the child Croissant process and removes its temporary data on
exit.

## What this does not decide

This is an integration example, not an application framework. It does not
provide group discovery, UI, moderation, conflict resolution, relay trust
policy, or persistent account/session handling. Those remain application
decisions. The independent controlled-relay acceptance proof remains in
[`apps/canary`](../../apps/canary), whose job is verification rather than
teaching application structure.
