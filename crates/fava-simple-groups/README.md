# fava-simple-groups

Pure multi-relay NIP-29 values for Fava. A `SimpleGroup` is one opaque simple
group id over an application-selected, non-empty host set. Each host remains
independently authoritative for the records it served. The crate prepares
ordinary queries, events, and saved-list edits; Fava owns observation,
signing, routing, publication, delivery, cancellation, and receipts.

## Feed and publication

Content reads retain local write-store visibility and request every selected host.
They require an explicit positive result bound of at most 4,096 rows. Record
and discovery helpers carry the same explicit whole-query bound.

```rust
use fava::{EventBuilder, Kind, Query, Timestamp, Write};
use fava_simple_groups::SimpleGroup;

let photos = SimpleGroup::on(
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
    println!("{}", record.event().content());
}

let draft = EventBuilder::new(me, Kind::from_u16(9))
    .created_at(Timestamp::from(42))
    .content("hello from both hosts")
    .build()?;
let prepared = photos.prepare(draft)?;
let write: Write = fava.to(photos.hosts())?.publish(prepared)?;
```

`prepare` is pure and kind-blind. `to` is an inert exact-route scope, and only
`publish` opens ordinary Fava work. `SimpleGroup` has no publication method.

## Records and fork visibility

Relay-authored kinds 39000 through 39005 use exact `d` selection and
`OnlyRelays` authority. One bounded query is returned for each configured host,
in configured-host order. Query owns same-id aggregation and coordinate winner
selection; typed simple-group values decode one already-selected event.

<a id="exact-host-records-1"></a>
```rust
//! Exact-host record query proof through the public API.

use fava_query::{QueryAcquisition, ResultAuthority};
use fava_simple_groups::{SimpleGroup, SimpleGroupRecords};
use nostr::types::RelayUrl;
use std::collections::BTreeSet;

#[test]
fn exact_host_records_1_builds_one_query_per_configured_host()
-> Result<(), Box<dyn std::error::Error>> {
    let b = RelayUrl::parse("wss://b.groups.example")?;
    let a = RelayUrl::parse("wss://a.groups.example")?;
    let group = SimpleGroup::on([b.clone(), a.clone()], "photos")?;
    let queries = group.records(SimpleGroupRecords::all())?;
    assert_eq!(queries.len(), 2);
    assert_eq!(
        queries.iter().map(|(host, _)| host).collect::<Vec<_>>(),
        [&b, &a]
    );
    for (host, query) in queries {
        let only = BTreeSet::from([host]);
        assert_eq!(
            query.source().acquisition(),
            &QueryAcquisition::Explicit(only.clone())
        );
        assert_eq!(
            query.source().authority(),
            &ResultAuthority::OnlyRelays(only)
        );
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096)
        );
    }
    Ok(())
}

#[test]
fn exact_host_records_1_preserves_all_256_exact_hosts() -> Result<(), Box<dyn std::error::Error>> {
    let hosts = (0..256)
        .map(|index| RelayUrl::parse(&format!("wss://host-{index}.groups.example")))
        .collect::<Result<Vec<_>, _>>()?;
    let group = SimpleGroup::on(hosts.clone(), "photos")?;
    let queries = group.records(SimpleGroupRecords::all())?;
    assert_eq!(queries.len(), 256);
    for ((host, query), expected) in queries.into_iter().zip(hosts) {
        assert_eq!(host, expected);
        let only = BTreeSet::from([expected]);
        assert_eq!(
            query.source().acquisition(),
            &QueryAcquisition::Explicit(only.clone())
        );
        assert_eq!(
            query.source().authority(),
            &ResultAuthority::OnlyRelays(only)
        );
        assert_eq!(
            query.result_limit().map(std::num::NonZeroUsize::get),
            Some(4_096)
        );
    }
    Ok(())
}
```

Applications open the returned queries as independent ordinary observations,
decode records with the existing typed `from_event(record.event())` parsers,
and compare decoded values when fork visibility matters. The query vector is
bounded by `SimpleGroup`'s 256-host construction bound; each query has a 4,096
result bound.

## Discovery and saved lists

Discovery is an ordinary bounded `Query`. Saved rows retain their author and
exact relay URL.

```rust
use fava_simple_groups::{SavedSimpleGroup, SimpleGroups};

let query = SimpleGroups::saved_simple_groups([me])?;
let observation = fava.observe(query).await?;
for record in &observation.current().events {
    for row in SavedSimpleGroup::from_event(record.event())? {
        let saved = row?;
        println!("{} @ {} {:?}", saved.id(), saved.relay(), saved.name());
    }
}

let admin_query = SimpleGroups::simple_groups_where_admin([me])?;
let member_query = SimpleGroups::simple_groups_where_member([me])?;
let saving_authors = SimpleGroups::simple_groups_saved_by(
    &observation.current(),
    &photos,
)?;
```

Saved simple group and relay changes are pure kind-10009 semantic edits. The
application supplies the author with `by`, the complete destination set with
`to`, and receives an ordinary `Write` from `publish`.

