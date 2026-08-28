# fava-simple-groups

Pure NIP-29 simple-group semantics for Fava. A `SimpleGroup` owns one opaque id
and a normalized, non-empty sequence of application-selected relays. This crate
lowers that value into ordinary queries and unsigned events, decodes individual
state events, and supplies pure kind-10009 edits. Fava retains observation,
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

let builder = EventBuilder::new(me, Kind::from_u16(9))
    .created_at(Timestamp::from(42))
    .content("hello from both relays")
    .simple_group(&photos)?;
let write: Write = fava.publish(builder)?;
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

## Saved group lists

Kind 10009 is queried, decoded, and edited at the crate root. One
`SavedGroupList` represents one event; `simple_groups()` and `relays()` retain
entry order, repetitions, and entry-local failures.

```rust,ignore
use fava_simple_groups::{
    SavedGroupList, save_simple_group, saved_group_list_materializer,
    saved_group_lists,
};

let observation = fava.observe(saved_group_lists([me])?).await?;
for record in &observation.current().events {
    let list = SavedGroupList::from_event(&record.event)?;
    for entry in list.simple_groups() {
        let saved = entry.as_ref()?;
        println!("{} @ {}", saved.id(), saved.relay());
    }
}

let fava = Fava::builder()
    .materializers([saved_group_list_materializer()])
    // configure the ordinary Fava owners
    .build()?;
let edit = save_simple_group(&photos, Some("Photography"))?;
let write: Write = fava.by(me).to(photos.relays())?.publish(edit)?;
```

Save, remove, and rename edits preserve opaque content, foreign tags, malformed
entries, unused trailing values, and unrelated order. The materializer is private
edit-codec plumbing for Fava’s generic semantic-write lifecycle; it does not own
author selection, routing, storage, signing, or delivery.

## Ownership boundary

