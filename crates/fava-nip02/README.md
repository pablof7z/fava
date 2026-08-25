# fava-nip02

Typed NIP-02 contact-list reads and lossless follow edits for Fava. The crate
returns ordinary `Query` and `ReplaceableEventEdit` values; `Fava` owns
observation and publication.

## Follow and unfollow

A follow edit names the person being followed. The application selects whose
kind-3 list is being edited with `by(...)`, then publishes through Fava's one
durable publication door.

```rust
use fava::all;
use fava_nip02::{follow, unfollow};

let write = fava.by(me).publish(follow(alice)?)?;
let receipt = write.settled(all()).await?;

let write = fava.by(me).publish(unfollow(alice)?)?;
let receipt = write.settled(all()).await?;
```

`publish` returns a `Write` after local acceptance. It does not wait for relay
delivery. The accepted materialization is immediately visible through ordinary
queries, including while offline.

Add an optional relay hint and exact petname with `follow_with`:

```rust
use fava::{all, RelayUrl};
use fava_nip02::follow_with;

let relay = RelayUrl::parse("wss://relay.example")?;
let edit = follow_with(alice, Some(relay.clone()), Some("alíce"))?;
let write = fava.by(me).to([relay])?.publish(edit)?;
let receipt = write.settled(all()).await?;
```

`by(...)` and `to(...)` are inert scopes. Either order is valid:

```rust
let first = fava.by(me).to([relay.clone()])?.publish(follow(alice)?)?;
let second = fava.to([relay])?.by(me).publish(follow(bob)?)?;
```

Unsigned and pre-signed events already carry their author, so `by(...)` accepts
only a `ReplaceableEventEdit`.

## Read a contact list

`contact_list` accepts one author or a finite collection. It creates a kind-3
query with a concrete author axis and no global result limit. The ordinary
query evaluator selects the newest replaceable event independently for each
author. Construction returns the neutral query owner's exact `QueryError` when
the author iterator exceeds its bound.

```rust
use fava_nip02::{ContactList, contact_list};

let observation = fava.observe(contact_list(alice)?).await?;
let snapshot = observation.current();

for record in &snapshot.events {
    let list = ContactList::from_event(&record.event)?;
    for followed in list.follows() {
        println!(
            "{} {:?} {:?}",
            followed.pubkey(),
            followed.relay(),
            followed.petname(),
        );
    }
}
```

An empty kind-3 list is valid. `follows()` returns valid first-occurrence `p`
entries in source order. Each `Follow` exposes its source index, public key,
optional valid relay hint, and optional petname. Petnames preserve their UTF-8
bytes without normalization and distinguish an absent petname from a present
empty one.

## Entry errors

Parsing accounts for every `p` entry. Entries that cannot safely become `Follow`
remain exact typed `ContactListEntryError` with their source index and raw
values:

- `MissingTarget`
- `InvalidPublicKey`
- `InvalidRelayHint`
- `DuplicateTarget`
- `UninterpretedExtraValues`

Invalid pubkeys and relay hints do not invalidate an otherwise valid kind-3
event. Wrong-kind, unfinalized, unverifiable, or over-bound events return
`ContactListError` before entry decoding.

```rust
let list = ContactList::from_event(&record.event)?;
for entry in list.entry_errors() {
    println!("entry {}: {:?}", entry.source_index(), entry.raw_tag());
}
```

## Discovery

Project followed keys from the current snapshot, then use them as the concrete
author axis of the next ordinary query:

```rust
use fava_nip02::{contact_list, follows_of};

let first = fava.observe(contact_list(alice)?).await?;
let first_hop = follows_of(first.current().as_ref());

let second = fava.observe(contact_list(first_hop.as_slice())?).await?;
let second_hop = follows_of(second.current().as_ref());
```

`follows_of` is a bounded pure projection. It opens no observation and owns no
mutable state. An empty author collection remains a present-empty author axis,
so it matches nothing instead of broadening to every kind-3 event.

Ask who follows a subject with the exact lowercase `p` tag axis:

```rust
use fava_nip02::{ContactList, followers_of};

let observation = fava.observe(followers_of(alice)?).await?;
let snapshot = observation.current();

for record in &snapshot.events {
    let list = ContactList::from_event(&record.event)?;
    println!("{} follows Alice", list.author());
}
```

