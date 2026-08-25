# fava-state

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_state` (Module)

Compiler-visible module `fava_state`.
<!-- api-item {"kind":"Module","item":"fava_state","signature":"pub mod fava_state","evidence":"cargo-public-api@0.52.0: pub mod fava_state"} -->

| Item | Purpose |
| --- | --- |
| **`Event`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::Event","signature":"pub use fava_state::Event","evidence":"cargo-public-api@0.52.0: pub use fava_state::Event"} --> | Compiler-visible public field owned by `fava_state`. |
| **`EventId`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::EventId","signature":"pub use fava_state::EventId","evidence":"cargo-public-api@0.52.0: pub use fava_state::EventId"} --> | Compiler-visible public field owned by `fava_state`. |
| **`Kind`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::Kind","signature":"pub use fava_state::Kind","evidence":"cargo-public-api@0.52.0: pub use fava_state::Kind"} --> | Compiler-visible public field owned by `fava_state`. |
| **`PublicKey`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::PublicKey","signature":"pub use fava_state::PublicKey","evidence":"cargo-public-api@0.52.0: pub use fava_state::PublicKey"} --> | Compiler-visible public field owned by `fava_state`. |
| **`RelayUrl`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelayUrl","signature":"pub use fava_state::RelayUrl","evidence":"cargo-public-api@0.52.0: pub use fava_state::RelayUrl"} --> | Compiler-visible public field owned by `fava_state`. |
| **`Tag`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::Tag","signature":"pub use fava_state::Tag","evidence":"cargo-public-api@0.52.0: pub use fava_state::Tag"} --> | Compiler-visible public field owned by `fava_state`. |
| **`Timestamp`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::Timestamp","signature":"pub use fava_state::Timestamp","evidence":"cargo-public-api@0.52.0: pub use fava_state::Timestamp"} --> | Compiler-visible public field owned by `fava_state`. |
| **`admission_mutations`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::admission_mutations","signature":"pub fn fava_state::admission_mutations(&[fava_state::CachedEvent], fava_state::CachedEvent, nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::CacheMutation>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::admission_mutations(&[fava_state::CachedEvent], fava_state::CachedEvent, nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::CacheMutation>"} --> | Compiler-visible function owned by `fava_state`. |
| **`candidate_is_newer`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::candidate_is_newer","signature":"pub fn fava_state::candidate_is_newer(&nostr::event::Event, &nostr::event::Event) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::candidate_is_newer(&nostr::event::Event, &nostr::event::Event) -> bool"} --> | Compiler-visible function owned by `fava_state`. |
| **`coordinate_for_event`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::coordinate_for_event","signature":"pub fn fava_state::coordinate_for_event(&nostr::event::Event) -> fava_state::EventCoordinate","evidence":"cargo-public-api@0.52.0: pub fn fava_state::coordinate_for_event(&nostr::event::Event) -> fava_state::EventCoordinate"} --> | Compiler-visible function owned by `fava_state`. |
| **`event_coordinate`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::event_coordinate","signature":"pub fn fava_state::event_coordinate(nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, &[nostr::event::tag::Tag]) -> fava_state::EventCoordinate","evidence":"cargo-public-api@0.52.0: pub fn fava_state::event_coordinate(nostr::event::id::EventId, nostr::key::public_key::PublicKey, nostr::event::kind::Kind, &[nostr::event::tag::Tag]) -> fava_state::EventCoordinate"} --> | Compiler-visible function owned by `fava_state`. |
| **`event_is_expired`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::event_is_expired","signature":"pub fn fava_state::event_is_expired(&nostr::event::Event, nostr::types::time::Timestamp) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::event_is_expired(&nostr::event::Event, nostr::types::time::Timestamp) -> bool"} --> | Compiler-visible function owned by `fava_state`. |
| **`expiration_mutations`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_state::expiration_mutations","signature":"pub fn fava_state::expiration_mutations(&[fava_state::CachedEvent], nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::CacheMutation>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::expiration_mutations(&[fava_state::CachedEvent], nostr::types::time::Timestamp) -> alloc::vec::Vec<fava_state::CacheMutation>"} --> | Compiler-visible function owned by `fava_state`. |

### `CacheMutation` (Enum)

Compiler-visible enum `fava_state::CacheMutation`.
<!-- api-item {"kind":"Enum","item":"fava_state::CacheMutation","signature":"pub enum fava_state::CacheMutation","evidence":"cargo-public-api@0.52.0: pub enum fava_state::CacheMutation"} -->

| Item | Purpose |
| --- | --- |
| **`Retract`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::CacheMutation::Retract","signature":"pub fava_state::CacheMutation::Retract","evidence":"cargo-public-api@0.52.0: pub fava_state::CacheMutation::Retract"} --> | Compiler-visible enum variant owned by `fava_state::CacheMutation`. |
| **`Field `cause` of `Retract``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::CacheMutation::Retract::cause","signature":"pub fava_state::CacheMutation::Retract::cause: fava_state::RetractionCause","evidence":"cargo-public-api@0.52.0: pub fava_state::CacheMutation::Retract::cause: fava_state::RetractionCause"} --> | Compiler-visible public field owned by `fava_state::CacheMutation`. |
| **`Field `event_id` of `Retract``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::CacheMutation::Retract::event_id","signature":"pub fava_state::CacheMutation::Retract::event_id: nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub fava_state::CacheMutation::Retract::event_id: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_state::CacheMutation`. |
| **`Upsert`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_state::CacheMutation::Upsert","signature":"pub fava_state::CacheMutation::Upsert(fava_state::CachedEvent)","evidence":"cargo-public-api@0.52.0: pub fava_state::CacheMutation::Upsert(fava_state::CachedEvent)"} --> | Compiler-visible enum variant owned by `fava_state::CacheMutation`. |
| **`Field `0` of `Upsert``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::CacheMutation::Upsert::0","signature":"fava_state::CachedEvent","evidence":"cargo-public-api@0.52.0: fava_state::CachedEvent"} --> | Compiler-visible public field owned by `fava_state::CacheMutation`. |
| **`is_retraction`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::CacheMutation::is_retraction","signature":"pub const fn fava_state::CacheMutation::is_retraction(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub const fn fava_state::CacheMutation::is_retraction(&self) -> bool"} --> | Compiler-visible method owned by `fava_state::CacheMutation`. |

### `CachedEvent` (Struct)

Compiler-visible struct `fava_state::CachedEvent`.
<!-- api-item {"kind":"Struct","item":"fava_state::CachedEvent","signature":"pub struct fava_state::CachedEvent","evidence":"cargo-public-api@0.52.0: pub struct fava_state::CachedEvent"} -->

| Item | Purpose |
| --- | --- |
| **`event`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::CachedEvent::event","signature":"pub fava_state::CachedEvent::event: nostr::event::Event","evidence":"cargo-public-api@0.52.0: pub fava_state::CachedEvent::event: nostr::event::Event"} --> | Compiler-visible public field owned by `fava_state::CachedEvent`. |
| **`evidence`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::CachedEvent::evidence","signature":"pub fava_state::CachedEvent::evidence: fava_state::RelayEvidence","evidence":"cargo-public-api@0.52.0: pub fava_state::CachedEvent::evidence: fava_state::RelayEvidence"} --> | Compiler-visible public field owned by `fava_state::CachedEvent`. |
| **`merge_evidence`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::CachedEvent::merge_evidence","signature":"pub fn fava_state::CachedEvent::merge_evidence(&mut self, &fava_state::RelayEvidence)","evidence":"cargo-public-api@0.52.0: pub fn fava_state::CachedEvent::merge_evidence(&mut self, &fava_state::RelayEvidence)"} --> | Compiler-visible method owned by `fava_state::CachedEvent`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::CachedEvent::new","signature":"pub fn fava_state::CachedEvent::new(nostr::event::Event, fava_state::RelayEvidence) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::CachedEvent::new(nostr::event::Event, fava_state::RelayEvidence) -> Self"} --> | Compiler-visible method owned by `fava_state::CachedEvent`. |

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

