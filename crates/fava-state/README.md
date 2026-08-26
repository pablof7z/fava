# fava-state

Pure universal Nostr event-state values and rules. The crate owns no cache,
provider, observation, serialization, or lifecycle. Callers supply finite
current atomic relay contributions and commit each returned mutation batch
atomically.

An admitted `RelayEvent` contains one signed event and one exact
`RelaySessionKey` occurrence. `relay_occurrences_for_event` creates the private,
event-id-bound aggregate used by query records. Replacement is per exact
session; deletion and expiration may retract several exact contributions.

Winner order is universal: later `created_at` wins, then the lower event id.
Malformed optional or sibling tags remain scoped to themselves.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Descriptions are hand-written and preserved across updates. Re-exports appear
at their exported path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_state` (Module)

Compiler-visible module `fava_state`.
<!-- api-item {"kind":"Module","item":"fava_state","signature":"pub mod fava_state","evidence":"cargo-public-api@0.52.0: pub mod fava_state"} -->

| Item | Purpose |
| --- | --- |
| **`deletion_applies`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::deletion_applies","signature":"pub fn fava_state::deletion_applies((nostr::key::public_key::PublicKey, nostr::event::kind::Kind, nostr::types::time::Timestamp, &[nostr::event::tag::Tag]), (nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, nostr::types::time::Timestamp, &[nostr::event::tag::Tag])) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::deletion_applies((nostr::key::public_key::PublicKey, nostr::event::kind::Kind, nostr::types::time::Timestamp, &[nostr::event::tag::Tag]), (nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, nostr::types::time::Timestamp, &[nostr::event::tag::Tag])) -> bool"} --> | Compiler-visible function owned by `fava_state`. |
| **`event_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::event_coordinate","signature":"pub fn fava_state::event_coordinate(nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, &[nostr::event::tag::Tag]) -> fava_state::EventCoordinate","evidence":"cargo-public-api@0.52.0: pub fn fava_state::event_coordinate(nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, &[nostr::event::tag::Tag]) -> fava_state::EventCoordinate"} --> | Compiler-visible function owned by `fava_state`. |
| **`event_is_expired`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::event_is_expired","signature":"pub fn fava_state::event_is_expired(&[nostr::event::tag::Tag], nostr::types::time::Timestamp) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::event_is_expired(&[nostr::event::tag::Tag], nostr::types::time::Timestamp) -> bool"} --> | Compiler-visible function owned by `fava_state`. |
| **`event_is_newer`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::event_is_newer","signature":"pub fn fava_state::event_is_newer((nostr::types::time::Timestamp, nostr::event::id::EventId), (nostr::types::time::Timestamp, nostr::event::id::EventId)) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::event_is_newer((nostr::types::time::Timestamp, nostr::event::id::EventId), (nostr::types::time::Timestamp, nostr::event::id::EventId)) -> bool"} --> | Compiler-visible function owned by `fava_state`. |
| **`mutations_for_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::mutations_for_event","signature":"pub fn fava_state::mutations_for_event(&[fava_state::RelayEvent], fava_state::RelayEvent, nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::EventStateMutation>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::mutations_for_event(&[fava_state::RelayEvent], fava_state::RelayEvent, nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::EventStateMutation>"} --> | Compiler-visible function owned by `fava_state`. |
| **`mutations_for_expiration`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::mutations_for_expiration","signature":"pub fn fava_state::mutations_for_expiration(&[fava_state::RelayEvent], nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::EventStateMutation>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::mutations_for_expiration(&[fava_state::RelayEvent], nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::EventStateMutation>"} --> | Compiler-visible function owned by `fava_state`. |
| **`relay_occurrences_for_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::relay_occurrences_for_event","signature":"pub fn fava_state::relay_occurrences_for_event(nostr::event::id::EventId, &[fava_state::RelayEvent]) -> core::option::Option<fava_state::RelayOccurrences>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::relay_occurrences_for_event(nostr::event::id::EventId, &[fava_state::RelayEvent]) -> core::option::Option<fava_state::RelayOccurrences>"} --> | Compiler-visible function owned by `fava_state`. |

### `EventCoordinate` (Enum)

Compiler-visible enum `fava_state::EventCoordinate`.
<!-- api-item {"kind":"Enum","item":"fava_state::EventCoordinate","signature":"pub enum fava_state::EventCoordinate","evidence":"cargo-public-api@0.52.0: pub enum fava_state::EventCoordinate"} -->

