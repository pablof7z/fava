# fava-simple-groups

Pure NIP-29 simple-group semantics for Fava. A `SimpleGroup` owns one opaque id
and a normalized, non-empty sequence of application-selected relays. This crate
lowers that value into ordinary queries, self-routed management event builders,
decoded individual state events, and pure kind-10009 edits. Fava retains observation,
provenance, bounds, signing, routing, publication, cancellation, and receipts.
Construction accepts a finite owned `Vec<RelayUrl>` and returns a public typed
error for exactly an empty id or empty vector. Every non-empty id remains
opaque, arbitrary iterators are not accepted, and later relay duplicates
collapse in first-occurrence order. URL parsing remains with `RelayUrl`.

## Group content

`events` must preserve an ordinary query, constrain the lowercase `h` axis to
exactly the group id without broadening an existing `h` selection, and add
acquisition from every group relay. It delegates exact narrowing to
query-owned `Query::intersect_tag_values`; disjoint axes remain present-empty
and match nothing. This crate does not inspect generic query internals,
validate query state, or translate the owning `QueryError`.

```rust,ignore
use fava::{EventBuilder, Kind, Query, RelayUrl, Timestamp, Write};
use fava_simple_groups::{SimpleGroup, SimpleGroupEventBuilder};

let bob = RelayUrl::parse("wss://bob.groups.example")?;
let alice = RelayUrl::parse("wss://alice.groups.example")?;
let photos = SimpleGroup::new("photos", vec![bob, alice])?;
let query = photos.events(Query::events().kinds([Kind::from_u16(9)])?)?;
let observation = fava.observe(query).await?;

let builder = EventBuilder::new(Kind::from_u16(9))
    .created_at(Timestamp::from(42))
    .content("hello from both relays")
    .simple_group(&photos)?;
let write: Write = fava.by(me).publish(builder)?;
```

`simple_group` is pure and kind-blind. It appends one exact two-cell `h` tag
when that exact context is absent and accumulates the group's relays as local
publication intent. Malformed, repeated, extended, and unrelated tags survive.
Calling it for several groups returns the same concrete `EventBuilder`, with
all distinct group contexts and the first-occurrence relay union. Signing still
belongs to Fava.

## Relay-generated state

`meta_events` builds an exact-`d`, `OnlyRelays` query for kinds 39000 through
39005 by delegating the supplied set to `Query::kinds`. It applies no private
result limit or validation and returns exact `QueryError` values. The returned
value is an ordinary `QuerySnapshot`; provenance and relay-local selection
remain generic Fava/application responsibilities.

```rust,ignore
use fava_simple_groups::{SimpleGroupMetadata, SimpleGroupStateEventKind};

let query = photos.meta_events([
    SimpleGroupStateEventKind::Metadata,
    SimpleGroupStateEventKind::Members,
])?;
let observation = fava.observe(query).await?;

for record in &observation.current().events {
    if let Ok(metadata) = SimpleGroupMetadata::from_event(&record.event) {
        println!("{}: {:?}", metadata.id(), metadata.name());
    }
}
```

Each decoder checks its event kind and the first `d` tag’s first value, then
decodes only its semantic tags. Unknown tags and unused extra values are ignored.
Repeated entries retain source order. A malformed entry becomes a local `Result`
error without erasing valid siblings. Decoders establish semantics, not trust or
provenance.

## Management events

Nine NIP-29 typed constructors build self-routed, authorless event builders:
`create_group`, `edit_metadata`, `invite`, `join_request`, `put_user`,
`remove_user`, `delete_event`, `delete_group`, and `leave_group`. Each returns
`Result<EventBuilder, WriteIntentError>` with the group's relays already
embedded as explicit routing. Callers supply the author at publish time with
`fava.by(author).publish(builder)`.

`invite` takes an exact required `code` string and emits `h` and `code` tags
only; NIP-29 upstream is authoritative and `p` tags belong to relay-acceptance
policy, not this constructor. `join_request` takes an optional `code`; an
optional reason stays ordinary builder `.content(...)` on the returned builder.
`put_user` and `remove_user` take ordered target slices and emit one `p` tag per
key; empty slices emit no `p` tags.

No management-local count bound, relay-policy value, or new error type is
introduced. Universal tag and byte limits remain exclusively `fava-write`'s
`EventBuildError` behavior after the completed body is constructed.

## Saved group lists

Kind 10009 is queried, decoded, and edited at the crate root. One
`SavedGroupList` represents one event; `simple_groups()` and `relays()` retain
entry order, repetitions, and entry-local failures.

```rust,ignore
use fava_simple_groups::{SavedGroupList, SimpleGroups, save_simple_group, saved_group_lists};

let observation = fava.observe(saved_group_lists([me])?).await?;
for record in &observation.current().events {
    let list = SavedGroupList::from_event(&record.event)?;
    for entry in list.simple_groups() {
        let saved = entry.as_ref()?;
        println!("{} @ {}", saved.id(), saved.relay());
    }
}

let fava = Fava::builder()
    .with_simple_groups()
    // configure the ordinary Fava owners
    .build()?;
let edit = save_simple_group(&photos, Some("Photography"))?;
let write: Write = fava.by(me).to(photos.relays())?.publish(edit)?;
```

Save, remove, and rename edits preserve opaque content, foreign tags, malformed
entries, unused trailing values, and unrelated order. `with_simple_groups()` is
the only door out of this crate for its private edit-codec applier; it does not
own author selection, routing, storage, signing, or delivery.

## Ownership boundary

Normal dependencies are exactly `fava-query`, `fava-state`, `fava-write`, and
`nostr`. There is no provider, snapshot,
projection, disagreement, verification, management-event, discovery-policy, or
private-bounds subsystem in this crate.