Normal dependencies are exactly `fava-query`, `fava-state`, `fava-write`, and
`nostr`. There is no provider, snapshot,
projection, disagreement, verification, management-event, discovery-policy, or
private-bounds subsystem in this crate.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_simple_groups` (Module)

Pure simple-group domain composition for NIP-29 and the kind-10009 Simple Group List; owns no engine, verifier, bound policy, router, store, publication, projection, or lifecycle.
<!-- api-item {"kind":"Module","item":"fava_simple_groups","signature":"pub mod fava_simple_groups","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Responsibility; crates/fava-simple-groups/src/lib.rs; docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md:1308-1336","example":"MOD-1"} -->
Example coverage: [MOD-1](#mod-1).

| Item | Purpose |
| --- | --- |
| **`create_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::create_group","signature":"pub fn fava_simple_groups::create_group(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9007"} --> | Builds an unsigned kind-9007 create-group event with the exact group id in one `h` tag. It does not publish the event or decide relay authorization. |
| **`delete_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::delete_event","signature":"pub fn fava_simple_groups::delete_event(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup, &nostr::event::id::EventId) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9005"} --> | Builds an unsigned kind-9005 group event-deletion request with exact `h` and `e` tags. Relay acceptance and deletion effects remain outside this constructor. |
| **`delete_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::delete_group","signature":"pub fn fava_simple_groups::delete_group(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9008"} --> | Builds an unsigned kind-9008 delete-group request carrying the exact group id. It does not authorize, publish, or apply deletion. |
| **`edit_metadata`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::edit_metadata","signature":"pub fn fava_simple_groups::edit_metadata(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup, &fava_simple_groups::MetadataEdit) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9002"} --> | Builds an unsigned kind-9002 metadata replacement body from the exact optional fields, visibility, and access supplied by the caller. Omitted fields emit no corresponding tag. |
| **`invite`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::invite","signature":"pub fn fava_simple_groups::invite(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup, &[nostr::key::public_key::PublicKey], &nostr::types::url::RelayUrl) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9009"} --> | Builds one unsigned kind-9009 invitation with exact `h`, one invitee `p` tag for every supplied user key, and a relay tag. It does not publish the invitation or establish membership. |
| **`join_request`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::join_request","signature":"pub fn fava_simple_groups::join_request(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9021"} --> | Builds an unsigned kind-9021 join request carrying the exact group id. Optional invitation context and publication remain caller composition. |
| **`leave_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::leave_group","signature":"pub fn fava_simple_groups::leave_group(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9022"} --> | Builds an unsigned kind-9022 leave request for the author and exact group id. Relay acceptance and membership state remain outside this constructor. |
| **`put_user`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::put_user","signature":"pub fn fava_simple_groups::put_user(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup, &[nostr::key::public_key::PublicKey], &[&str]) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9000"} --> | Builds one unsigned kind-9000 put-user event with the exact group id and one `p` tag, including the caller-supplied roles, for every supplied user key. |
| **`remove_saved_relay`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::remove_saved_relay","signature":"pub fn fava_simple_groups::remove_saved_relay(nostr::types::url::RelayUrl) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Produces a kind-10009 edit removing every semantic `r` tag for the exact relay. The URL does not acquire routing or publication meaning.<br><br>Example: [MOD-1](#mod-1). |
| **`remove_saved_simple_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::remove_saved_simple_group","signature":"pub fn fava_simple_groups::remove_saved_simple_group(&fava_simple_groups::SimpleGroup) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Produces a kind-10009 edit removing every semantic `group` tag whose id and parsed relay match a selected relay. The name states removal from saved state, not deletion of the group.<br><br>Example: [MOD-1](#mod-1). |
| **`remove_user`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::remove_user","signature":"pub fn fava_simple_groups::remove_user(nostr::key::public_key::PublicKey, &fava_simple_groups::SimpleGroup, &[nostr::key::public_key::PublicKey]) -> core::result::Result<nostr::event::unsigned::UnsignedEvent, fava_write::builder::EventBuildError>","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9001"} --> | Builds one unsigned kind-9001 remove-user event with the exact group id and one `p` tag for every supplied user key. It does not authorize, publish, or apply membership removal. |
| **`rename_saved_simple_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::rename_saved_simple_group","signature":"pub fn fava_simple_groups::rename_saved_simple_group(&fava_simple_groups::SimpleGroup, &str) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Produces a kind-10009 edit setting the display name for every selected id/relay entry, retaining first positions, removing later matches, and appending absent selected relays. Preserves unused trailing values on retained tags.<br><br>Example: [MOD-1](#mod-1). |
| **`save_relay`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::save_relay","signature":"pub fn fava_simple_groups::save_relay(nostr::types::url::RelayUrl) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Produces a kind-10009 edit ensuring one semantic `r` tag for the exact inert relay URL, keeping the first match, removing later matches, and appending when absent.<br><br>Example: [MOD-1](#mod-1). |
| **`save_simple_group`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::save_simple_group","signature":"pub fn fava_simple_groups::save_simple_group(&fava_simple_groups::SimpleGroup, core::option::Option<&str>) -> core::result::Result<fava_write::edit::ReplaceableEventEdit, fava_write::WriteIntentError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Produces a kind-10009 edit ensuring one semantic `group` tag for every normalized relay. Keeps the first existing match unchanged, removes later matches, and appends missing relays in group order.<br><br>Example: [MOD-1](#mod-1). |
| **`saved_group_list_materializer`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::saved_group_list_materializer","signature":"pub fn fava_simple_groups::saved_group_list_materializer() -> alloc::sync::Arc<dyn fava_write::materialization::ReplaceableEventMaterializer>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/edit.rs","example":"MOD-1"} --> | Returns a fresh kind-10009 materializer behind the neutral contract. Its type and edit codec stay private. It verifies a signed or unsigned source, requires matching author/kind and a strictly later output timestamp, preserves opaque content and unrelated tags, and reports refusal as `WriteIntentError`.<br><br>Example: [MOD-1](#mod-1). |
| **`saved_group_lists`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_simple_groups::saved_group_lists","signature":"pub fn fava_simple_groups::saved_group_lists(impl core::iter::traits::collect::IntoIterator<Item = nostr::key::public_key::PublicKey>) -> core::result::Result<fava_query::Query, fava_query::QueryError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Simple Group List; crates/fava-simple-groups/src/query.rs; NIP-51 kind 10009","example":"MOD-1"} --> | Builds the ordinary kind-10009 query for the exact supplied author set. Empty input intentionally matches nothing; callers own observation. Replaces the current identical saved-group and saved-relay queries.<br><br>Example: [MOD-1](#mod-1). |

<a id="mod-1"></a>
#### MOD-1 — concrete coverage
```rust,no_run
use std::collections::BTreeSet;
use std::error::Error;
use fava_query::{Kind, RelayUrl};
use fava_simple_groups::{
    SimpleGroup, remove_saved_relay, remove_saved_simple_group, rename_saved_simple_group,
    save_relay, save_simple_group, saved_group_list_materializer, saved_group_lists,
};
use fava_write::{EventValue, ReplaceableEventMaterializer, Tag, Timestamp};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
fn has_tag(tags: &[Tag], expected: &[&str]) -> bool {
    tags.iter().any(|tag| {
        tag.as_slice()
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    })
}
fn exercise_saved_edits() -> Result<(), Box<dyn Error>> {
    let keys = Keys::generate();
    let author = keys.public_key();
    let query = saved_group_lists([author])?;
    assert_eq!(
        query.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(10_009)])),
    );
    assert_eq!(query.selection().authors, Some(BTreeSet::from([author])));
    let relay_a = RelayUrl::parse("wss://a.example")?;
    let relay_b = RelayUrl::parse("wss://b.example")?;
    let group =
        SimpleGroup::new("photos", vec![relay_a.clone(), relay_b.clone()])?;
    let source = NostrEventBuilder::new(Kind::from_u16(10_009), "opaque")
        .tags([
            Tag::parse(["group", "photos", "wss://a.example", "Old"])?,
            Tag::parse(["r", "not a relay URL/ß"])?,
            Tag::parse(["x", "preserved"])?,
        ])
        .custom_created_at(Timestamp::from(1))
        .finalize(&keys)?;
    let source = EventValue::Signed(source);
    let materializer = saved_group_list_materializer();
    assert_eq!(materializer.kind(), Kind::from_u16(10_009));
    let save = save_simple_group(&group, Some("Photos"))?;
    assert!(materializer.supports(&save));
    let saved = materializer.materialize(&save, author, None, Timestamp::from(2))?;
    assert!(has_tag(
        &saved.tags,
        &["group", "photos", "wss://a.example", "Photos"]
    ));
    assert!(has_tag(
        &saved.tags,
        &["group", "photos", "wss://b.example", "Photos"]
    ));
    let rename = rename_saved_simple_group(&group, "Renamed")?;
    let renamed = materializer.materialize(&rename, author, Some(&source), Timestamp::from(2))?;
    assert!(has_tag(
        &renamed.tags,
        &["group", "photos", "wss://a.example", "Renamed"]
    ));
    assert!(has_tag(
        &renamed.tags,
        &["group", "photos", "wss://b.example", "Renamed"]
    ));
    assert!(has_tag(&renamed.tags, &["x", "preserved"]));
    assert_eq!(renamed.content, "opaque");
    let remove_group = remove_saved_simple_group(&group)?;
    let without_group =
        materializer.materialize(&remove_group, author, Some(&source), Timestamp::from(2))?;
    assert!(
        !without_group
            .tags
            .iter()
            .any(|tag| { tag.as_slice().first().map(String::as_str) == Some("group") })
    );
    let add_relay = save_relay(relay_b.clone())?;
    let with_relay =
        materializer.materialize(&add_relay, author, Some(&source), Timestamp::from(2))?;
    assert!(has_tag(&with_relay.tags, &["r", "wss://b.example"]));
    let remove_relay = remove_saved_relay(relay_a)?;
    let without_relay =
        materializer.materialize(&remove_relay, author, Some(&source), Timestamp::from(2))?;
    assert!(!has_tag(&without_relay.tags, &["r", "wss://a.example"]));
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    exercise_saved_edits()
}
```

### `GroupAccess` (Enum)

Controls whether a kind-9002 metadata edit emits the NIP-29 `closed` tag that requires relay approval to join.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::GroupAccess","signature":"pub enum fava_simple_groups::GroupAccess","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9002 closed tag","example":"GA-1"} -->
Example coverage: [GA-1](#ga-1).

| Item | Purpose |
| --- | --- |
| **`Closed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::GroupAccess::Closed","signature":"pub fava_simple_groups::GroupAccess::Closed","evidence":"NIP-29 closed group metadata","example":"GA-1"} --> | Emits `closed`, asking the relay to require approval before a user joins.<br><br>Example: [GA-1](#ga-1). |
| **`Open`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::GroupAccess::Open","signature":"pub fava_simple_groups::GroupAccess::Open","evidence":"NIP-29 open group metadata","example":"GA-1"} --> | Emits no `closed` tag, leaving joining open under NIP-29 metadata semantics.<br><br>Example: [GA-1](#ga-1). |

<a id="ga-1"></a>
#### GA-1 — concrete coverage
```rust,no_run
use fava_simple_groups::GroupAccess;
assert_eq!(GroupAccess::Open, GroupAccess::Open);
assert_eq!(GroupAccess::Closed, GroupAccess::Closed);
```

### `GroupVisibility` (Enum)

Controls whether a kind-9002 metadata edit emits the NIP-29 `private` tag that withholds group content from non-members.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::GroupVisibility","signature":"pub enum fava_simple_groups::GroupVisibility","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9002 private tag","example":"GV-1"} -->
Example coverage: [GV-1](#gv-1).

| Item | Purpose |
| --- | --- |
| **`Private`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::GroupVisibility::Private","signature":"pub fava_simple_groups::GroupVisibility::Private","evidence":"NIP-29 private group metadata","example":"GV-1"} --> | Emits `private`, asking the relay to withhold group content from non-members.<br><br>Example: [GV-1](#gv-1). |
| **`Public`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::GroupVisibility::Public","signature":"pub fava_simple_groups::GroupVisibility::Public","evidence":"NIP-29 public group metadata","example":"GV-1"} --> | Emits no `private` tag, leaving group content visible under NIP-29 metadata semantics.<br><br>Example: [GV-1](#gv-1). |

<a id="gv-1"></a>
#### GV-1 — concrete coverage
```rust,no_run
use fava_simple_groups::GroupVisibility;
assert_eq!(GroupVisibility::Public, GroupVisibility::Public);
assert_eq!(GroupVisibility::Private, GroupVisibility::Private);
```

### `MetadataEdit` (Struct)

The exact optional fields used to build one kind-9002 metadata replacement request; `None` omits that field's tag.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::MetadataEdit","signature":"pub struct fava_simple_groups::MetadataEdit","evidence":"crates/fava-simple-groups/src/management.rs; NIP-29 kind 9002","example":"ME-1"} -->
Example coverage: [ME-1](#me-1).

| Item | Purpose |
| --- | --- |
| **`about`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::MetadataEdit::about","signature":"pub fava_simple_groups::MetadataEdit::about: core::option::Option<alloc::string::String>","evidence":"NIP-29 kind 9002 about tag","example":"ME-1"} --> | Supplies the exact group description; `None` emits no `about` tag.<br><br>Example: [ME-1](#me-1). |
| **`access`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::MetadataEdit::access","signature":"pub fava_simple_groups::MetadataEdit::access: core::option::Option<fava_simple_groups::GroupAccess>","evidence":"NIP-29 kind 9002 closed tag","example":"ME-1"} --> | Selects open or approval-required joining; `None` emits no `closed` tag.<br><br>Example: [ME-1](#me-1). |
| **`name`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::MetadataEdit::name","signature":"pub fava_simple_groups::MetadataEdit::name: core::option::Option<alloc::string::String>","evidence":"NIP-29 kind 9002 name tag","example":"ME-1"} --> | Supplies the exact human-readable group name; `None` emits no `name` tag.<br><br>Example: [ME-1](#me-1). |
| **`picture`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::MetadataEdit::picture","signature":"pub fava_simple_groups::MetadataEdit::picture: core::option::Option<alloc::string::String>","evidence":"NIP-29 kind 9002 picture tag","example":"ME-1"} --> | Supplies the exact group-picture URL text; `None` emits no `picture` tag.<br><br>Example: [ME-1](#me-1). |
| **`visibility`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::MetadataEdit::visibility","signature":"pub fava_simple_groups::MetadataEdit::visibility: core::option::Option<fava_simple_groups::GroupVisibility>","evidence":"NIP-29 kind 9002 private tag","example":"ME-1"} --> | Selects public or member-only content visibility; `None` emits no `private` tag.<br><br>Example: [ME-1](#me-1). |

<a id="me-1"></a>
#### ME-1 — concrete coverage
```rust,no_run
use fava_simple_groups::{GroupAccess, GroupVisibility, MetadataEdit};
let edit = MetadataEdit {
    name: Some("Cats".to_owned()),
    about: None,
    picture: None,
    visibility: Some(GroupVisibility::Private),
    access: Some(GroupAccess::Closed),
};
assert_eq!(edit.name.as_deref(), Some("Cats"));
```

### `SavedGroupList` (Struct)

One kind-10009 event decoded once, retaining its author plus every public `group` and `r` entry or tag-local failure.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SavedGroupList","signature":"pub struct fava_simple_groups::SavedGroupList","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Simple Group List; NIP-51 kind 10009","example":"SGL-1"} -->
Example coverage: [SGL-1](#sgl-1).

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedGroupList::author","signature":"pub const fn fava_simple_groups::SavedGroupList::author(&self) -> nostr::key::public_key::PublicKey","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Simple Group List","example":"SGL-1"} --> | Returns the event author whose public preference list this is.<br><br>Example: [SGL-1](#sgl-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedGroupList::from_event","signature":"pub fn fava_simple_groups::SavedGroupList::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SavedGroupListDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Simple Group List; current parsers crates/fava-simple-groups/src/saved.rs","example":"SGL-1"} --> | Checks kind 10009, then decodes every `group` and `r` tag independently. Unknown tags and unused extras are ignored; repetitions and valid siblings survive; no signature/id verification or generic bounds.<br><br>Example: [SGL-1](#sgl-1). |
| **`relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedGroupList::relays","signature":"pub fn fava_simple_groups::SavedGroupList::relays(&self) -> &[core::result::Result<alloc::string::String, fava_simple_groups::SavedGroupListDecodeError>]","evidence":"NIP-51 kind-10009 r tags","example":"SGL-1"} --> | Returns every `r` tag's exact first value or a local missing-position failure in source order. Present strings are not URL-validated; repetitions survive.<br><br>Example: [SGL-1](#sgl-1). |
| **`simple_groups`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedGroupList::simple_groups","signature":"pub fn fava_simple_groups::SavedGroupList::simple_groups(&self) -> &[core::result::Result<fava_simple_groups::SavedSimpleGroup, fava_simple_groups::SavedGroupListDecodeError>]","evidence":"NIP-51 kind-10009 group tags","example":"SGL-1"} --> | Returns all `group` entries and local failures in their relative source order.<br><br>Example: [SGL-1](#sgl-1). |

<a id="sgl-1"></a>
#### SGL-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SavedGroupList, SavedGroupListDecodeError};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let event = EventBuilder::new(author, Kind::from_u16(10_009))
        .created_at(Timestamp::from(1))
        .tags([
            Tag::parse(["group", "photos", "wss://a.example", "A", "ignored"])?,
            Tag::parse(["group", "missing-relay"])?,
            Tag::parse(["group", "photos", "not a relay URL/ß", "A"])?,
            Tag::parse(["r", "wss://a.example", "ignored"])?,
            Tag::parse(["r"])?,
            Tag::parse(["r", "not a relay URL/ß"])?,
        ])
        .build()?;
    let decoded = SavedGroupList::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.author(), author);
    let groups = decoded.simple_groups();
    assert_eq!(groups[0].as_ref().expect("first").id(), "photos");
    match &groups[1] {
        Err(SavedGroupListDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (1, 2)),
        other => panic!("unexpected middle group: {other:?}"),
    }
    assert_eq!(
        groups[2].as_ref().expect("foreign relay").relay(),
        "not a relay URL/ß"
    );
    let relays = decoded.relays();
    assert_eq!(
        relays[0].as_ref().expect("first").as_str(),
        "wss://a.example"
    );
    match &relays[1] {
        Err(SavedGroupListDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (4, 1)),
        other => panic!("unexpected middle relay: {other:?}"),
    }
    assert_eq!(
        relays[2].as_ref().expect("foreign relay").as_str(),
        "not a relay URL/ß"
    );
    Ok(())
}
```

### `SavedGroupListDecodeError` (Enum)

Source-positioned kind-10009 public-tag decode failures; valid sibling entries remain available.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::SavedGroupListDecodeError","signature":"pub enum fava_simple_groups::SavedGroupListDecodeError","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Errors,#Simple Group List; NIP-51 kind 10009","example":"SGLE-1"} -->
Example coverage: [SGLE-1](#sgle-1).

| Item | Purpose |
| --- | --- |
| **`MissingTagValue`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SavedGroupListDecodeError::MissingTagValue","signature":"pub fava_simple_groups::SavedGroupListDecodeError::MissingTagValue","evidence":"NIP-51 group and r required positions","example":"SGLE-1"} --> | A recognized `group` or `r` tag lacks a required position.<br><br>Example: [SGLE-1](#sgle-1). |
| **`Field `tag_index` of `MissingTagValue``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SavedGroupListDecodeError::MissingTagValue::tag_index","signature":"pub fava_simple_groups::SavedGroupListDecodeError::MissingTagValue::tag_index: usize","evidence":"source-position conservation","example":"SGLE-1"} --> | Zero-based failing tag index.<br><br>Example: [SGLE-1](#sgle-1). |
| **`Field `value_index` of `MissingTagValue``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SavedGroupListDecodeError::MissingTagValue::value_index","signature":"pub fava_simple_groups::SavedGroupListDecodeError::MissingTagValue::value_index: usize","evidence":"source-position conservation","example":"SGLE-1"} --> | Zero-based missing value position.<br><br>Example: [SGLE-1](#sgle-1). |
| **`WrongEventKind`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SavedGroupListDecodeError::WrongEventKind","signature":"pub fava_simple_groups::SavedGroupListDecodeError::WrongEventKind","evidence":"SavedGroupList owns kind 10009","example":"SGLE-1"} --> | The decoder received a non-10009 event.<br><br>Example: [SGLE-1](#sgle-1). |
| **`Field `actual` of `WrongEventKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SavedGroupListDecodeError::WrongEventKind::actual","signature":"pub fava_simple_groups::SavedGroupListDecodeError::WrongEventKind::actual: nostr::event::kind::Kind","evidence":"supplied EventValue kind","example":"SGLE-1"} --> | Supplied kind.<br><br>Example: [SGLE-1](#sgle-1). |
| **`Field `expected` of `WrongEventKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SavedGroupListDecodeError::WrongEventKind::expected","signature":"pub fava_simple_groups::SavedGroupListDecodeError::WrongEventKind::expected: nostr::event::kind::Kind","evidence":"kind-10009 boundary","example":"SGLE-1"} --> | Exact required kind.<br><br>Example: [SGLE-1](#sgle-1). |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_simple_groups::SavedGroupListDecodeError as core::fmt::Display>::fmt","signature":"pub fn fava_simple_groups::SavedGroupListDecodeError::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"standard public error presentation","example":"SGLE-1"} --> | Renders kind and source-position failures without retaining attacker-controlled tag text.<br><br>Example: [SGLE-1](#sgle-1). |

<a id="sgle-1"></a>
#### SGLE-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SavedGroupList, SavedGroupListDecodeError};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn value(kind: u16, tags: Vec<Tag>) -> Result<EventValue, Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    Ok(EventValue::Unsigned(
        EventBuilder::new(author, Kind::from_u16(kind))
            .created_at(Timestamp::from(1))
            .tags(tags)
            .build()?,
    ))
}
fn main() -> Result<(), Box<dyn Error>> {
    let wrong = SavedGroupList::from_event(&value(1, Vec::new())?).unwrap_err();
    match wrong {
        SavedGroupListDecodeError::WrongEventKind { expected, actual } => assert_eq!(
            (expected, actual),
            (Kind::from_u16(10_009), Kind::from_u16(1))
        ),
        other => panic!("unexpected error: {other}"),
    }
    let missing = SavedGroupList::from_event(&value(10_009, vec![Tag::parse(["group", "g"])?])?)?;
    match &missing.simple_groups()[0] {
        Err(SavedGroupListDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (0, 2)),
        other => panic!("unexpected entry: {other:?}"),
    }
    let foreign =
        SavedGroupList::from_event(&value(10_009, vec![Tag::parse(["r", "not-a-relay"])?])?)?;
    assert_eq!(foreign.relays()[0], Ok("not-a-relay".to_owned()));
    Ok(())
}
```

### `SavedSimpleGroup` (Struct)

One public `group` tag semantic entry: exact id, exact inert relay string, and optional display name. It proves a saved preference, not group existence, URL validity, or authority.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SavedSimpleGroup","signature":"pub struct fava_simple_groups::SavedSimpleGroup","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Simple Group List; crates/fava-simple-groups/src/saved.rs; NIP-51 kind 10009","example":"SSG-1"} -->
Example coverage: [SSG-1](#ssg-1).

| Item | Purpose |
| --- | --- |
| **`display_name`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedSimpleGroup::display_name","signature":"pub fn fava_simple_groups::SavedSimpleGroup::display_name(&self) -> core::option::Option<&str>","evidence":"NIP-51 kind-10009 optional group name","example":"SSG-1"} --> | Returns the optional exact display name; `Some("")` differs from `None`.<br><br>Example: [SSG-1](#ssg-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedSimpleGroup::id","signature":"pub fn fava_simple_groups::SavedSimpleGroup::id(&self) -> &str","evidence":"NIP-51 kind-10009 group tag first value","example":"SSG-1"} --> | Borrows the exact group id; empty remains a value.<br><br>Example: [SSG-1](#ssg-1). |
| **`relay`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SavedSimpleGroup::relay","signature":"pub fn fava_simple_groups::SavedSimpleGroup::relay(&self) -> &str","evidence":"NIP-51 kind-10009 group tag second value","example":"SSG-1"} --> | Borrows the exact inert relay string without URL validation.<br><br>Example: [SSG-1](#ssg-1). |

<a id="ssg-1"></a>
#### SSG-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::SavedGroupList;
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let event = EventBuilder::new(author, Kind::from_u16(10_009))
        .created_at(Timestamp::from(1))
        .tag(Tag::parse(["group", "photos", "not a relay URL/ß", ""])?)
        .build()?;
    let list = SavedGroupList::from_event(&EventValue::Unsigned(event))?;
    let saved = list.simple_groups()[0].as_ref().expect("valid group entry");
    assert_eq!(saved.id(), "photos");
    assert_eq!(saved.relay(), "not a relay URL/ß");
    assert_eq!(saved.display_name(), Some(""));
    Ok(())
}
```

### `SimpleGroup` (Struct)

Immutable non-empty opaque simple-group id plus a normalized non-empty application-selected relay sequence. It lowers context into ordinary queries and unsigned events without opening work.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroup","signature":"pub struct fava_simple_groups::SimpleGroup","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#SimpleGroup; crates/fava-simple-groups/src/simple_group.rs; NIP-29 Group identity, migration and forking","example":"SG-1"} -->
Example coverage: [SG-1](#sg-1).

| Item | Purpose |
| --- | --- |
| **`events`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroup::events","signature":"pub fn fava_simple_groups::SimpleGroup::events(&self, fava_query::Query) -> core::result::Result<fava_query::Query, fava_query::QueryError>","evidence":"docs/issues/0028-query-tag-axis-composition.md; crates/fava-simple-groups/src/simple_group.rs; crates/fava-query/tests/query_identity.rs; NIP-29 The h tag","example":"SG-1"} --> | Narrows lowercase `h` through query-owned exact intersection, so absent becomes this id, overlap narrows to this id, and disjoint remains present-empty match-nothing. Then delegates acquisition to all retained relays and returns exact `QueryError` refusals.<br><br>Example: [SG-1](#sg-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroup::id","signature":"pub fn fava_simple_groups::SimpleGroup::id(&self) -> &str","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/simple_group.rs","example":"SG-1"} --> | Borrows the exact supplied non-empty opaque id; no trimming or normalization is performed.<br><br>Example: [SG-1](#sg-1). |
| **`meta_events`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroup::meta_events","signature":"pub fn fava_simple_groups::SimpleGroup::meta_events<I>(&self, I) -> core::result::Result<fava_query::Query, fava_query::QueryError> where I: core::iter::traits::collect::IntoIterator<Item = fava_simple_groups::SimpleGroupStateEventKind>","evidence":"crates/fava-simple-groups/src/simple_group.rs; crates/fava-query/src/selection.rs; NIP-29 Relay-generated events","example":"SG-1"} --> | Delegates kinds to `Query::kinds`, adds exact `d = id`, and delegates relay authority to `Query::only_from_relays`. Adds no private bound or validation and returns exact `QueryError` refusals.<br><br>Example: [SG-1](#sg-1). |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroup::new","signature":"pub fn fava_simple_groups::SimpleGroup::new(impl core::convert::Into<alloc::string::String>, alloc::vec::Vec<nostr::types::url::RelayUrl>) -> core::result::Result<Self, fava_simple_groups::SimpleGroupConstructionError>","evidence":"docs/issues/0027-simple-group-relay-input-boundary.md; crates/fava-simple-groups/src/simple_group.rs","example":"SG-1"} --> | Accepts the complete finite parsed relay vector and returns an exact typed refusal for an empty id or empty vector. Preserves every other id exactly and collapses later relay duplicates in first-occurrence order without a numeric domain limit.<br><br>Example: [SG-1](#sg-1). |
| **`relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroup::relays","signature":"pub fn fava_simple_groups::SimpleGroup::relays(&self) -> impl core::iter::traits::iterator::Iterator<Item = nostr::types::url::RelayUrl> + '_","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; crates/fava-simple-groups/src/simple_group.rs","example":"SG-1"} --> | Yields cloned normalized relays in first-occurrence order for query composition and the application’s explicit route.<br><br>Example: [SG-1](#sg-1). |

<a id="sg-1"></a>
#### SG-1 — concrete coverage
```rust,no_run
use std::collections::BTreeSet;
use std::error::Error;
use fava_query::{
    Kind, Query, QueryAcquisition, RelayUrl, ResultAuthority, SingleLetterTag,
};
use fava_simple_groups::{
    SimpleGroup, SimpleGroupEventBuilder, SimpleGroupStateEventKind,
};
use fava_write::{EventBuilder, PublicKey, Timestamp, WriteRouting};
fn exercise_simple_group() -> Result<(), Box<dyn Error>> {
    let first = RelayUrl::parse("wss://a.example")?;
    let second = RelayUrl::parse("wss://b.example")?;
    let group = SimpleGroup::new(
        " photos ",
        vec![first.clone(), second.clone(), first.clone()],
    )?;
    assert_eq!(group.id(), " photos ");
    assert_eq!(
        group.relays().collect::<Vec<_>>(),
        [first, second],
    );
    let h = SingleLetterTag::from_char('h').expect("lowercase h");
    let content = group.events(Query::events().kinds([Kind::from_u16(1)])?)?;
    assert_eq!(
        content.selection().tag_values.get(&h),
        Some(&BTreeSet::from([" photos ".to_owned()])),
    );
    assert!(matches!(
        content.source().acquisition(),
        QueryAcquisition::Explicit(relays) if relays.len() == 2
    ));
    assert_eq!(content.source().authority(), &ResultAuthority::AnyLocal);
    let disjoint = group.events(Query::events().tag_values(h, ["other"])?)?;
    assert_eq!(
        disjoint.selection().tag_values.get(&h),
        Some(&BTreeSet::new()),
    );
    let meta_events = group.meta_events([SimpleGroupStateEventKind::Members])?;
    let d = SingleLetterTag::from_char('d').expect("lowercase d");
    assert_eq!(
        meta_events.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(39_002)])),
    );
    assert_eq!(
        meta_events.selection().tag_values.get(&d),
        Some(&BTreeSet::from([" photos ".to_owned()])),
    );
    assert!(matches!(
        meta_events.source().authority(),
        ResultAuthority::OnlyRelays(relays) if relays.len() == 2
    ));
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let builder = EventBuilder::new(author, Kind::from_u16(1))
        .created_at(Timestamp::from(7))
        .content("hello")
        .simple_group(&group)?;
    let (event, routing) = builder.into_event_and_routing()?;
    assert!(event.tags.iter().any(|tag| {
        tag.as_slice().get(0).map(String::as_str) == Some("h")
            && tag.as_slice().get(1).map(String::as_str) == Some(" photos ")
    }));
    assert!(matches!(routing, WriteRouting::Explicit(relays) if relays.len() == 2));
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    exercise_simple_group()
}
```

### `SimpleGroupAdmins` (Struct)

One tolerant kind-39001 semantic decode preserving every administrator entry, assigned role labels, source order, and local failure.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupAdmins","signature":"pub struct fava_simple_groups::SimpleGroupAdmins","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/people.rs; NIP-29 kind 39001","example":"ADM-1"} -->
Example coverage: [ADM-1](#adm-1).

| Item | Purpose |
| --- | --- |
| **`admins`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupAdmins::admins","signature":"pub fn fava_simple_groups::SimpleGroupAdmins::admins(&self) -> &[core::result::Result<(alloc::string::String, alloc::vec::Vec<alloc::string::String>), fava_simple_groups::SimpleGroupDecodeError>]","evidence":"NIP-29 kind 39001 p tags","example":"ADM-1"} --> | Returns every `p` tag in source order. Success contains the exact key string and all role values; a key and at least one role position are required. Present keys are not public-key-validated; repetitions survive.<br><br>Example: [ADM-1](#adm-1). |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupAdmins::author","signature":"pub const fn fava_simple_groups::SimpleGroupAdmins::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"ADM-1"} --> | Returns the event author without converting it into serving-relay authority.<br><br>Example: [ADM-1](#adm-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupAdmins::from_event","signature":"pub fn fava_simple_groups::SimpleGroupAdmins::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/people.rs","example":"ADM-1"} --> | Checks kind 39001 and the first `d` position, then decodes each `p` tag independently without verification, bounds, or deduplication.<br><br>Example: [ADM-1](#adm-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupAdmins::id","signature":"pub fn fava_simple_groups::SimpleGroupAdmins::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"ADM-1"} --> | Borrows the selected opaque id.<br><br>Example: [ADM-1](#adm-1). |

<a id="adm-1"></a>
#### ADM-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::SimpleGroupAdmins;
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let admin = "not-a-public-key-ß";
    let event = EventBuilder::new(author, Kind::from_u16(39_001))
        .created_at(Timestamp::from(1))
        .tags([
            Tag::parse(["d", "g", "ignored"])?,
            Tag::parse(["p", admin, "admin", "moderator"])?,
            Tag::parse(["p", admin, "admin"])?,
            Tag::parse(["p", admin, "admin", "moderator", "speaker"])?,
        ])
        .build()?;
    let decoded = SimpleGroupAdmins::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "g");
    assert_eq!(decoded.author(), author);
    let entries = decoded.admins();
    assert_eq!(
        entries[0],
        Ok((admin.to_owned(), vec!["admin".to_owned(), "moderator".to_owned()]))
    );
    assert_eq!(entries[1], Ok((admin.to_owned(), vec!["admin".to_owned()])));
    assert_eq!(
        entries[2],
        Ok((
            admin.to_owned(),
            vec![
                "admin".to_owned(),
                "moderator".to_owned(),
                "speaker".to_owned()
            ]
        ))
    );
    Ok(())
}
```