`followers_of` still returns an ordinary `Query` inside the same fallible
construction contract; it makes no relay-global completeness claim.

## Lossless shared-document edits

Kind 3 is shared with clients and extensions Fava may not understand. Every
materialization therefore begins from the newest qualified source and changes
only entries whose lowercase `p` target parses as the requested public key.

- `follow` keeps the first matching target entry byte-for-byte and removes later
  matching duplicates. If no matching entry exists, it appends one canonical
  entry.
- `follow_with` uses its relay hint and petname only when it appends a missing
  target. An existing first entry remains authoritative.
- `unfollow` removes every matching target entry.
- Event content and every non-target entry retain their bytes and order. This
  includes unknown entries such as `["something-something"]`, extension entries such
  as `["t", "nostr"]`, malformed unrelated `p` entries, extra values, and
  unrelated valid follows.

The accepted author and receipt stay fixed while the publication owner may
rematerialize the edit over newer qualified source state. Stale signing or
delivery work from an older materialization cannot advance the current one.

## API surface

```text
follow(target) -> Result<ReplaceableEventEdit, WriteIntentError>
unfollow(target) -> Result<ReplaceableEventEdit, WriteIntentError>
follow_with(target, Option<RelayUrl>, Option<&str>)
    -> Result<ReplaceableEventEdit, WriteIntentError>
materializer() -> Arc<dyn ReplaceableEventMaterializer>

contact_list(authors) -> Result<Query, QueryError>
followers_of(subject) -> Result<Query, QueryError>
follows_of(&QuerySnapshot) -> Vec<PublicKey>

ContactList::from_event(&EventValue) -> Result<ContactList, ContactListError>
ContactList::{author,follows,entry_errors,supersedes}
Follow::{source_index,pubkey,relay,petname}
ContactListEntryError::{source_index,raw_tag}
```

Targets accept `PublicKey`, hex strings, and owned hex strings. Invalid input is
a typed refusal without echoing the raw value. Parsing and edits enforce event,
tag-count, and byte bounds; over-bound input is refused, never truncated.

## Executable evidence

The README surface is exercised by:

- `crates/fava-nip02/tests/public_api.rs` for every exported NIP-02 function and
  type used above;
- `crates/fava-nip02/src/tests/contact_list.rs` for empty lists, ordered typed
  entries, complete entry evidence, exact UTF-8 petnames, and event-level refusal;
- `crates/fava-nip02/src/tests/query.rs` for one/many/empty author axes,
  per-author newest selection, exact follower queries, and two-hop projection;
- `crates/fava-nip02/src/tests/edit.rs` for metadata encoding, rebasing,
  foreign-tag/content preservation, bounds, and redacted errors;
- `crates/fava/tests/publication_scopes.rs` and
  `crates/fava/tests/semantic_write_publication.rs` for `by`/`to` composition,
  synchronous acceptance, immediate query visibility, and `Write::settled`.

Run the focused contract:

