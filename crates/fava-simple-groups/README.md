# fava-simple-groups

Pure multi-relay NIP-29 values for Fava. A `Group` is one opaque group id over
an application-selected, non-empty host set. Each host remains independently
authoritative for the records it served. The crate prepares ordinary queries,
events, and saved-list edits; Fava owns observation, signing, routing,
publication, delivery, cancellation, and receipts.

## Feed and publication

Content reads retain local write-store visibility and request every selected host.
They require an explicit positive result bound of at most 4,096 rows. Record
and discovery helpers carry the same explicit whole-query bound.

```rust
use fava::{EventBuilder, Kind, Query, Timestamp, Write};
use fava_simple_groups::Group;

let photos = Group::on(
    ["wss://bob.groups.example", "wss://alice.groups.example"],
    "photos",
)?;
let query = photos.events(
    Query::events()
        .kind(Kind::from_u16(9))
        .limit(50)?,
)?;
let observation = fava.observe(query).await?;
for record in &observation.current().events {
    println!("{}", record.event.content());
}

let draft = EventBuilder::new(me, Kind::from_u16(9))
    .created_at(Timestamp::from(42))
    .content("hello from both hosts")
    .build()?;
let prepared = photos.prepare(draft)?;
let write: Write = fava.to(photos.hosts())?.publish(prepared)?;
```

`prepare` is pure and kind-blind. `to` is an inert exact-route scope, and only
`publish` opens ordinary Fava work. `Group` has no publication method.

## Records and fork visibility

Relay-authored kinds 39000 through 39005 use exact `d` selection and
`OnlyRelays` authority for the configured hosts. Projection never invents a
record for an unobserved host and never lets one host speak for another.

```rust
use fava_simple_groups::GroupRecords;

let records = photos.records(GroupRecords::all())?;
let observation = fava.observe(records).await?;
let snapshot = photos.project(&observation.current())?;

for (host, metadata) in snapshot.metadata() {
    println!("{}: {:?}", host, metadata.name());
}
for (host, member) in snapshot.members() {
    println!("member {} was listed by {}", member, host);
}

if snapshot.metadata_differ() {
    let bob = fava::RelayUrl::parse("wss://bob.groups.example")?;
    if let Some(bob_view) = snapshot.at(&bob) {
        for (_, metadata) in bob_view.metadata() {
            println!("Bob host: {:?}", metadata.about());
        }
    }
}
```

`GroupSnapshot::at` is an explicit application choice. An empty host view is
only an empty positive-evidence view; it is not an absence or completeness
claim. Disagreement compares complete optional host-local records. Projection
refuses a 4,097th input row after examining at most the bound plus one; it never
silently truncates a snapshot.

## Discovery and saved lists

Discovery is an ordinary bounded `Query`. Saved rows retain their author and
exact relay URL.

```rust
use fava_simple_groups::{SavedGroup, SimpleGroups};

let query = SimpleGroups::saved_groups([me])?;
let observation = fava.observe(query).await?;
for record in &observation.current().events {
    for row in SavedGroup::from_event(&record.event)? {
        let saved = row?;
        println!("{} @ {} {:?}", saved.id(), saved.relay(), saved.name());
    }
}

let admin_query = SimpleGroups::groups_where_admin([me])?;
let member_query = SimpleGroups::groups_where_member([me])?;
let saving_authors = SimpleGroups::groups_saved_by(
    &observation.current(),
    &photos,
)?;
```

Saved group and relay changes are pure kind-10009 semantic edits. The
application supplies the author with `by`, the complete destination set with
`to`, and receives an ordinary `Write` from `publish`.

```rust
use fava::Write;
use fava_simple_groups::SimpleGroups;

let edit = SimpleGroups::save_group(&photos, Some("Photography"))?;
let write: Write = fava
    .by(me)
    .to(photos.hosts())?
    .publish(edit)?;

let relay = fava::RelayUrl::parse("wss://bob.groups.example")?;
let edit = SimpleGroups::save_relay(relay)?;
let write: Write = fava
    .by(me)
    .to(photos.hosts())?
    .publish(edit)?;
```

Save, remove, and rename preserve opaque content, foreign rows, malformed rows,
and unrelated source order. A parsed saved relay is evidence only; it does not
select acquisition or publication policy.

## Arbitrary and signed events

Unsigned preparation preserves the payload and normalizes exactly one matching
`h` row. Kinds 9002 and 9010 remain ordinary author-bearing events:

```rust
let metadata = photos.edit_metadata(metadata_draft)?;
let metadata_write: Write = fava.to(photos.hosts())?.publish(metadata)?;

let pins = photos.set_pins(pins_draft)?;
let pins_write: Write = fava.to(photos.hosts())?.publish(pins)?;
```

Any other event kind uses the same path. Signed preparation either returns the
exact original event or refuses before Fava custody:

```rust
let prepared = photos.prepare(signed.clone())?;
assert_eq!(prepared, signed);
let write: Write = fava.to(photos.hosts())?.publish(prepared)?;
```

## Cancellation and close

Publication and observation keep their ordinary Fava lifecycles:

```rust
let cancelled = fava.cancel_publication(write.receipt_id())?;
observation.close();
```

Closing an observation is idempotent. Cancellation remains scoped to the exact
ordinary receipt and its current publication phase.

## Public values

- `Group`, `GroupRecords`, `GroupSnapshot`, `SimpleGroups`, and `GroupError`.
- `GroupMetadata`, `GroupAdmins`, `GroupMembers`, `GroupRoles`,
  `GroupParticipants`, and `GroupPins` for exact relay-authored records.
- `PinnedItem`, `SavedGroup`, and `SavedRelay` for bounded typed rows.

The crate's normal dependencies are exactly `fava-query`, `fava-state`, and
`fava-write`. It owns no engine, provider, signer, router, store, publisher,
transport, runtime, observation, delivery, cancellation, or receipt state.
Universal Fava owners contain no NIP-29 kind switch, group-id branch, or
production dependency on this capability.
