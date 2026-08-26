# fava-nip65

`fava-nip65` decodes one event-shaped value as a kind-10002 relay list and
builds the ordinary bounded query used to acquire relay-list events. It owns
the interpretation of NIP-65 `r` tags, but universal query evaluation owns
replaceable-event winner selection.

## Decode a relay list

`RelayList::from_event` accepts the current `fava_write::EventValue` boundary.
For each tag independently:

- exact lowercase `r` with no marker contributes to read and write;
- exact `read` or `write` contributes only to that set;
- short tags, invalid relay URLs, unknown markers, present empty markers, and
  unrelated tags are ignored;
- later tag cells are ignored and repeated parsed relay identities deduplicate.

The union of the two sets is bounded to 256 distinct relay identities. The
decoder stops at identity 257 and returns no partial list.

```rust
use fava_nip65::RelayList;

# fn inspect(event: &fava_write::EventValue) -> Result<(), Box<dyn std::error::Error>> {
let list = RelayList::from_event(event)?;
println!("author: {}", list.author());
for relay in list.read_relays() {
    println!("reads mentions at {relay}");
}
for relay in list.write_relays() {
    println!("publishes at {relay}");
}
# Ok(())
# }
```

## Query composition

`relay_lists(authors)` returns one ordinary bounded kind-10002 query. The
query evaluator selects the current event per author before `RelayList` parses
the resulting record. The protocol crate retains no duplicate event identity,
timestamp, or winner comparator.

## Current refusals

- `WrongKind { actual }` reports a non-10002 kind.
- `TooManyRelays { actual: 257, maximum: 256 }` reports the exact first
  over-bound distinct relay.

Malformed relay URLs are ignored per tag and are not event-level refusals.

## Executable evidence

```sh
cargo test -p fava-nip65 --doc
cargo test -p fava-nip65 --all-targets
bazel test //crates/fava-nip65:unit_tests
python3 tools/crate_readme_api.py check fava-nip65
```

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_nip65` (Module)

NIP-65 relay-list query and decoder crate.
<!-- api-item {"kind":"Module","item":"fava_nip65","signature":"pub mod fava_nip65","evidence":"cargo-public-api@0.52.0: pub mod fava_nip65"} -->

| Item | Purpose |
| --- | --- |
| **`relay_lists`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_nip65::relay_lists","signature":"pub fn fava_nip65::relay_lists(impl core::iter::traits::collect::IntoIterator<Item = nostr::key::public_key::PublicKey>) -> core::result::Result<fava_query::Query, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_nip65::relay_lists(impl core::iter::traits::collect::IntoIterator<Item = nostr::key::public_key::PublicKey>) -> core::result::Result<fava_query::Query, fava_query::QueryError>"} --> | Builds the ordinary bounded kind-10002 query for exact authors. |

### `RelayList` (Struct)

Decoded author and deterministic read/write relay sets.
<!-- api-item {"kind":"Struct","item":"fava_nip65::RelayList","signature":"pub struct fava_nip65::RelayList","evidence":"cargo-public-api@0.52.0: pub struct fava_nip65::RelayList"} -->

| Item | Purpose |
| --- | --- |
| **`author`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip65::RelayList::author","signature":"pub const fn fava_nip65::RelayList::author(&self) -> nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip65::RelayList::author(&self) -> nostr::key::public_key::PublicKey"} --> | Returns the author whose kind-10002 event was decoded. |
| **`from_event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip65::RelayList::from_event","signature":"pub fn fava_nip65::RelayList::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_nip65::RelayListError>","evidence":"cargo-public-api@0.52.0: pub fn fava_nip65::RelayList::from_event(&fava_write::EventValue) -> core::result::Result<Self, fava_nip65::RelayListError>"} --> | Tolerantly decodes one event-shaped value with an exact distinct-result bound. |
| **`read_relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip65::RelayList::read_relays","signature":"pub const fn fava_nip65::RelayList::read_relays(&self) -> &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip65::RelayList::read_relays(&self) -> &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>"} --> | Borrows the distinct decoded read relay set. |
| **`write_relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_nip65::RelayList::write_relays","signature":"pub const fn fava_nip65::RelayList::write_relays(&self) -> &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>","evidence":"cargo-public-api@0.52.0: pub const fn fava_nip65::RelayList::write_relays(&self) -> &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>"} --> | Borrows the distinct decoded write relay set. |

### `RelayListError` (Enum)

Current event-level relay-list decoding refusals.
<!-- api-item {"kind":"Enum","item":"fava_nip65::RelayListError","signature":"pub enum fava_nip65::RelayListError","evidence":"cargo-public-api@0.52.0: pub enum fava_nip65::RelayListError"} -->

| Item | Purpose |
| --- | --- |
| **`TooManyRelays`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip65::RelayListError::TooManyRelays","signature":"pub fava_nip65::RelayListError::TooManyRelays","evidence":"cargo-public-api@0.52.0: pub fava_nip65::RelayListError::TooManyRelays"} --> | Refuses the 257th distinct valid accepted relay identity. |
| **`Field `actual` of `TooManyRelays``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip65::RelayListError::TooManyRelays::actual","signature":"pub fava_nip65::RelayListError::TooManyRelays::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip65::RelayListError::TooManyRelays::actual: usize"} --> | Distinct accepted relay count at refusal. |
| **`Field `maximum` of `TooManyRelays``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip65::RelayListError::TooManyRelays::maximum","signature":"pub fava_nip65::RelayListError::TooManyRelays::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_nip65::RelayListError::TooManyRelays::maximum: usize"} --> | Declared maximum of 256. |
| **`WrongKind`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_nip65::RelayListError::WrongKind","signature":"pub fava_nip65::RelayListError::WrongKind","evidence":"cargo-public-api@0.52.0: pub fava_nip65::RelayListError::WrongKind"} --> | Refuses an event whose kind is not 10002. |
| **`Field `actual` of `WrongKind``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_nip65::RelayListError::WrongKind::actual","signature":"pub fava_nip65::RelayListError::WrongKind::actual: u16","evidence":"cargo-public-api@0.52.0: pub fava_nip65::RelayListError::WrongKind::actual: u16"} --> | Exact received event kind number. |
<!-- END crate-readme-api inventory -->