### `SimpleGroupConstructionError` (Enum)

Exact caller-attributable refusals from the finite `SimpleGroup` constructor boundary.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::SimpleGroupConstructionError","signature":"pub enum fava_simple_groups::SimpleGroupConstructionError","evidence":"docs/issues/0027-simple-group-relay-input-boundary.md; crates/fava-simple-groups/src/simple_group.rs","example":"SGE-1"} -->
Example coverage: [SGE-1](#sge-1).

| Item | Purpose |
| --- | --- |
| **`EmptyId`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupConstructionError::EmptyId","signature":"pub fava_simple_groups::SimpleGroupConstructionError::EmptyId","evidence":"docs/issues/0027-simple-group-relay-input-boundary.md; crates/fava-simple-groups/src/tests/simple_group.rs","example":"SGE-1"} --> | The supplied id is exactly zero length. Whitespace and every other non-empty id remain opaque valid values.<br><br>Example: [SGE-1](#sge-1). |
| **`EmptyRelays`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupConstructionError::EmptyRelays","signature":"pub fava_simple_groups::SimpleGroupConstructionError::EmptyRelays","evidence":"docs/issues/0027-simple-group-relay-input-boundary.md; crates/fava-simple-groups/src/tests/simple_group.rs","example":"SGE-1"} --> | The supplied finite relay vector contains no relay.<br><br>Example: [SGE-1](#sge-1). |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_simple_groups::SimpleGroupConstructionError as core::fmt::Display>::fmt","signature":"pub fn fava_simple_groups::SimpleGroupConstructionError::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"crates/fava-simple-groups/src/simple_group.rs","example":"SGE-1"} --> | Formats the exact constructor refusal without erasing its typed variant.<br><br>Example: [SGE-1](#sge-1). |

<a id="sge-1"></a>
#### SGE-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_query::RelayUrl;
use fava_simple_groups::{SimpleGroup, SimpleGroupConstructionError};
fn main() -> Result<(), Box<dyn Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    assert_eq!(
        SimpleGroup::new("", vec![relay]),
        Err(SimpleGroupConstructionError::EmptyId),
    );
    assert_eq!(
        SimpleGroup::new("photos", Vec::new()),
        Err(SimpleGroupConstructionError::EmptyRelays),
    );
    assert_eq!(
        SimpleGroupConstructionError::EmptyId.to_string(),
        "simple group id must not be empty",
    );
    Ok(())
}
```

### `SimpleGroupDecodeError` (Enum)

Source-positioned NIP-29 semantic decode failures. Whole-event errors cover kind and required group id; repeated-entry errors remain local so valid siblings survive.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::SimpleGroupDecodeError","signature":"pub enum fava_simple_groups::SimpleGroupDecodeError","evidence":"crates/fava-simple-groups/src/records.rs; crates/fava-simple-groups/src/tests/codec.rs","example":"DE-1"} -->
Example coverage: [DE-1](#de-1).

| Item | Purpose |
| --- | --- |
| **`InvalidLivekitParticipantPublicKey`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey","signature":"pub fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey","evidence":"NIP-29 39004 lowercase 64-character hex requirement","example":"DE-1"} --> | A participant key is not exact lowercase 64-character hex or fails `PublicKey` parsing.<br><br>Example: [DE-1](#de-1). |
| **`Field `tag_index` of `InvalidLivekitParticipantPublicKey``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey::tag_index","signature":"pub fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey::tag_index: usize","evidence":"source-position conservation","example":"DE-1"} --> | Zero-based failing tag index.<br><br>Example: [DE-1](#de-1). |
| **`Field `value_index` of `InvalidLivekitParticipantPublicKey``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey::value_index","signature":"pub fava_simple_groups::SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey::value_index: usize","evidence":"source-position conservation","example":"DE-1"} --> | Zero-based failing value position.<br><br>Example: [DE-1](#de-1). |
| **`MissingIdentifierTag`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupDecodeError::MissingIdentifierTag","signature":"pub fava_simple_groups::SimpleGroupDecodeError::MissingIdentifierTag","evidence":"NIP-29 relay-generated events require d tag","example":"DE-1"} --> | No `d` tag exists, so the event cannot supply its group id.<br><br>Example: [DE-1](#de-1). |
| **`MissingTagValue`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupDecodeError::MissingTagValue","signature":"pub fava_simple_groups::SimpleGroupDecodeError::MissingTagValue","evidence":"crates/fava-simple-groups/src/records.rs; crates/fava-simple-groups/src/tests/codec.rs","example":"DE-1"} --> | A recognized repeated entry, or the first `d` tag, lacks a protocol-required position.<br><br>Example: [DE-1](#de-1). |
| **`Field `tag_index` of `MissingTagValue``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::MissingTagValue::tag_index","signature":"pub fava_simple_groups::SimpleGroupDecodeError::MissingTagValue::tag_index: usize","evidence":"source-position conservation","example":"DE-1"} --> | Zero-based event tag index.<br><br>Example: [DE-1](#de-1). |
| **`Field `value_index` of `MissingTagValue``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::MissingTagValue::value_index","signature":"pub fava_simple_groups::SimpleGroupDecodeError::MissingTagValue::value_index: usize","evidence":"source-position conservation","example":"DE-1"} --> | Zero-based index in `Tag::as_slice()`; `1` is the first value after the tag name.<br><br>Example: [DE-1](#de-1). |
| **`WrongEventKind`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupDecodeError::WrongEventKind","signature":"pub fava_simple_groups::SimpleGroupDecodeError::WrongEventKind","evidence":"one exact kind per decoder","example":"DE-1"} --> | The selected decoder does not own the supplied event kind.<br><br>Example: [DE-1](#de-1). |
| **`Field `actual` of `WrongEventKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::WrongEventKind::actual","signature":"pub fava_simple_groups::SimpleGroupDecodeError::WrongEventKind::actual: nostr::event::kind::Kind","evidence":"supplied EventValue kind","example":"DE-1"} --> | Supplied kind.<br><br>Example: [DE-1](#de-1). |
| **`Field `expected` of `WrongEventKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_simple_groups::SimpleGroupDecodeError::WrongEventKind::expected","signature":"pub fava_simple_groups::SimpleGroupDecodeError::WrongEventKind::expected: nostr::event::kind::Kind","evidence":"one exact kind per decoder","example":"DE-1"} --> | Exact required kind.<br><br>Example: [DE-1](#de-1). |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_simple_groups::SimpleGroupDecodeError as core::fmt::Display>::fmt","signature":"pub fn fava_simple_groups::SimpleGroupDecodeError::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"standard public error presentation","example":"DE-1"} --> | Renders kind and source-position failures without retaining attacker-controlled tag text.<br><br>Example: [DE-1](#de-1). |

<a id="de-1"></a>
#### DE-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{
    SimpleGroupAdmins, SimpleGroupDecodeError, SimpleGroupLivekitParticipants, SimpleGroupMetadata,
    SimpleGroupPins,
};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn value(kind: u16, tags: Vec<Tag>) -> Result<EventValue, Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    Ok(EventValue::Unsigned(
        EventBuilder::new(author, Kind::from_u16(kind))
            .created_at(Timestamp::from(1))
            .tags(tags)
            .build()?,
    ))
}
fn main() -> Result<(), Box<dyn Error>> {
    let wrong = SimpleGroupMetadata::from_event(&value(39_001, vec![Tag::parse(["d", "g"])?])?)
        .unwrap_err();
    match wrong {
        SimpleGroupDecodeError::WrongEventKind { expected, actual } => assert_eq!(
            (expected, actual),
            (Kind::from_u16(39_000), Kind::from_u16(39_001))
        ),
        other => panic!("unexpected error: {other}"),
    }
    let missing_id = SimpleGroupMetadata::from_event(&value(39_000, Vec::new())?).unwrap_err();
    match missing_id {
        SimpleGroupDecodeError::MissingIdentifierTag => {}
        other => panic!("unexpected error: {other}"),
    }
    let missing_value =
        SimpleGroupMetadata::from_event(&value(39_000, vec![Tag::parse(["d"])?])?).unwrap_err();
    match missing_value {
        SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        } => assert_eq!((tag_index, value_index), (0, 1)),
        other => panic!("unexpected error: {other}"),
    }
    // Admin pubkeys are stored as raw strings; "bad" is returned as-is
    let with_bad_key = SimpleGroupAdmins::from_event(&value(
        39_001,
        vec![Tag::parse(["d", "g"])?, Tag::parse(["p", "bad", "admin"])?],
    )?)?;
    assert!(with_bad_key.admins()[0].is_ok());
    let invalid_live = SimpleGroupLivekitParticipants::from_event(&value(
        39_004,
        vec![Tag::parse(["d", "g"])?, Tag::parse(["participant", "ABC"])?],
    )?)?;
    match &invalid_live.participants()[0] {
        Err(SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (1, 1)),
        other => panic!("unexpected entry: {other:?}"),
    }
    // supported_kinds() returns raw strings; "bad" is preserved as-is
    let with_bad_kind = SimpleGroupMetadata::from_event(&value(
        39_000,
        vec![
            Tag::parse(["d", "g"])?,
            Tag::parse(["supported_kinds", "bad"])?,
        ],
    )?)?;
    let kinds = with_bad_kind.supported_kinds().expect("tag present");
    assert_eq!(kinds[0], "bad");
    // pins() returns cloned Tag; e/a tags with any first value succeed
    let with_bad_e = SimpleGroupPins::from_event(&value(
        39_005,
        vec![Tag::parse(["d", "g"])?, Tag::parse(["e", "bad"])?],
    )?)?;
    assert!(with_bad_e.pins()[0].is_ok());
    let with_bad_a = SimpleGroupPins::from_event(&value(
        39_005,
        vec![Tag::parse(["d", "g"])?, Tag::parse(["a", "bad"])?],
    )?)?;
    assert!(with_bad_a.pins()[0].is_ok());
    Ok(())
}
```