| Item | Purpose |
| --- | --- |
| **`Event`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::EventCoordinate::Event","signature":"pub fava_state::EventCoordinate::Event(nostr::event::id::EventId)","evidence":"cargo-public-api@0.52.0: pub fava_state::EventCoordinate::Event(nostr::event::id::EventId)"} --> | Compiler-visible enum variant owned by `fava_state::EventCoordinate`. |
| **`Field `0` of `Event``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventCoordinate::Event::0","signature":"nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_state::EventCoordinate`. |
| **`Replaceable`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::EventCoordinate::Replaceable","signature":"pub fava_state::EventCoordinate::Replaceable","evidence":"cargo-public-api@0.52.0: pub fava_state::EventCoordinate::Replaceable"} --> | Compiler-visible enum variant owned by `fava_state::EventCoordinate`. |
| **`Field `author` of `Replaceable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventCoordinate::Replaceable::author","signature":"pub fava_state::EventCoordinate::Replaceable::author: nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: pub fava_state::EventCoordinate::Replaceable::author: nostr::key::public_key::PublicKey"} --> | Compiler-visible public field owned by `fava_state::EventCoordinate`. |
| **`Field `identifier` of `Replaceable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventCoordinate::Replaceable::identifier","signature":"pub fava_state::EventCoordinate::Replaceable::identifier: core::option::Option<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_state::EventCoordinate::Replaceable::identifier: core::option::Option<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_state::EventCoordinate`. |
| **`Field `kind` of `Replaceable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventCoordinate::Replaceable::kind","signature":"pub fava_state::EventCoordinate::Replaceable::kind: nostr::event::kind::Kind","evidence":"cargo-public-api@0.52.0: pub fava_state::EventCoordinate::Replaceable::kind: nostr::event::kind::Kind"} --> | Compiler-visible public field owned by `fava_state::EventCoordinate`. |

### `EventStateMutation` (Enum)

Compiler-visible enum `fava_state::EventStateMutation`.
<!-- api-item {"kind":"Enum","item":"fava_state::EventStateMutation","signature":"pub enum fava_state::EventStateMutation","evidence":"cargo-public-api@0.52.0: pub enum fava_state::EventStateMutation"} -->

| Item | Purpose |
| --- | --- |
| **`Retract`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::EventStateMutation::Retract","signature":"pub fava_state::EventStateMutation::Retract","evidence":"cargo-public-api@0.52.0: pub fava_state::EventStateMutation::Retract"} --> | Compiler-visible enum variant owned by `fava_state::EventStateMutation`. |
| **`Field `cause` of `Retract``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventStateMutation::Retract::cause","signature":"pub fava_state::EventStateMutation::Retract::cause: fava_state::RetractionCause","evidence":"cargo-public-api@0.52.0: pub fava_state::EventStateMutation::Retract::cause: fava_state::RetractionCause"} --> | Compiler-visible public field owned by `fava_state::EventStateMutation`. |
| **`Field `event_id` of `Retract``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventStateMutation::Retract::event_id","signature":"pub fava_state::EventStateMutation::Retract::event_id: nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub fava_state::EventStateMutation::Retract::event_id: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_state::EventStateMutation`. |
| **`Field `session` of `Retract``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventStateMutation::Retract::session","signature":"pub fava_state::EventStateMutation::Retract::session: fava_relay::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_state::EventStateMutation::Retract::session: fava_relay::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_state::EventStateMutation`. |
| **`Upsert`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::EventStateMutation::Upsert","signature":"pub fava_state::EventStateMutation::Upsert(fava_state::RelayEvent)","evidence":"cargo-public-api@0.52.0: pub fava_state::EventStateMutation::Upsert(fava_state::RelayEvent)"} --> | Compiler-visible enum variant owned by `fava_state::EventStateMutation`. |
| **`Field `0` of `Upsert``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventStateMutation::Upsert::0","signature":"fava_state::RelayEvent","evidence":"cargo-public-api@0.52.0: fava_state::RelayEvent"} --> | Compiler-visible public field owned by `fava_state::EventStateMutation`. |

### `RelayEvent` (Struct)

Compiler-visible struct `fava_state::RelayEvent`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayEvent","signature":"pub struct fava_state::RelayEvent","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayEvent"} -->

