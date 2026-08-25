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

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_simple_groups` |  |
| Struct | `fava_simple_groups::Group` |  |
| Method | `fava_simple_groups::Group::edit_metadata` |  |
| Method | `fava_simple_groups::Group::events` |  |
| Method | `fava_simple_groups::Group::hosts` |  |
| Method | `fava_simple_groups::Group::id` |  |
| Method | `fava_simple_groups::Group::on` |  |
| Method | `fava_simple_groups::Group::prepare` |  |
| Method | `fava_simple_groups::Group::project` |  |
| Method | `fava_simple_groups::Group::records` |  |
| Method | `fava_simple_groups::Group::set_pins` |  |
| Struct | `fava_simple_groups::GroupAdmins` |  |
| Method | `fava_simple_groups::GroupAdmins::admins` |  |
| Method | `fava_simple_groups::GroupAdmins::author` |  |
| Method | `fava_simple_groups::GroupAdmins::from_event` |  |
| Method | `fava_simple_groups::GroupAdmins::id` |  |
| Enum | `fava_simple_groups::GroupError` |  |
| Enum variant | `fava_simple_groups::GroupError::AmbiguousRecordField` |  |
| Public field | `fava_simple_groups::GroupError::AmbiguousRecordField::0` |  |
| Enum variant | `fava_simple_groups::GroupError::ConflictingGroupContext` |  |
| Enum variant | `fava_simple_groups::GroupError::ConflictingRecordId` |  |
| Enum variant | `fava_simple_groups::GroupError::DuplicateGroupContext` |  |
| Enum variant | `fava_simple_groups::GroupError::DuplicateHost` |  |
| Public field | `fava_simple_groups::GroupError::DuplicateHost::relay` |  |
| Enum variant | `fava_simple_groups::GroupError::DuplicateRecordId` |  |
| Enum variant | `fava_simple_groups::GroupError::DuplicateRecordRow` |  |
| Public field | `fava_simple_groups::GroupError::DuplicateRecordRow::tag_index` |  |
| Enum variant | `fava_simple_groups::GroupError::EmptyGroupContext` |  |
| Enum variant | `fava_simple_groups::GroupError::EmptyHosts` |  |
| Enum variant | `fava_simple_groups::GroupError::EmptyId` |  |
| Enum variant | `fava_simple_groups::GroupError::EmptyRecordId` |  |
| Enum variant | `fava_simple_groups::GroupError::Event` |  |
| Public field | `fava_simple_groups::GroupError::Event::0` |  |
| Enum variant | `fava_simple_groups::GroupError::GroupContextTooLong` |  |
| Public field | `fava_simple_groups::GroupError::GroupContextTooLong::bytes` |  |
| Public field | `fava_simple_groups::GroupError::GroupContextTooLong::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::GroupIdTooLong` |  |
| Public field | `fava_simple_groups::GroupError::GroupIdTooLong::bytes` |  |
| Public field | `fava_simple_groups::GroupError::GroupIdTooLong::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::InvalidHost` |  |
| Public field | `fava_simple_groups::GroupError::InvalidHost::0` |  |
| Enum variant | `fava_simple_groups::GroupError::InvalidRecordId` |  |
| Enum variant | `fava_simple_groups::GroupError::InvalidRecordSignature` |  |
| Enum variant | `fava_simple_groups::GroupError::MalformedRecordRow` |  |
| Public field | `fava_simple_groups::GroupError::MalformedRecordRow::reason` |  |
| Public field | `fava_simple_groups::GroupError::MalformedRecordRow::tag_index` |  |
| Enum variant | `fava_simple_groups::GroupError::MissingGroupContext` |  |
| Enum variant | `fava_simple_groups::GroupError::MissingRecordId` |  |
| Enum variant | `fava_simple_groups::GroupError::Query` |  |
| Public field | `fava_simple_groups::GroupError::Query::0` |  |
| Enum variant | `fava_simple_groups::GroupError::RecordTooLarge` |  |
| Public field | `fava_simple_groups::GroupError::RecordTooLarge::bytes` |  |
| Public field | `fava_simple_groups::GroupError::RecordTooLarge::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::RecordValueTooLong` |  |
| Public field | `fava_simple_groups::GroupError::RecordValueTooLong::bytes` |  |
| Public field | `fava_simple_groups::GroupError::RecordValueTooLong::maximum` |  |
| Public field | `fava_simple_groups::GroupError::RecordValueTooLong::tag_index` |  |
| Public field | `fava_simple_groups::GroupError::RecordValueTooLong::value_index` |  |
| Enum variant | `fava_simple_groups::GroupError::TooManyContextTags` |  |
| Public field | `fava_simple_groups::GroupError::TooManyContextTags::actual` |  |
| Public field | `fava_simple_groups::GroupError::TooManyContextTags::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::TooManyDiscoveryItems` |  |
| Public field | `fava_simple_groups::GroupError::TooManyDiscoveryItems::actual` |  |
| Public field | `fava_simple_groups::GroupError::TooManyDiscoveryItems::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::TooManyHosts` |  |
| Public field | `fava_simple_groups::GroupError::TooManyHosts::actual` |  |
| Public field | `fava_simple_groups::GroupError::TooManyHosts::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::TooManyRecordTagValues` |  |
| Public field | `fava_simple_groups::GroupError::TooManyRecordTagValues::actual` |  |
| Public field | `fava_simple_groups::GroupError::TooManyRecordTagValues::maximum` |  |
| Public field | `fava_simple_groups::GroupError::TooManyRecordTagValues::tag_index` |  |
| Enum variant | `fava_simple_groups::GroupError::TooManyRecordTags` |  |
| Public field | `fava_simple_groups::GroupError::TooManyRecordTags::actual` |  |
| Public field | `fava_simple_groups::GroupError::TooManyRecordTags::maximum` |  |
| Enum variant | `fava_simple_groups::GroupError::UnsignedRecord` |  |
| Enum variant | `fava_simple_groups::GroupError::WrongRecordKind` |  |
| Public field | `fava_simple_groups::GroupError::WrongRecordKind::actual` |  |
| Public field | `fava_simple_groups::GroupError::WrongRecordKind::expected` |  |
| Method | `<fava_simple_groups::GroupError as core::fmt::Display>::fmt` |  |
| Method | `<fava_simple_groups::GroupError as core::convert::From<fava_query::QueryError>>::from` |  |
| Method | `<fava_simple_groups::GroupError as core::convert::From<fava_write::WriteIntentError>>::from` |  |
| Method | `<fava_simple_groups::GroupError as core::convert::From<fava_write::builder::EventBuildError>>::from` |  |
| Struct | `fava_simple_groups::GroupMembers` |  |
| Method | `fava_simple_groups::GroupMembers::author` |  |
| Method | `fava_simple_groups::GroupMembers::from_event` |  |
| Method | `fava_simple_groups::GroupMembers::id` |  |
| Method | `fava_simple_groups::GroupMembers::members` |  |
| Struct | `fava_simple_groups::GroupMetadata` |  |
| Method | `fava_simple_groups::GroupMetadata::about` |  |
| Method | `fava_simple_groups::GroupMetadata::author` |  |
| Method | `fava_simple_groups::GroupMetadata::banner` |  |
| Method | `fava_simple_groups::GroupMetadata::children` |  |
| Method | `fava_simple_groups::GroupMetadata::from_event` |  |
| Method | `fava_simple_groups::GroupMetadata::has_livekit` |  |
| Method | `fava_simple_groups::GroupMetadata::id` |  |
| Method | `fava_simple_groups::GroupMetadata::is_closed` |  |
| Method | `fava_simple_groups::GroupMetadata::is_hidden` |  |
| Method | `fava_simple_groups::GroupMetadata::is_private` |  |
| Method | `fava_simple_groups::GroupMetadata::is_restricted` |  |
| Method | `fava_simple_groups::GroupMetadata::name` |  |
| Method | `fava_simple_groups::GroupMetadata::parent` |  |
| Method | `fava_simple_groups::GroupMetadata::picture` |  |
| Method | `fava_simple_groups::GroupMetadata::supported_kinds` |  |
| Struct | `fava_simple_groups::GroupParticipants` |  |
| Method | `fava_simple_groups::GroupParticipants::author` |  |
| Method | `fava_simple_groups::GroupParticipants::from_event` |  |
| Method | `fava_simple_groups::GroupParticipants::id` |  |
| Method | `fava_simple_groups::GroupParticipants::participants` |  |
| Struct | `fava_simple_groups::GroupPins` |  |
| Method | `fava_simple_groups::GroupPins::author` |  |
| Method | `fava_simple_groups::GroupPins::from_event` |  |
| Method | `fava_simple_groups::GroupPins::id` |  |
| Method | `fava_simple_groups::GroupPins::items` |  |
| Enum | `fava_simple_groups::GroupRecords` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Admins` |  |
| Enum variant | `fava_simple_groups::GroupRecords::All` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Members` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Metadata` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Participants` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Pins` |  |
| Enum variant | `fava_simple_groups::GroupRecords::Roles` |  |
| Method | `fava_simple_groups::GroupRecords::admins` |  |
| Method | `fava_simple_groups::GroupRecords::all` |  |
| Method | `fava_simple_groups::GroupRecords::members` |  |
| Method | `fava_simple_groups::GroupRecords::metadata` |  |
| Method | `fava_simple_groups::GroupRecords::participants` |  |
| Method | `fava_simple_groups::GroupRecords::pins` |  |
| Method | `fava_simple_groups::GroupRecords::roles` |  |
| Struct | `fava_simple_groups::GroupRoles` |  |
| Method | `fava_simple_groups::GroupRoles::author` |  |
| Method | `fava_simple_groups::GroupRoles::from_event` |  |
| Method | `fava_simple_groups::GroupRoles::id` |  |
| Method | `fava_simple_groups::GroupRoles::roles` |  |
| Struct | `fava_simple_groups::GroupSnapshot` |  |
| Method | `fava_simple_groups::GroupSnapshot::admin_records` |  |
| Method | `fava_simple_groups::GroupSnapshot::admins` |  |
| Method | `fava_simple_groups::GroupSnapshot::admins_differ` |  |
| Method | `fava_simple_groups::GroupSnapshot::at` |  |
| Method | `fava_simple_groups::GroupSnapshot::events` |  |
| Method | `fava_simple_groups::GroupSnapshot::hosts` |  |
| Method | `fava_simple_groups::GroupSnapshot::member_records` |  |
| Method | `fava_simple_groups::GroupSnapshot::members` |  |
| Method | `fava_simple_groups::GroupSnapshot::members_differ` |  |
| Method | `fava_simple_groups::GroupSnapshot::metadata` |  |
| Method | `fava_simple_groups::GroupSnapshot::metadata_differ` |  |
| Method | `fava_simple_groups::GroupSnapshot::participant_records` |  |
| Method | `fava_simple_groups::GroupSnapshot::participants_differ` |  |
| Method | `fava_simple_groups::GroupSnapshot::pin_records` |  |
| Method | `fava_simple_groups::GroupSnapshot::pins_differ` |  |
| Method | `fava_simple_groups::GroupSnapshot::role_records` |  |
| Method | `fava_simple_groups::GroupSnapshot::roles_differ` |  |
| Enum | `fava_simple_groups::PinnedItem` |  |
| Enum variant | `fava_simple_groups::PinnedItem::Address` |  |
| Public field | `fava_simple_groups::PinnedItem::Address::0` |  |
| Enum variant | `fava_simple_groups::PinnedItem::Event` |  |
| Public field | `fava_simple_groups::PinnedItem::Event::0` |  |
| Struct | `fava_simple_groups::SavedGroup` |  |
| Method | `fava_simple_groups::SavedGroup::author` |  |
| Method | `fava_simple_groups::SavedGroup::from_event` |  |
| Method | `fava_simple_groups::SavedGroup::id` |  |
| Method | `fava_simple_groups::SavedGroup::name` |  |
| Method | `fava_simple_groups::SavedGroup::relay` |  |
| Struct | `fava_simple_groups::SavedRelay` |  |
| Method | `fava_simple_groups::SavedRelay::author` |  |
| Method | `fava_simple_groups::SavedRelay::from_event` |  |
| Method | `fava_simple_groups::SavedRelay::relay` |  |
| Struct | `fava_simple_groups::SimpleGroups` |  |
| Method | `fava_simple_groups::SimpleGroups::groups_saved_by` |  |
| Method | `fava_simple_groups::SimpleGroups::groups_where_admin` |  |
| Method | `fava_simple_groups::SimpleGroups::groups_where_member` |  |
| Method | `fava_simple_groups::SimpleGroups::materializer` |  |
| Method | `fava_simple_groups::SimpleGroups::remove_group` |  |
| Method | `fava_simple_groups::SimpleGroups::remove_relay` |  |
| Method | `fava_simple_groups::SimpleGroups::rename_saved_group` |  |
| Method | `fava_simple_groups::SimpleGroups::save_group` |  |
| Method | `fava_simple_groups::SimpleGroups::save_relay` |  |
| Method | `fava_simple_groups::SimpleGroups::saved_groups` |  |
| Method | `fava_simple_groups::SimpleGroups::saved_relays` |  |
<!-- END crate-readme-api inventory -->