### `SimpleGroupEventBuilder` (Trait)

Fluent NIP-29 context composition for the concrete generic `EventBuilder`.
<!-- api-item {"kind":"Trait","item":"fava_simple_groups::SimpleGroupEventBuilder","signature":"pub trait fava_simple_groups::SimpleGroupEventBuilder","evidence":"cargo-public-api@0.52.0: pub trait fava_simple_groups::SimpleGroupEventBuilder"} -->

| Item | Purpose |
| --- | --- |
| **`simple_group`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupEventBuilder::simple_group","signature":"pub fn fava_simple_groups::SimpleGroupEventBuilder::simple_group(self, &fava_simple_groups::SimpleGroup) -> core::result::Result<fava_write::builder::EventBuilder, fava_write::WriteIntentError>","evidence":"cargo-public-api@0.52.0: pub fn fava_simple_groups::SimpleGroupEventBuilder::simple_group(self, &fava_simple_groups::SimpleGroup) -> core::result::Result<fava_write::builder::EventBuilder, fava_write::WriteIntentError>"} --> | Returns the same concrete builder after adding one exact `h` context and accumulating the group's relays as bounded local publication intent; repeated exact contexts do not add duplicate tags, while distinct contexts compose. |

```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroup, SimpleGroupEventBuilder};
use fava_write::{EventBuilder, Kind, PublicKey};
use nostr::types::RelayUrl;

fn main() -> Result<(), Box<dyn Error>> {
    let author = PublicKey::from_hex(
        "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    )?;
    let group = SimpleGroup::new(
        "photos",
        vec![RelayUrl::parse("wss://groups.example")?],
    )?;
    let _builder = EventBuilder::new(author, Kind::from_u16(9)).simple_group(&group)?;
    Ok(())
}
```