| Item | Purpose |
| --- | --- |
| **`event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvent::event","signature":"pub const fn fava_state::RelayEvent::event(&self) -> &nostr::event::Event","evidence":"cargo-public-api@0.52.0: pub const fn fava_state::RelayEvent::event(&self) -> &nostr::event::Event"} --> | Compiler-visible method owned by `fava_state::RelayEvent`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvent::new","signature":"pub fn fava_state::RelayEvent::new(nostr::event::Event, fava_relay::RelaySessionKey, nostr::types::time::Timestamp) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvent::new(nostr::event::Event, fava_relay::RelaySessionKey, nostr::types::time::Timestamp) -> Self"} --> | Compiler-visible method owned by `fava_state::RelayEvent`. |
| **`occurrence`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvent::occurrence","signature":"pub const fn fava_state::RelayEvent::occurrence(&self) -> &fava_state::RelayOccurrence","evidence":"cargo-public-api@0.52.0: pub const fn fava_state::RelayEvent::occurrence(&self) -> &fava_state::RelayOccurrence"} --> | Compiler-visible method owned by `fava_state::RelayEvent`. |

### `RelayOccurrence` (Struct)

Compiler-visible struct `fava_state::RelayOccurrence`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayOccurrence","signature":"pub struct fava_state::RelayOccurrence","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayOccurrence"} -->

| Item | Purpose |
| --- | --- |
| **`observed_at`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelayOccurrence::observed_at","signature":"pub fava_state::RelayOccurrence::observed_at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_state::RelayOccurrence::observed_at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_state::RelayOccurrence`. |
| **`session`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelayOccurrence::session","signature":"pub fava_state::RelayOccurrence::session: fava_relay::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_state::RelayOccurrence::session: fava_relay::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_state::RelayOccurrence`. |

### `RelayOccurrences` (Struct)

Compiler-visible struct `fava_state::RelayOccurrences`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayOccurrences","signature":"pub struct fava_state::RelayOccurrences","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayOccurrences"} -->

| Item | Purpose |
| --- | --- |
| **`event_id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayOccurrences::event_id","signature":"pub const fn fava_state::RelayOccurrences::event_id(&self) -> nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub const fn fava_state::RelayOccurrences::event_id(&self) -> nostr::event::id::EventId"} --> | Compiler-visible method owned by `fava_state::RelayOccurrences`. |
| **`is_empty`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayOccurrences::is_empty","signature":"pub fn fava_state::RelayOccurrences::is_empty(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayOccurrences::is_empty(&self) -> bool"} --> | Compiler-visible method owned by `fava_state::RelayOccurrences`. |
| **`len`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayOccurrences::len","signature":"pub fn fava_state::RelayOccurrences::len(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayOccurrences::len(&self) -> usize"} --> | Compiler-visible method owned by `fava_state::RelayOccurrences`. |
| **`occurrences`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayOccurrences::occurrences","signature":"pub fn fava_state::RelayOccurrences::occurrences(&self) -> impl core::iter::traits::iterator::Iterator<Item = &fava_state::RelayOccurrence>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayOccurrences::occurrences(&self) -> impl core::iter::traits::iterator::Iterator<Item = &fava_state::RelayOccurrence>"} --> | Compiler-visible method owned by `fava_state::RelayOccurrences`. |

### `RetractionCause` (Enum)

Compiler-visible enum `fava_state::RetractionCause`.
<!-- api-item {"kind":"Enum","item":"fava_state::RetractionCause","signature":"pub enum fava_state::RetractionCause","evidence":"cargo-public-api@0.52.0: pub enum fava_state::RetractionCause"} -->

| Item | Purpose |
| --- | --- |
| **`Deleted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::RetractionCause::Deleted","signature":"pub fava_state::RetractionCause::Deleted","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Deleted"} --> | Compiler-visible enum variant owned by `fava_state::RetractionCause`. |
| **`Field `deletion` of `Deleted``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RetractionCause::Deleted::deletion","signature":"pub fava_state::RetractionCause::Deleted::deletion: nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Deleted::deletion: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_state::RetractionCause`. |
| **`Evicted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::RetractionCause::Evicted","signature":"pub fava_state::RetractionCause::Evicted","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Evicted"} --> | Compiler-visible enum variant owned by `fava_state::RetractionCause`. |
| **`Expired`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::RetractionCause::Expired","signature":"pub fava_state::RetractionCause::Expired","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Expired"} --> | Compiler-visible enum variant owned by `fava_state::RetractionCause`. |
| **`Superseded`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::RetractionCause::Superseded","signature":"pub fava_state::RetractionCause::Superseded","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Superseded"} --> | Compiler-visible enum variant owned by `fava_state::RetractionCause`. |
| **`Field `by` of `Superseded``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RetractionCause::Superseded::by","signature":"pub fava_state::RetractionCause::Superseded::by: nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Superseded::by: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_state::RetractionCause`. |
<!-- END crate-readme-api inventory -->