```rust
use fava::Write;
use fava_simple_groups::SimpleGroups;

let edit = SimpleGroups::save_simple_group(&photos, Some("Photography"))?;
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

- `SimpleGroup`, `SimpleGroupRecords`, `SimpleGroups`, and `SimpleGroupError`.
- `SimpleGroupMetadata`, `SimpleGroupAdmins`, `SimpleGroupMembers`, `SimpleGroupRoles`,
  `SimpleGroupParticipants`, and `SimpleGroupPins` for exact relay-authored records.
- `PinnedItem`, `SavedSimpleGroup`, and `SavedRelay` for bounded typed rows.

The crate's normal dependencies are exactly `fava-query`, `fava-relay`,
`fava-state`, and `fava-write`. It owns no engine, provider, signer, router, store, publisher,
transport, runtime, observation, delivery, cancellation, or receipt state.
Universal Fava owners contain no NIP-29 kind switch, simple-group-id branch, or
production dependency on this capability.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
| Kind | Item | Description |
| --- | --- | --- |
| Module | `fava_simple_groups` |  |
| Enum | `fava_simple_groups::PinnedItem` |  |
| Enum variant | `fava_simple_groups::PinnedItem::Address` |  |
| Public field | `fava_simple_groups::PinnedItem::Address::0` |  |
| Enum variant | `fava_simple_groups::PinnedItem::Event` |  |
| Public field | `fava_simple_groups::PinnedItem::Event::0` |  |
| Struct | `fava_simple_groups::SavedRelay` |  |
| Method | `fava_simple_groups::SavedRelay::author` |  |
| Method | `fava_simple_groups::SavedRelay::from_event` |  |
| Method | `fava_simple_groups::SavedRelay::relay` |  |
| Struct | `fava_simple_groups::SavedSimpleGroup` |  |
| Method | `fava_simple_groups::SavedSimpleGroup::author` |  |
| Method | `fava_simple_groups::SavedSimpleGroup::from_event` |  |
| Method | `fava_simple_groups::SavedSimpleGroup::id` |  |
| Method | `fava_simple_groups::SavedSimpleGroup::name` |  |
| Method | `fava_simple_groups::SavedSimpleGroup::relay` |  |
| Struct | `fava_simple_groups::SimpleGroup` |  |
| Method | `fava_simple_groups::SimpleGroup::edit_metadata` |  |
| Method | `fava_simple_groups::SimpleGroup::events` |  |
| Method | `fava_simple_groups::SimpleGroup::hosts` |  |
| Method | `fava_simple_groups::SimpleGroup::id` |  |
| Method | `fava_simple_groups::SimpleGroup::on` |  |
| Method | `fava_simple_groups::SimpleGroup::prepare` |  |
| Method | `fava_simple_groups::SimpleGroup::records` |  |
| Method | `fava_simple_groups::SimpleGroup::set_pins` |  |
| Struct | `fava_simple_groups::SimpleGroupAdmins` |  |
| Method | `fava_simple_groups::SimpleGroupAdmins::admins` |  |
| Method | `fava_simple_groups::SimpleGroupAdmins::author` |  |
| Method | `fava_simple_groups::SimpleGroupAdmins::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupAdmins::id` |  |
| Enum | `fava_simple_groups::SimpleGroupError` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::AmbiguousRecordField` |  |
| Public field | `fava_simple_groups::SimpleGroupError::AmbiguousRecordField::0` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::ConflictingRecordId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::ConflictingSimpleGroupContext` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::DuplicateHost` |  |
| Public field | `fava_simple_groups::SimpleGroupError::DuplicateHost::relay` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::DuplicateRecordId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::DuplicateRecordRow` |  |
| Public field | `fava_simple_groups::SimpleGroupError::DuplicateRecordRow::tag_index` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::DuplicateSimpleGroupContext` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::EmptyHosts` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::EmptyId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::EmptyRecordId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::EmptySimpleGroupContext` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::Event` |  |
| Public field | `fava_simple_groups::SimpleGroupError::Event::0` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::InvalidHost` |  |
| Public field | `fava_simple_groups::SimpleGroupError::InvalidHost::0` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::InvalidRecordId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::InvalidRecordSignature` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::MalformedRecordRow` |  |
| Public field | `fava_simple_groups::SimpleGroupError::MalformedRecordRow::reason` |  |
| Public field | `fava_simple_groups::SimpleGroupError::MalformedRecordRow::tag_index` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::MissingRecordId` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::MissingSimpleGroupContext` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::Query` |  |
| Public field | `fava_simple_groups::SimpleGroupError::Query::0` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::RecordTooLarge` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordTooLarge::bytes` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordTooLarge::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::RecordValueTooLong` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordValueTooLong::bytes` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordValueTooLong::maximum` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordValueTooLong::tag_index` |  |
| Public field | `fava_simple_groups::SimpleGroupError::RecordValueTooLong::value_index` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::SimpleGroupContextTooLong` |  |
| Public field | `fava_simple_groups::SimpleGroupError::SimpleGroupContextTooLong::bytes` |  |
| Public field | `fava_simple_groups::SimpleGroupError::SimpleGroupContextTooLong::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::SimpleGroupIdTooLong` |  |
| Public field | `fava_simple_groups::SimpleGroupError::SimpleGroupIdTooLong::bytes` |  |
| Public field | `fava_simple_groups::SimpleGroupError::SimpleGroupIdTooLong::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::TooManyContextTags` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyContextTags::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyContextTags::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::TooManyDiscoveryItems` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyDiscoveryItems::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyDiscoveryItems::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::TooManyHosts` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyHosts::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyHosts::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::TooManyRecordTagValues` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyRecordTagValues::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyRecordTagValues::maximum` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyRecordTagValues::tag_index` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::TooManyRecordTags` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyRecordTags::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::TooManyRecordTags::maximum` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::UnsignedRecord` |  |
| Enum variant | `fava_simple_groups::SimpleGroupError::WrongRecordKind` |  |
| Public field | `fava_simple_groups::SimpleGroupError::WrongRecordKind::actual` |  |
| Public field | `fava_simple_groups::SimpleGroupError::WrongRecordKind::expected` |  |
| Method | `<fava_simple_groups::SimpleGroupError as core::fmt::Display>::fmt` |  |
| Method | `<fava_simple_groups::SimpleGroupError as core::convert::From<fava_query::QueryError>>::from` |  |
| Method | `<fava_simple_groups::SimpleGroupError as core::convert::From<fava_write::WriteIntentError>>::from` |  |
| Method | `<fava_simple_groups::SimpleGroupError as core::convert::From<fava_write::builder::EventBuildError>>::from` |  |
| Struct | `fava_simple_groups::SimpleGroupMembers` |  |
| Method | `fava_simple_groups::SimpleGroupMembers::author` |  |
| Method | `fava_simple_groups::SimpleGroupMembers::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupMembers::id` |  |
| Method | `fava_simple_groups::SimpleGroupMembers::members` |  |
| Struct | `fava_simple_groups::SimpleGroupMetadata` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::about` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::author` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::banner` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::children` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::has_livekit` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::id` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::is_closed` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::is_hidden` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::is_private` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::is_restricted` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::name` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::parent` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::picture` |  |
| Method | `fava_simple_groups::SimpleGroupMetadata::supported_kinds` |  |
| Struct | `fava_simple_groups::SimpleGroupParticipants` |  |
| Method | `fava_simple_groups::SimpleGroupParticipants::author` |  |
| Method | `fava_simple_groups::SimpleGroupParticipants::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupParticipants::id` |  |
| Method | `fava_simple_groups::SimpleGroupParticipants::participants` |  |
| Struct | `fava_simple_groups::SimpleGroupPins` |  |
| Method | `fava_simple_groups::SimpleGroupPins::author` |  |
| Method | `fava_simple_groups::SimpleGroupPins::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupPins::id` |  |
| Method | `fava_simple_groups::SimpleGroupPins::items` |  |
| Enum | `fava_simple_groups::SimpleGroupRecords` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Admins` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::All` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Members` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Metadata` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Participants` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Pins` |  |
| Enum variant | `fava_simple_groups::SimpleGroupRecords::Roles` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::admins` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::all` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::members` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::metadata` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::participants` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::pins` |  |
| Method | `fava_simple_groups::SimpleGroupRecords::roles` |  |
| Struct | `fava_simple_groups::SimpleGroupRoles` |  |
| Method | `fava_simple_groups::SimpleGroupRoles::author` |  |
| Method | `fava_simple_groups::SimpleGroupRoles::from_event` |  |
| Method | `fava_simple_groups::SimpleGroupRoles::id` |  |
| Method | `fava_simple_groups::SimpleGroupRoles::roles` |  |
| Struct | `fava_simple_groups::SimpleGroups` |  |
| Method | `fava_simple_groups::SimpleGroups::materializer` |  |
| Method | `fava_simple_groups::SimpleGroups::remove_relay` |  |
| Method | `fava_simple_groups::SimpleGroups::remove_simple_group` |  |
| Method | `fava_simple_groups::SimpleGroups::rename_saved_simple_group` |  |
| Method | `fava_simple_groups::SimpleGroups::save_relay` |  |
| Method | `fava_simple_groups::SimpleGroups::save_simple_group` |  |
| Method | `fava_simple_groups::SimpleGroups::saved_relays` |  |
| Method | `fava_simple_groups::SimpleGroups::saved_simple_groups` |  |
| Method | `fava_simple_groups::SimpleGroups::simple_groups_saved_by` |  |
| Method | `fava_simple_groups::SimpleGroups::simple_groups_where_admin` |  |
| Method | `fava_simple_groups::SimpleGroups::simple_groups_where_member` |  |
<!-- END crate-readme-api inventory -->