### `RelayAccess` (Struct)

Compiler-visible struct `fava_state::RelayAccess`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayAccess","signature":"pub struct fava_state::RelayAccess(_)","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayAccess(_)"} -->

| Item | Purpose |
| --- | --- |
| **`as_str`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayAccess::as_str","signature":"pub fn fava_state::RelayAccess::as_str(&self) -> &str","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayAccess::as_str(&self) -> &str"} --> | Compiler-visible method owned by `fava_state::RelayAccess`. |
| **`named`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayAccess::named","signature":"pub fn fava_state::RelayAccess::named(impl core::convert::Into<alloc::string::String>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayAccess::named(impl core::convert::Into<alloc::string::String>) -> Self"} --> | Compiler-visible method owned by `fava_state::RelayAccess`. |
| **`public`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayAccess::public","signature":"pub fn fava_state::RelayAccess::public() -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayAccess::public() -> Self"} --> | Compiler-visible method owned by `fava_state::RelayAccess`. |

### `RelayEvidence` (Struct)

Compiler-visible struct `fava_state::RelayEvidence`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayEvidence","signature":"pub struct fava_state::RelayEvidence","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayEvidence"} -->

| Item | Purpose |
| --- | --- |
| **`includes_any_relay`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::includes_any_relay","signature":"pub fn fava_state::RelayEvidence::includes_any_relay(&self, &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::includes_any_relay(&self, &alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>) -> bool"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |
| **`is_empty`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::is_empty","signature":"pub fn fava_state::RelayEvidence::is_empty(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::is_empty(&self) -> bool"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |
| **`len`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::len","signature":"pub fn fava_state::RelayEvidence::len(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::len(&self) -> usize"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |
| **`merge`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::merge","signature":"pub fn fava_state::RelayEvidence::merge(&mut self, &Self)","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::merge(&mut self, &Self)"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |
| **`observations`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::observations","signature":"pub fn fava_state::RelayEvidence::observations(&self) -> impl core::iter::traits::iterator::Iterator<Item = &fava_state::RelayObservation>","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::observations(&self) -> impl core::iter::traits::iterator::Iterator<Item = &fava_state::RelayObservation>"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |
| **`one`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelayEvidence::one","signature":"pub fn fava_state::RelayEvidence::one(fava_state::RelaySessionKey, nostr::types::time::Timestamp) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelayEvidence::one(fava_state::RelaySessionKey, nostr::types::time::Timestamp) -> Self"} --> | Compiler-visible method owned by `fava_state::RelayEvidence`. |

### `RelayObservation` (Struct)

Compiler-visible struct `fava_state::RelayObservation`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelayObservation","signature":"pub struct fava_state::RelayObservation","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelayObservation"} -->

| Item | Purpose |
| --- | --- |
| **`observed_at`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelayObservation::observed_at","signature":"pub fava_state::RelayObservation::observed_at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_state::RelayObservation::observed_at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_state::RelayObservation`. |
| **`session`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelayObservation::session","signature":"pub fava_state::RelayObservation::session: fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_state::RelayObservation::session: fava_state::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_state::RelayObservation`. |

### `RelaySessionKey` (Struct)

Compiler-visible struct `fava_state::RelaySessionKey`.
<!-- api-item {"kind":"Struct","item":"fava_state::RelaySessionKey","signature":"pub struct fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub struct fava_state::RelaySessionKey"} -->

| Item | Purpose |
| --- | --- |
| **`access`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelaySessionKey::access","signature":"pub fava_state::RelaySessionKey::access: fava_state::RelayAccess","evidence":"cargo-public-api@0.52.0: pub fava_state::RelaySessionKey::access: fava_state::RelayAccess"} --> | Compiler-visible public field owned by `fava_state::RelaySessionKey`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_state::RelaySessionKey::new","signature":"pub fn fava_state::RelaySessionKey::new(nostr::types::url::RelayUrl, fava_state::RelayAccess) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_state::RelaySessionKey::new(nostr::types::url::RelayUrl, fava_state::RelayAccess) -> Self"} --> | Compiler-visible method owned by `fava_state::RelaySessionKey`. |
| **`relay`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RelaySessionKey::relay","signature":"pub fava_state::RelaySessionKey::relay: nostr::types::url::RelayUrl","evidence":"cargo-public-api@0.52.0: pub fava_state::RelaySessionKey::relay: nostr::types::url::RelayUrl"} --> | Compiler-visible public field owned by `fava_state::RelaySessionKey`. |

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
| **`Field `coordinate` of `Superseded``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_state::RetractionCause::Superseded::coordinate","signature":"pub fava_state::RetractionCause::Superseded::coordinate: fava_state::EventCoordinate","evidence":"cargo-public-api@0.52.0: pub fava_state::RetractionCause::Superseded::coordinate: fava_state::EventCoordinate"} --> | Compiler-visible public field owned by `fava_state::RetractionCause`. |
<!-- END crate-readme-api inventory -->