### `SimpleGroupLivekitParticipants` (Struct)

One tolerant kind-39004 semantic decode of current `LiveKit` participants, distinct from durable group membership.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupLivekitParticipants","signature":"pub struct fava_simple_groups::SimpleGroupLivekitParticipants","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/people.rs; NIP-29 kind 39004","example":"LIVE-1"} -->
Example coverage: [LIVE-1](#live-1).

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupLivekitParticipants::author","signature":"pub const fn fava_simple_groups::SimpleGroupLivekitParticipants::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"LIVE-1"} --> | Returns the event author without converting it into serving-relay authority.<br><br>Example: [LIVE-1](#live-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupLivekitParticipants::from_event","signature":"pub fn fava_simple_groups::SimpleGroupLivekitParticipants::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/people.rs","example":"LIVE-1"} --> | Checks kind 39004 and the first `d` position, then decodes each `participant` tag independently without verification, bounds, or deduplication.<br><br>Example: [LIVE-1](#live-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupLivekitParticipants::id","signature":"pub fn fava_simple_groups::SimpleGroupLivekitParticipants::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"LIVE-1"} --> | Borrows the selected opaque id.<br><br>Example: [LIVE-1](#live-1). |
| **`participants`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupLivekitParticipants::participants","signature":"pub fn fava_simple_groups::SimpleGroupLivekitParticipants::participants(&self) -> &[core::result::Result<nostr::key::public_key::PublicKey, fava_simple_groups::SimpleGroupDecodeError>]","evidence":"NIP-29 kind 39004 participant tags and lowercase requirement","example":"LIVE-1"} --> | Returns every `participant` tag in source order. The first value must be exact 64-character lowercase hex accepted by `PublicKey`; unused extras are ignored and repetitions survive.<br><br>Example: [LIVE-1](#live-1). |

<a id="live-1"></a>
#### LIVE-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroupDecodeError, SimpleGroupLivekitParticipants};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let participant =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")?;
    let lower = participant.to_hex();
    let upper = lower.to_uppercase();
    let event = EventBuilder::new(author, Kind::from_u16(39_004))
        .created_at(Timestamp::from(1))
        .tags([
            Tag::parse(["d", "g"])?,
            Tag::parse(["participant", &lower, "ignored"])?,
            Tag::parse(["participant", &upper])?,
            Tag::parse(["participant", &lower])?,
        ])
        .build()?;
    let decoded = SimpleGroupLivekitParticipants::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "g");
    assert_eq!(decoded.author(), author);
    let entries = decoded.participants();
    assert_eq!(entries[0], Ok(participant));
    match &entries[1] {
        Err(SimpleGroupDecodeError::InvalidLivekitParticipantPublicKey {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (2, 1)),
        other => panic!("unexpected middle entry: {other:?}"),
    }
    assert_eq!(entries[2], Ok(participant));
    Ok(())
}
```

### `SimpleGroupMembers` (Struct)

One tolerant kind-39002 semantic decode of positive member entries; absence never proves non-membership or completeness.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupMembers","signature":"pub struct fava_simple_groups::SimpleGroupMembers","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/people.rs; NIP-29 kind 39002","example":"MEM-1"} -->
Example coverage: [MEM-1](#mem-1).

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMembers::author","signature":"pub const fn fava_simple_groups::SimpleGroupMembers::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"MEM-1"} --> | Returns the event author without converting it into serving-relay authority.<br><br>Example: [MEM-1](#mem-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMembers::from_event","signature":"pub fn fava_simple_groups::SimpleGroupMembers::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/people.rs","example":"MEM-1"} --> | Checks kind 39002 and the first `d` position, then decodes each `p` tag independently without verification, bounds, or deduplication.<br><br>Example: [MEM-1](#mem-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMembers::id","signature":"pub fn fava_simple_groups::SimpleGroupMembers::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"MEM-1"} --> | Borrows the selected opaque id.<br><br>Example: [MEM-1](#mem-1). |
| **`members`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMembers::members","signature":"pub fn fava_simple_groups::SimpleGroupMembers::members(&self) -> &[core::result::Result<alloc::string::String, fava_simple_groups::SimpleGroupDecodeError>]","evidence":"NIP-29 kind 39002 p tags","example":"MEM-1"} --> | Returns every `p` tag in source order as its parsed first value or local error. Unused extras are ignored and repetitions survive.<br><br>Example: [MEM-1](#mem-1). |

<a id="mem-1"></a>
#### MEM-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroupDecodeError, SimpleGroupMembers};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let member =
        PublicKey::from_hex("c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5")?;
    let member_hex = member.to_hex();
    let event = EventBuilder::new(author, Kind::from_u16(39_002))
        .created_at(Timestamp::from(1))
        .tags([
            Tag::parse(["d", "g"])?,
            Tag::parse(["p", &member_hex, "ignored"])?,
            Tag::parse(["p"])?,
            Tag::parse(["p", &member_hex])?,
        ])
        .build()?;
    let decoded = SimpleGroupMembers::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "g");
    assert_eq!(decoded.author(), author);
    let entries = decoded.members();
    assert_eq!(entries[0], Ok(member_hex.clone()));
    match &entries[1] {
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (2, 1)),
        other => panic!("unexpected middle entry: {other:?}"),
    }
    assert_eq!(entries[2], Ok(member_hex.clone()));
    Ok(())
}
```