```sh
cargo test -p fava-nip02 --doc
cargo test -p fava-nip02 --test public_api
cargo test -p fava-nip02 --all-targets
cargo test -p fava --test publication_scopes
cargo test -p fava --test semantic_write_publication
```

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_nip02` (Module)

Compiler-visible module `fava_nip02`.
<!-- api-item {"kind":"Module","item":"fava_nip02","signature":"pub mod fava_nip02","evidence":"cargo-public-api@0.52.0: pub mod fava_nip02"} -->

### `ContactList` (Struct)

Compiler-visible struct `fava_nip02::ContactList`.
<!-- api-item {"kind":"Struct","item":"fava_nip02::ContactList","signature":"pub struct fava_nip02::ContactList","evidence":"cargo-public-api@0.52.0: pub struct fava_nip02::ContactList"} -->

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactList::author","signature":"pub const fn fava_nip02::ContactList::author(&self) -> nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::ContactList::author(&self) -> nostr::key::public_key::PublicKey"} --> | Compiler-visible method owned by `fava_nip02::ContactList`. |
| **`entry_errors`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactList::entry_errors","signature":"pub const fn fava_nip02::ContactList::entry_errors(&self) -> &[fava_nip02::ContactListEntryError]","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::ContactList::entry_errors(&self) -> &[fava_nip02::ContactListEntryError]"} --> | Compiler-visible method owned by `fava_nip02::ContactList`. |
| **`follows`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactList::follows","signature":"pub const fn fava_nip02::ContactList::follows(&self) -> &[fava_nip02::Follow]","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::ContactList::follows(&self) -> &[fava_nip02::Follow]"} --> | Compiler-visible method owned by `fava_nip02::ContactList`. |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactList::from_event","signature":"pub fn fava_nip02::ContactList::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_nip02::ContactListError>","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::ContactList::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_nip02::ContactListError>"} --> | Compiler-visible method owned by `fava_nip02::ContactList`. |
| **`supersedes`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactList::supersedes","signature":"pub fn fava_nip02::ContactList::supersedes(&self, &Self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::ContactList::supersedes(&self, &Self) -> bool"} --> | Compiler-visible method owned by `fava_nip02::ContactList`. |

### `ContactListEntryError` (Enum)

Compiler-visible enum `fava_nip02::ContactListEntryError`.
<!-- api-item {"kind":"Enum","item":"fava_nip02::ContactListEntryError","signature":"pub enum fava_nip02::ContactListEntryError","evidence":"cargo-public-api@0.52.0: pub enum fava_nip02::ContactListEntryError"} -->

| Item | Purpose |
| --- | --- |
| **`DuplicateTarget`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListEntryError::DuplicateTarget","signature":"pub fava_nip02::ContactListEntryError::DuplicateTarget","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::DuplicateTarget"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListEntryError`. |
| **`Field `pubkey` of `DuplicateTarget``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::DuplicateTarget::pubkey","signature":"pub fava_nip02::ContactListEntryError::DuplicateTarget::pubkey: nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::DuplicateTarget::pubkey: nostr::key::public_key::PublicKey"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `raw_tag` of `DuplicateTarget``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::DuplicateTarget::raw_tag","signature":"pub fava_nip02::ContactListEntryError::DuplicateTarget::raw_tag: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::DuplicateTarget::raw_tag: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `source_index` of `DuplicateTarget``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::DuplicateTarget::source_index","signature":"pub fava_nip02::ContactListEntryError::DuplicateTarget::source_index: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::DuplicateTarget::source_index: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`InvalidPublicKey`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListEntryError::InvalidPublicKey","signature":"pub fava_nip02::ContactListEntryError::InvalidPublicKey","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidPublicKey"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListEntryError`. |
| **`Field `raw_tag` of `InvalidPublicKey``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::InvalidPublicKey::raw_tag","signature":"pub fava_nip02::ContactListEntryError::InvalidPublicKey::raw_tag: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidPublicKey::raw_tag: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `source_index` of `InvalidPublicKey``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::InvalidPublicKey::source_index","signature":"pub fava_nip02::ContactListEntryError::InvalidPublicKey::source_index: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidPublicKey::source_index: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`InvalidRelayHint`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListEntryError::InvalidRelayHint","signature":"pub fava_nip02::ContactListEntryError::InvalidRelayHint","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidRelayHint"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListEntryError`. |
| **`Field `raw_tag` of `InvalidRelayHint``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::InvalidRelayHint::raw_tag","signature":"pub fava_nip02::ContactListEntryError::InvalidRelayHint::raw_tag: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidRelayHint::raw_tag: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `source_index` of `InvalidRelayHint``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::InvalidRelayHint::source_index","signature":"pub fava_nip02::ContactListEntryError::InvalidRelayHint::source_index: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::InvalidRelayHint::source_index: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`MissingTarget`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListEntryError::MissingTarget","signature":"pub fava_nip02::ContactListEntryError::MissingTarget","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::MissingTarget"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListEntryError`. |
| **`Field `raw_tag` of `MissingTarget``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::MissingTarget::raw_tag","signature":"pub fava_nip02::ContactListEntryError::MissingTarget::raw_tag: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::MissingTarget::raw_tag: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `source_index` of `MissingTarget``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::MissingTarget::source_index","signature":"pub fava_nip02::ContactListEntryError::MissingTarget::source_index: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::MissingTarget::source_index: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`UninterpretedExtraValues`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListEntryError::UninterpretedExtraValues","signature":"pub fava_nip02::ContactListEntryError::UninterpretedExtraValues","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::UninterpretedExtraValues"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListEntryError`. |
| **`Field `raw_tag` of `UninterpretedExtraValues``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::UninterpretedExtraValues::raw_tag","signature":"pub fava_nip02::ContactListEntryError::UninterpretedExtraValues::raw_tag: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::UninterpretedExtraValues::raw_tag: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`Field `source_index` of `UninterpretedExtraValues``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListEntryError::UninterpretedExtraValues::source_index","signature":"pub fava_nip02::ContactListEntryError::UninterpretedExtraValues::source_index: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListEntryError::UninterpretedExtraValues::source_index: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListEntryError`. |
| **`raw_tag`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactListEntryError::raw_tag","signature":"pub fn fava_nip02::ContactListEntryError::raw_tag(&self) -> &[alloc::string::String]","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::ContactListEntryError::raw_tag(&self) -> &[alloc::string::String]"} --> | Compiler-visible method owned by `fava_nip02::ContactListEntryError`. |
| **`source_index`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::ContactListEntryError::source_index","signature":"pub const fn fava_nip02::ContactListEntryError::source_index(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::ContactListEntryError::source_index(&self) -> usize"} --> | Compiler-visible method owned by `fava_nip02::ContactListEntryError`. |

### `ContactListError` (Enum)

Compiler-visible enum `fava_nip02::ContactListError`.
<!-- api-item {"kind":"Enum","item":"fava_nip02::ContactListError","signature":"pub enum fava_nip02::ContactListError","evidence":"cargo-public-api@0.52.0: pub enum fava_nip02::ContactListError"} -->

| Item | Purpose |
| --- | --- |
| **`DuplicateRelay`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::DuplicateRelay","signature":"pub fava_nip02::ContactListError::DuplicateRelay","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::DuplicateRelay"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `relay` of `DuplicateRelay``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::DuplicateRelay::relay","signature":"pub fava_nip02::ContactListError::DuplicateRelay::relay: nostr::types::url::RelayUrl","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::DuplicateRelay::relay: nostr::types::url::RelayUrl"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`Encoding`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::Encoding","signature":"pub fava_nip02::ContactListError::Encoding(alloc::string::String)","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::Encoding(alloc::string::String)"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `0` of `Encoding``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::Encoding::0","signature":"alloc::string::String","evidence":"cargo-public-api@0.52.0: alloc::string::String"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`InvalidEvent`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::InvalidEvent","signature":"pub fava_nip02::ContactListError::InvalidEvent(alloc::string::String)","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::InvalidEvent(alloc::string::String)"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `0` of `InvalidEvent``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::InvalidEvent::0","signature":"alloc::string::String","evidence":"cargo-public-api@0.52.0: alloc::string::String"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`InvalidRoute`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::InvalidRoute","signature":"pub fava_nip02::ContactListError::InvalidRoute(alloc::string::String)","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::InvalidRoute(alloc::string::String)"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `0` of `InvalidRoute``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::InvalidRoute::0","signature":"alloc::string::String","evidence":"cargo-public-api@0.52.0: alloc::string::String"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`MissingEventId`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::MissingEventId","signature":"pub fava_nip02::ContactListError::MissingEventId","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::MissingEventId"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`TooLarge`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::TooLarge","signature":"pub fava_nip02::ContactListError::TooLarge","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooLarge"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `bytes` of `TooLarge``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::TooLarge::bytes","signature":"pub fava_nip02::ContactListError::TooLarge::bytes: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooLarge::bytes: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`Field `maximum` of `TooLarge``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::TooLarge::maximum","signature":"pub fava_nip02::ContactListError::TooLarge::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooLarge::maximum: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`TooManyTags`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::TooManyTags","signature":"pub fava_nip02::ContactListError::TooManyTags","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooManyTags"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `actual` of `TooManyTags``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::TooManyTags::actual","signature":"pub fava_nip02::ContactListError::TooManyTags::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooManyTags::actual: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`Field `maximum` of `TooManyTags``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::TooManyTags::maximum","signature":"pub fava_nip02::ContactListError::TooManyTags::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::TooManyTags::maximum: usize"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`WrongKind`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip02::ContactListError::WrongKind","signature":"pub fava_nip02::ContactListError::WrongKind(u16)","evidence":"cargo-public-api@0.52.0: pub fava_nip02::ContactListError::WrongKind(u16)"} --> | Compiler-visible enum variant owned by `fava_nip02::ContactListError`. |
| **`Field `0` of `WrongKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip02::ContactListError::WrongKind::0","signature":"u16","evidence":"cargo-public-api@0.52.0: u16"} --> | Compiler-visible public field owned by `fava_nip02::ContactListError`. |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_nip02::ContactListError as core::fmt::Display>::fmt","signature":"pub fn fava_nip02::ContactListError::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::ContactListError::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result"} --> | Compiler-visible method owned by `fava_nip02::ContactListError`. |

