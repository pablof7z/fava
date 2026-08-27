# A Fava NIP-29 simple-groups app

This is a runnable application, not part of the `fava-simple-groups` protocol
crate. It assembles a concrete Fava engine, chooses relays and identities,
opens live views, publishes events, and decides how to present NIP-29 data.
`fava-simple-groups` only turns a `SimpleGroup` into ordinary Fava queries and
an unsigned event with the exact group `h` tag.

The example deliberately makes the ownership boundary visible:

| The application owns | Fava owns | `fava-simple-groups` owns |
| --- | --- | --- |
| group ids, selected relay URLs, identities, provider selection, UI policy, and which relay-local state to show | observations, sockets, cache/store, signing, exact-route delivery, receipts, cancellation, and event provenance | NIP-29 `h`/`d` query lowering, pure content preparation, event-local state decoding, and kind-10009 edits |

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

To send content, the app builds an unsigned event, asks the group to add its
`h` tag, and explicitly routes it to exactly that group's relays. Fava signs
with the application-selected account and tracks the resulting write receipt.

```rust
let draft = EventBuilder::new(me, Kind::TextNote).content("hello").build()?;
let prepared = group.prepare(draft)?;
let write = fava.to(group.relays())?.publish(prepared)?;
let receipt = write.settled(at_least(1)?).await?;
```

NIP-29 group-management events and kind-10009 saved-group-list edits are also
ordinary unsigned values. The example uses the typed constructors, publishes
them through the same Fava path, and observes the resulting relay state. It
does not create a second NIP-29 runtime, socket, or publication lifecycle.

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

The app uses disposable Alice, Bob, and Carol keys, creates a unique group id,
opens the content/state/saved-list observations before writing, then creates and
edits the group, sends content, exercises saved-list edits, and removes its
state. It stops the child Croissant process and removes its temporary data on
exit.

## What this does not decide

This is an integration example, not an application framework. It does not
provide group discovery, UI, moderation, conflict resolution, relay trust
policy, or persistent account/session handling. Those remain application
decisions. The independent controlled-relay acceptance proof remains in
[`apps/canary`](../../apps/canary), whose job is verification rather than
teaching application structure.