### `SimpleGroupMetadata` (Struct)

One tolerant semantic decode of kind 39000: exact id and author, optional display fields, presence flags, supported kinds, and subgroup relationships.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupMetadata","signature":"pub struct fava_simple_groups::SimpleGroupMetadata","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/metadata.rs; NIP-29 kind 39000","example":"META-1"} -->
Example coverage: [META-1](#meta-1).

| Item | Purpose |
| --- | --- |
| **`about`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::about","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::about(&self) -> core::option::Option<&str>","evidence":"NIP-29 kind 39000 about tag","example":"META-1"} --> | Returns the first usable exact description text.<br><br>Example: [META-1](#meta-1). |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::author","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"META-1"} --> | Returns the event author only; does not assert NIP-11 relay identity or serving-relay provenance.<br><br>Example: [META-1](#meta-1). |
| **`banner`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::banner","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::banner(&self) -> core::option::Option<&str>","evidence":"NIP-29 kind 39000 banner tag","example":"META-1"} --> | Returns the first usable banner URL text without fetching it.<br><br>Example: [META-1](#meta-1). |
| **`children`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::children","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::children(&self) -> &[core::result::Result<alloc::string::String, fava_simple_groups::SimpleGroupDecodeError>]","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; NIP-29 Subgroups child tags","example":"META-1"} --> | Returns every `child` tag in source order as its first value or a local missing-position error. Repetitions survive and unused extras are ignored.<br><br>Example: [META-1](#meta-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::from_event","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/metadata.rs","example":"META-1"} --> | Checks kind 39000 and the first `d` tag’s first value, then decodes protocol fields without signature/id verification or generic bounds. First usable singleton wins; unknown tags, later singleton occurrences, and unused extras are ignored.<br><br>Example: [META-1](#meta-1). |
| **`has_livekit`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::has_livekit","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::has_livekit(&self) -> bool","evidence":"NIP-29 kind 39000 livekit tag","example":"META-1"} --> | Reports presence of the `LiveKit` capability tag; owns no endpoint or NIP-11 behavior.<br><br>Example: [META-1](#meta-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::id","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"META-1"} --> | Borrows the selected opaque id; empty is a value.<br><br>Example: [META-1](#meta-1). |
| **`is_closed`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::is_closed","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::is_closed(&self) -> bool","evidence":"NIP-29 kind 39000 closed tag","example":"META-1"} --> | Reports presence of `closed`, meaning join requests are ignored.<br><br>Example: [META-1](#meta-1). |
| **`is_hidden`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::is_hidden","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::is_hidden(&self) -> bool","evidence":"NIP-29 kind 39000 hidden tag","example":"META-1"} --> | Reports presence of `hidden`; makes no discovery-completeness claim.<br><br>Example: [META-1](#meta-1). |
| **`is_private`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::is_private","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::is_private(&self) -> bool","evidence":"NIP-29 kind 39000 private tag","example":"META-1"} --> | Reports presence of `private`; absence means the event does not declare private reading.<br><br>Example: [META-1](#meta-1). |
| **`is_restricted`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::is_restricted","signature":"pub const fn fava_simple_groups::SimpleGroupMetadata::is_restricted(&self) -> bool","evidence":"NIP-29 kind 39000 restricted tag","example":"META-1"} --> | Reports presence of `restricted`; absence means the event does not declare member-only writing.<br><br>Example: [META-1](#meta-1). |
| **`name`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::name","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::name(&self) -> core::option::Option<&str>","evidence":"NIP-29 kind 39000 name tag","example":"META-1"} --> | Returns the first usable exact display name; absent and present-empty remain distinct.<br><br>Example: [META-1](#meta-1). |
| **`parent`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::parent","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::parent(&self) -> core::option::Option<&str>","evidence":"NIP-29 Subgroups parent tag","example":"META-1"} --> | Returns the first usable exact parent id; absence makes this event describe a root.<br><br>Example: [META-1](#meta-1). |
| **`picture`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::picture","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::picture(&self) -> core::option::Option<&str>","evidence":"NIP-29 kind 39000 picture tag","example":"META-1"} --> | Returns the first usable picture URL text without fetching or validating it as a relay.<br><br>Example: [META-1](#meta-1). |
| **`supported_kinds`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupMetadata::supported_kinds","signature":"pub fn fava_simple_groups::SimpleGroupMetadata::supported_kinds(&self) -> core::option::Option<&[alloc::string::String]>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; NIP-29 supported_kinds","example":"META-1"} --> | Raw string values from the first `supported_kinds` tag in source order. `None` means unspecified; `Some([])` means explicitly none; repetitions survive.<br><br>Example: [META-1](#meta-1). |

<a id="meta-1"></a>
#### META-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroupDecodeError, SimpleGroupMetadata};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let event = EventBuilder::new(author, Kind::from_u16(39_000))
        .created_at(Timestamp::from(7))
        .tags([
            Tag::parse(["d", "photos", "ignored"])?,
            Tag::parse(["name"])?,
            Tag::parse(["name", "Photos", "ignored"])?,
            Tag::parse(["name", "Later"])?,
            Tag::parse(["picture", "https://example/picture.png"])?,
            Tag::parse(["banner", "https://example/banner.png"])?,
            Tag::parse(["about", "Exact about"])?,
            Tag::parse(["private", "ignored"])?,
            Tag::parse(["restricted"])?,
            Tag::parse(["hidden"])?,
            Tag::parse(["closed"])?,
            Tag::parse(["livekit"])?,
            Tag::parse(["supported_kinds", "1", "bad", "1"])?,
            Tag::parse(["parent", "root", "ignored"])?,
            Tag::parse(["child", "one", "ignored"])?,
            Tag::parse(["child"])?,
            Tag::parse(["child", "one"])?,
        ])
        .build()?;
    let decoded = SimpleGroupMetadata::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "photos");
    assert_eq!(decoded.author(), author);
    assert_eq!(decoded.name(), Some("Photos"));
    assert_eq!(decoded.picture(), Some("https://example/picture.png"));
    assert_eq!(decoded.banner(), Some("https://example/banner.png"));
    assert_eq!(decoded.about(), Some("Exact about"));
    assert!(decoded.is_private());
    assert!(decoded.is_restricted());
    assert!(decoded.is_hidden());
    assert!(decoded.is_closed());
    assert!(decoded.has_livekit());
    assert_eq!(decoded.parent(), Some("root"));
    let kinds = decoded.supported_kinds().expect("tag was present");
    assert_eq!(kinds[0], "1");
    assert_eq!(kinds[1], "bad");
    assert_eq!(kinds[2], "1");
    let children = decoded.children();
    assert_eq!(children[0], Ok("one".to_owned()));
    match &children[1] {
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => {
            assert_eq!((*tag_index, *value_index), (15, 1));
        }
        other => panic!("unexpected middle child: {other:?}"),
    }
    assert_eq!(children[2], Ok("one".to_owned()));
    Ok(())
}
```

### `SimpleGroupPins` (Struct)

One tolerant kind-39005 semantic decode preserving interleaved event-id and addressable-coordinate pins as the existing `EventCoordinate` type.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupPins","signature":"pub struct fava_simple_groups::SimpleGroupPins","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/pins.rs; NIP-29 kind 39005","example":"PIN-1"} -->
Example coverage: [PIN-1](#pin-1).

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupPins::author","signature":"pub const fn fava_simple_groups::SimpleGroupPins::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"PIN-1"} --> | Returns the event author without converting it into serving-relay authority.<br><br>Example: [PIN-1](#pin-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupPins::from_event","signature":"pub fn fava_simple_groups::SimpleGroupPins::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/pins.rs","example":"PIN-1"} --> | Checks kind 39005 and the first `d` position, then decodes every `e` or `a` tag independently without verification, bounds, or deduplication.<br><br>Example: [PIN-1](#pin-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupPins::id","signature":"pub fn fava_simple_groups::SimpleGroupPins::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"PIN-1"} --> | Borrows the selected opaque id.<br><br>Example: [PIN-1](#pin-1). |
| **`pins`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupPins::pins","signature":"pub fn fava_simple_groups::SimpleGroupPins::pins(&self) -> &[core::result::Result<nostr::event::tag::Tag, fava_simple_groups::SimpleGroupDecodeError>]","evidence":"NIP-29 kind 39005 e/a tags; fava-state EventCoordinate","example":"PIN-1"} --> | Returns interleaved `e` and `a` tags in source order. `e` becomes `EventCoordinate::Event`; `a` becomes addressable `EventCoordinate::Replaceable`; unknown tags and unused extras are ignored; repetitions survive. No `PinnedItem` wrapper exists.<br><br>Example: [PIN-1](#pin-1). |

<a id="pin-1"></a>
#### PIN-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroupDecodeError, SimpleGroupPins};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let target = EventBuilder::new(author, Kind::from_u16(1))
        .created_at(Timestamp::from(1))
        .content("target")
        .build()?;
    let target_id = target.id.expect("builder computes id");
    let address = format!("30023:{}:article:one", author.to_hex());
    let event = EventBuilder::new(author, Kind::from_u16(39_005))
        .created_at(Timestamp::from(2))
        .tags([
            Tag::parse(["d", "g"])?,
            Tag::parse(["e", &target_id.to_hex(), "ignored"])?,
            Tag::parse(["e"])?,
            Tag::parse(["e", &target_id.to_hex()])?,
            Tag::parse(["a", &address, "ignored"])?,
        ])
        .build()?;
    let decoded = SimpleGroupPins::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "g");
    assert_eq!(decoded.author(), author);
    let pins = decoded.pins();
    let pin0 = pins[0].as_ref().expect("valid e-tag pin");
    assert_eq!(pin0.as_slice()[0], "e");
    assert_eq!(pin0.as_slice()[1], target_id.to_hex());
    match &pins[1] {
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (2, 1)),
        other => panic!("unexpected middle pin: {other:?}"),
    }
    let pin2 = pins[2].as_ref().expect("valid e-tag pin");
    assert_eq!(pin2.as_slice()[0], "e");
    assert_eq!(pin2.as_slice()[1], target_id.to_hex());
    let pin3 = pins[3].as_ref().expect("valid a-tag pin");
    assert_eq!(pin3.as_slice()[0], "a");
    assert_eq!(pin3.as_slice()[1], address);
    Ok(())
}
```