### `Follow` (Struct)

Compiler-visible struct `fava_nip02::Follow`.
<!-- api-item {"kind":"Struct","item":"fava_nip02::Follow","signature":"pub struct fava_nip02::Follow","evidence":"cargo-public-api@0.52.0: pub struct fava_nip02::Follow"} -->

| Item | Purpose |
| --- | --- |
| **`petname`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::Follow::petname","signature":"pub fn fava_nip02::Follow::petname(&self) -> core::option::Option<&str>","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::Follow::petname(&self) -> core::option::Option<&str>"} --> | Compiler-visible method owned by `fava_nip02::Follow`. |
| **`pubkey`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::Follow::pubkey","signature":"pub const fn fava_nip02::Follow::pubkey(&self) -> nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::Follow::pubkey(&self) -> nostr::key::public_key::PublicKey"} --> | Compiler-visible method owned by `fava_nip02::Follow`. |
| **`relay`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::Follow::relay","signature":"pub const fn fava_nip02::Follow::relay(&self) -> core::option::Option<&nostr::types::url::RelayUrl>","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::Follow::relay(&self) -> core::option::Option<&nostr::types::url::RelayUrl>"} --> | Compiler-visible method owned by `fava_nip02::Follow`. |
| **`source_index`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::Follow::source_index","signature":"pub const fn fava_nip02::Follow::source_index(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip02::Follow::source_index(&self) -> usize"} --> | Compiler-visible method owned by `fava_nip02::Follow`. |

### `IntoContactAuthors` (Trait)

Compiler-visible trait `fava_nip02::IntoContactAuthors`.
<!-- api-item {"kind":"Trait","item":"fava_nip02::IntoContactAuthors","signature":"pub trait fava_nip02::IntoContactAuthors: sealed::Sealed","evidence":"cargo-public-api@0.52.0: pub trait fava_nip02::IntoContactAuthors: sealed::Sealed"} -->

| Item | Purpose |
| --- | --- |
| **`IntoIter`**<br><sub>Type alias</sub><!-- api-item {"kind":"Type alias","item":"fava_nip02::IntoContactAuthors::IntoIter","signature":"pub type fava_nip02::IntoContactAuthors::IntoIter: core::iter::traits::iterator::Iterator<Item = nostr::key::public_key::PublicKey>","evidence":"cargo-public-api@0.52.0: pub type fava_nip02::IntoContactAuthors::IntoIter: core::iter::traits::iterator::Iterator<Item = nostr::key::public_key::PublicKey>"} --> | Compiler-visible type alias owned by `fava_nip02::IntoContactAuthors`. |
| **`into_contact_authors`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip02::IntoContactAuthors::into_contact_authors","signature":"pub fn fava_nip02::IntoContactAuthors::into_contact_authors(self) -> Self::IntoIter","evidence":"cargo-public-api@0.52.0: pub fn fava_nip02::IntoContactAuthors::into_contact_authors(self) -> Self::IntoIter"} --> | Compiler-visible method owned by `fava_nip02::IntoContactAuthors`. |
<!-- END crate-readme-api inventory -->