### `SimpleGroupRoles` (Struct)

One tolerant kind-39003 semantic decode preserving every role name, optional description, source position, and repetition.
<!-- api-item {"kind":"Struct","item":"fava_simple_groups::SimpleGroupRoles","signature":"pub struct fava_simple_groups::SimpleGroupRoles","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; crates/fava-simple-groups/src/people.rs; NIP-29 kind 39003","example":"ROLE-1"} -->
Example coverage: [ROLE-1](#role-1).

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupRoles::author","signature":"pub const fn fava_simple_groups::SimpleGroupRoles::author(&self) -> nostr::key::public_key::PublicKey","evidence":"EventValue author","example":"ROLE-1"} --> | Returns the event author without converting it into serving-relay authority.<br><br>Example: [ROLE-1](#role-1). |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupRoles::from_event","signature":"pub fn fava_simple_groups::SimpleGroupRoles::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_simple_groups::SimpleGroupDecodeError>","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#NIP-29 decoding; current parser crates/fava-simple-groups/src/people.rs","example":"ROLE-1"} --> | Checks kind 39003 and the first `d` position, then decodes each `role` tag independently without verification, bounds, or deduplication.<br><br>Example: [ROLE-1](#role-1). |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupRoles::id","signature":"pub fn fava_simple_groups::SimpleGroupRoles::id(&self) -> &str","evidence":"NIP-29 first d-tag value","example":"ROLE-1"} --> | Borrows the selected opaque id.<br><br>Example: [ROLE-1](#role-1). |
| **`roles`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_simple_groups::SimpleGroupRoles::roles","signature":"pub fn fava_simple_groups::SimpleGroupRoles::roles(&self) -> &[core::result::Result<(alloc::string::String, core::option::Option<alloc::string::String>), fava_simple_groups::SimpleGroupDecodeError>]","evidence":"NIP-29 kind 39003 role tags","example":"ROLE-1"} --> | Returns every `role` tag in source order as exact name plus optional description or local missing-position error. Empty strings remain values, unused extras are ignored, and repetitions survive.<br><br>Example: [ROLE-1](#role-1). |

<a id="role-1"></a>
#### ROLE-1 — concrete coverage
```rust,no_run
use std::error::Error;
use fava_simple_groups::{SimpleGroupDecodeError, SimpleGroupRoles};
use fava_write::{EventBuilder, EventValue, Kind, PublicKey, Tag, Timestamp};
fn main() -> Result<(), Box<dyn Error>> {
    let author =
        PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")?;
    let event = EventBuilder::new(author, Kind::from_u16(39_003))
        .created_at(Timestamp::from(1))
        .tags([
            Tag::parse(["d", "g"])?,
            Tag::parse(["role", "moderator", "delete", "ignored"])?,
            Tag::parse(["role"])?,
            Tag::parse(["role", "moderator", "delete"])?,
        ])
        .build()?;
    let decoded = SimpleGroupRoles::from_event(&EventValue::Unsigned(event))?;
    assert_eq!(decoded.id(), "g");
    assert_eq!(decoded.author(), author);
    let entries = decoded.roles();
    assert_eq!(
        entries[0],
        Ok(("moderator".to_owned(), Some("delete".to_owned())))
    );
    match &entries[1] {
        Err(SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        }) => assert_eq!((*tag_index, *value_index), (2, 1)),
        other => panic!("unexpected middle entry: {other:?}"),
    }
    assert_eq!(
        entries[2],
        Ok(("moderator".to_owned(), Some("delete".to_owned())))
    );
    Ok(())
}
```

### `SimpleGroupStateEventKind` (Enum)

Closed query selector with the exact mapping Metadata→39000, Admins→39001, Members→39002, Roles→39003, LivekitParticipants→39004, and Pins→39005.
<!-- api-item {"kind":"Enum","item":"fava_simple_groups::SimpleGroupStateEventKind","signature":"pub enum fava_simple_groups::SimpleGroupStateEventKind","evidence":"pad:fava/2026-08-simple-groups-api-redesign-proposal#Proposed public shape; NIP-29 Group metadata events","example":"KIND-1"} -->
Example coverage: [KIND-1](#kind-1).

| Item | Purpose |
| --- | --- |
| **`ALL`**<br><sub>Constant</sub><!-- api-item {"kind":"Constant","item":"fava_simple_groups::SimpleGroupStateEventKind::ALL","signature":"pub const fava_simple_groups::SimpleGroupStateEventKind::ALL: [Self; 6]","evidence":"crates/fava-simple-groups/src/query.rs; NIP-29 kinds 39000-39005"} --> | Enumerates all six relay-generated simple-group state-event kinds for callers that intentionally select the complete state family. |
| **`Admins`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::Admins","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::Admins","evidence":"NIP-29 kind 39001","example":"KIND-1"} --> | Admins selects kind 39001 group administrators and their role labels.<br><br>Example: [KIND-1](#kind-1). |
| **`LivekitParticipants`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::LivekitParticipants","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::LivekitParticipants","evidence":"NIP-29 kind 39004 LiveKit participants","example":"KIND-1"} --> | `LivekitParticipants` selects kind 39004 current `LiveKit` participation, not membership.<br><br>Example: [KIND-1](#kind-1). |
| **`Members`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::Members","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::Members","evidence":"NIP-29 kind 39002","example":"KIND-1"} --> | Members selects kind 39002 positive member entries.<br><br>Example: [KIND-1](#kind-1). |
| **`Metadata`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::Metadata","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::Metadata","evidence":"NIP-29 kind 39000","example":"KIND-1"} --> | Metadata selects kind 39000 group metadata.<br><br>Example: [KIND-1](#kind-1). |
| **`Pins`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::Pins","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::Pins","evidence":"NIP-29 kind 39005","example":"KIND-1"} --> | Pins selects kind 39005 ordered group pins.<br><br>Example: [KIND-1](#kind-1). |
| **`Roles`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_simple_groups::SimpleGroupStateEventKind::Roles","signature":"pub fava_simple_groups::SimpleGroupStateEventKind::Roles","evidence":"NIP-29 kind 39003","example":"KIND-1"} --> | Roles selects kind 39003 role definitions.<br><br>Example: [KIND-1](#kind-1). |

<a id="kind-1"></a>
#### KIND-1 — concrete coverage
```rust,no_run
use std::collections::BTreeSet;
use std::error::Error;
use fava_query::{Kind, Query, RelayUrl};
use fava_simple_groups::{SimpleGroup, SimpleGroupStateEventKind};
fn selected(
    group: &SimpleGroup,
    kind: SimpleGroupStateEventKind,
) -> Result<BTreeSet<Kind>, Box<dyn Error>> {
    Ok(group
        .meta_events([kind])?
        .selection()
        .kinds
        .clone()
        .expect("state-event query has kinds"))
}
fn main() -> Result<(), Box<dyn Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let group = SimpleGroup::new("g", vec![relay])?;
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::Metadata)?,
        BTreeSet::from([Kind::from_u16(39_000)])
    );
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::Admins)?,
        BTreeSet::from([Kind::from_u16(39_001)])
    );
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::Members)?,
        BTreeSet::from([Kind::from_u16(39_002)])
    );
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::Roles)?,
        BTreeSet::from([Kind::from_u16(39_003)])
    );
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::LivekitParticipants)?,
        BTreeSet::from([Kind::from_u16(39_004)])
    );
    assert_eq!(
        selected(&group, SimpleGroupStateEventKind::Pins)?,
        BTreeSet::from([Kind::from_u16(39_005)])
    );
    let all = group.meta_events(SimpleGroupStateEventKind::ALL)?;
    assert_eq!(
        all.selection().kinds,
        Some((39_000..=39_005).map(Kind::from_u16).collect()),
    );
    let _: Query = all;
    Ok(())
}
```
<!-- END crate-readme-api inventory -->
