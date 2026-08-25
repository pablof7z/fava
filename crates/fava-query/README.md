# fava-query

Declarative event-query values and query-owned construction rules. Literal
authors, ids, kinds, tag values, and explicit relays are collected under one
provisional 4,096-input resource-safety cap per operation; the cap is not a
protocol fact or domain semantic. Construction returns exact `QueryError`
refusals before work opens.

`tag_values` unions values into an axis. `intersect_tag_values` adds an AND
constraint without exposing query representation: an absent axis becomes the
supplied set, an existing axis narrows by intersection, and a disjoint axis
stays present-empty to match nothing. Both paths use the same tag-input cap.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_query` (Module)

Compiler-visible module `fava_query`.
<!-- api-item {"kind":"Module","item":"fava_query","signature":"pub mod fava_query","evidence":"cargo-public-api@0.52.0: pub mod fava_query"} -->

| Item | Purpose |
| --- | --- |
| **`EventId`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::EventId","signature":"pub use fava_query::EventId","evidence":"cargo-public-api@0.52.0: pub use fava_query::EventId"} --> | Compiler-visible public field owned by `fava_query`. |
| **`Kind`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::Kind","signature":"pub use fava_query::Kind","evidence":"cargo-public-api@0.52.0: pub use fava_query::Kind"} --> | Compiler-visible public field owned by `fava_query`. |
| **`PublicKey`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::PublicKey","signature":"pub use fava_query::PublicKey","evidence":"cargo-public-api@0.52.0: pub use fava_query::PublicKey"} --> | Compiler-visible public field owned by `fava_query`. |
| **`RelayUrl`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayUrl","signature":"pub use fava_query::RelayUrl","evidence":"cargo-public-api@0.52.0: pub use fava_query::RelayUrl"} --> | Compiler-visible public field owned by `fava_query`. |
| **`SingleLetterTag`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SingleLetterTag","signature":"pub use fava_query::SingleLetterTag","evidence":"cargo-public-api@0.52.0: pub use fava_query::SingleLetterTag"} --> | Compiler-visible public field owned by `fava_query`. |
| **`SourceChangeFuture`**<br><sub>Type alias</sub><!-- api-item {"kind":"Type alias","item":"fava_query::SourceChangeFuture","signature":"pub type fava_query::SourceChangeFuture<'a> = core::pin::Pin<alloc::boxed::Box<(dyn core::future::future::Future<Output = core::result::Result<fava_query::SourceSnapshot, fava_query::QuerySourceClosed>> + core::marker::Send + 'a)>>","evidence":"cargo-public-api@0.52.0: pub type fava_query::SourceChangeFuture<'a> = core::pin::Pin<alloc::boxed::Box<(dyn core::future::future::Future<Output = core::result::Result<fava_query::SourceSnapshot, fava_query::QuerySourceClosed>> + core::marker::Send + 'a)>>"} --> | Compiler-visible type alias owned by `fava_query`. |
| **`Timestamp`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::Timestamp","signature":"pub use fava_query::Timestamp","evidence":"cargo-public-api@0.52.0: pub use fava_query::Timestamp"} --> | Compiler-visible public field owned by `fava_query`. |

### `AuthenticationState` (Enum)

Compiler-visible enum `fava_query::AuthenticationState`.
<!-- api-item {"kind":"Enum","item":"fava_query::AuthenticationState","signature":"pub enum fava_query::AuthenticationState","evidence":"cargo-public-api@0.52.0: pub enum fava_query::AuthenticationState"} -->

| Item | Purpose |
| --- | --- |
| **`AcceptedButStillRefused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::AuthenticationState::AcceptedButStillRefused","signature":"pub fava_query::AuthenticationState::AcceptedButStillRefused","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::AcceptedButStillRefused"} --> | Compiler-visible enum variant owned by `fava_query::AuthenticationState`. |
| **`Attempted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::AuthenticationState::Attempted","signature":"pub fava_query::AuthenticationState::Attempted","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::Attempted"} --> | Compiler-visible enum variant owned by `fava_query::AuthenticationState`. |
| **`ChallengeReceived`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::AuthenticationState::ChallengeReceived","signature":"pub fava_query::AuthenticationState::ChallengeReceived","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::ChallengeReceived"} --> | Compiler-visible enum variant owned by `fava_query::AuthenticationState`. |
| **`Declined`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::AuthenticationState::Declined","signature":"pub fava_query::AuthenticationState::Declined","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::Declined"} --> | Compiler-visible enum variant owned by `fava_query::AuthenticationState`. |
| **`Rejected`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::AuthenticationState::Rejected","signature":"pub fava_query::AuthenticationState::Rejected","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::Rejected"} --> | Compiler-visible enum variant owned by `fava_query::AuthenticationState`. |
| **`Field `message` of `Rejected``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::AuthenticationState::Rejected::message","signature":"pub fava_query::AuthenticationState::Rejected::message: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::AuthenticationState::Rejected::message: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::AuthenticationState`. |

### `BoundedText` (Struct)

Compiler-visible struct `fava_query::BoundedText`.
<!-- api-item {"kind":"Struct","item":"fava_query::BoundedText","signature":"pub struct fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub struct fava_query::BoundedText"} -->

| Item | Purpose |
| --- | --- |
| **`MAX_BYTES`**<br><sub>Constant</sub><!-- api-item {"kind":"Constant","item":"fava_query::BoundedText::MAX_BYTES","signature":"pub const fava_query::BoundedText::MAX_BYTES: usize","evidence":"cargo-public-api@0.52.0: pub const fava_query::BoundedText::MAX_BYTES: usize"} --> | Compiler-visible constant owned by `fava_query::BoundedText`. |
| **`as_str`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::BoundedText::as_str","signature":"pub fn fava_query::BoundedText::as_str(&self) -> &str","evidence":"cargo-public-api@0.52.0: pub fn fava_query::BoundedText::as_str(&self) -> &str"} --> | Compiler-visible method owned by `fava_query::BoundedText`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::BoundedText::new","signature":"pub fn fava_query::BoundedText::new(impl core::convert::AsRef<str>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::BoundedText::new(impl core::convert::AsRef<str>) -> Self"} --> | Compiler-visible method owned by `fava_query::BoundedText`. |
| **`truncated_bytes`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::BoundedText::truncated_bytes","signature":"pub const fn fava_query::BoundedText::truncated_bytes(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::BoundedText::truncated_bytes(&self) -> usize"} --> | Compiler-visible method owned by `fava_query::BoundedText`. |

### `DesiredPlanEvidence` (Struct)

Compiler-visible struct `fava_query::DesiredPlanEvidence`.
<!-- api-item {"kind":"Struct","item":"fava_query::DesiredPlanEvidence","signature":"pub struct fava_query::DesiredPlanEvidence","evidence":"cargo-public-api@0.52.0: pub struct fava_query::DesiredPlanEvidence"} -->

| Item | Purpose |
| --- | --- |
| **`installed`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::DesiredPlanEvidence::installed","signature":"pub fava_query::DesiredPlanEvidence::installed: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::DesiredPlanEvidence::installed: usize"} --> | Compiler-visible public field owned by `fava_query::DesiredPlanEvidence`. |
| **`relays`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::DesiredPlanEvidence::relays","signature":"pub fava_query::DesiredPlanEvidence::relays: alloc::vec::Vec<fava_state::RelaySessionKey>","evidence":"cargo-public-api@0.52.0: pub fava_query::DesiredPlanEvidence::relays: alloc::vec::Vec<fava_state::RelaySessionKey>"} --> | Compiler-visible public field owned by `fava_query::DesiredPlanEvidence`. |
| **`revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::DesiredPlanEvidence::revision","signature":"pub fava_query::DesiredPlanEvidence::revision: u64","evidence":"cargo-public-api@0.52.0: pub fava_query::DesiredPlanEvidence::revision: u64"} --> | Compiler-visible public field owned by `fava_query::DesiredPlanEvidence`. |

### `EventRecord` (Struct)

Compiler-visible struct `fava_query::EventRecord`.
<!-- api-item {"kind":"Struct","item":"fava_query::EventRecord","signature":"pub struct fava_query::EventRecord","evidence":"cargo-public-api@0.52.0: pub struct fava_query::EventRecord"} -->

| Item | Purpose |
| --- | --- |
| **`created_at`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::EventRecord::created_at","signature":"pub fn fava_query::EventRecord::created_at(&self) -> nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fn fava_query::EventRecord::created_at(&self) -> nostr::types::time::Timestamp"} --> | Compiler-visible method owned by `fava_query::EventRecord`. |
| **`event`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::EventRecord::event","signature":"pub fava_query::EventRecord::event: fava_write::EventValue","evidence":"cargo-public-api@0.52.0: pub fava_query::EventRecord::event: fava_write::EventValue"} --> | Compiler-visible public field owned by `fava_query::EventRecord`. |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::EventRecord::id","signature":"pub const fn fava_query::EventRecord::id(&self) -> nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::EventRecord::id(&self) -> nostr::event::id::EventId"} --> | Compiler-visible method owned by `fava_query::EventRecord`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::EventRecord::new","signature":"pub fn fava_query::EventRecord::new(fava_write::EventValue, fava_state::RelayEvidence, core::option::Option<fava_write::receipt::PublicationEvidence>) -> core::result::Result<Self, fava_query::QueryEvaluationError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::EventRecord::new(fava_write::EventValue, fava_state::RelayEvidence, core::option::Option<fava_write::receipt::PublicationEvidence>) -> core::result::Result<Self, fava_query::QueryEvaluationError>"} --> | Compiler-visible method owned by `fava_query::EventRecord`. |
| **`publication`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::EventRecord::publication","signature":"pub fava_query::EventRecord::publication: core::option::Option<fava_write::receipt::PublicationEvidence>","evidence":"cargo-public-api@0.52.0: pub fava_query::EventRecord::publication: core::option::Option<fava_write::receipt::PublicationEvidence>"} --> | Compiler-visible public field owned by `fava_query::EventRecord`. |
| **`relay_evidence`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::EventRecord::relay_evidence","signature":"pub fava_query::EventRecord::relay_evidence: fava_state::RelayEvidence","evidence":"cargo-public-api@0.52.0: pub fava_query::EventRecord::relay_evidence: fava_state::RelayEvidence"} --> | Compiler-visible public field owned by `fava_query::EventRecord`. |

### `FilterSelection` (Struct)

Compiler-visible struct `fava_query::FilterSelection`.
<!-- api-item {"kind":"Struct","item":"fava_query::FilterSelection","signature":"pub struct fava_query::FilterSelection","evidence":"cargo-public-api@0.52.0: pub struct fava_query::FilterSelection"} -->

| Item | Purpose |
| --- | --- |
| **`authors`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::FilterSelection::authors","signature":"pub fava_query::FilterSelection::authors: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::key::public_key::PublicKey>>","evidence":"cargo-public-api@0.52.0: pub fava_query::FilterSelection::authors: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::key::public_key::PublicKey>>"} --> | Compiler-visible public field owned by `fava_query::FilterSelection`. |
| **`ids`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::FilterSelection::ids","signature":"pub fava_query::FilterSelection::ids: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::event::id::EventId>>","evidence":"cargo-public-api@0.52.0: pub fava_query::FilterSelection::ids: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::event::id::EventId>>"} --> | Compiler-visible public field owned by `fava_query::FilterSelection`. |
| **`kinds`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::FilterSelection::kinds","signature":"pub fava_query::FilterSelection::kinds: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::event::kind::Kind>>","evidence":"cargo-public-api@0.52.0: pub fava_query::FilterSelection::kinds: core::option::Option<alloc::collections::btree::set::BTreeSet<nostr::event::kind::Kind>>"} --> | Compiler-visible public field owned by `fava_query::FilterSelection`. |
| **`tag_values`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::FilterSelection::tag_values","signature":"pub fava_query::FilterSelection::tag_values: alloc::collections::btree::map::BTreeMap<nostr::filter::single_letter::SingleLetterTag, alloc::collections::btree::set::BTreeSet<alloc::string::String>>","evidence":"cargo-public-api@0.52.0: pub fava_query::FilterSelection::tag_values: alloc::collections::btree::map::BTreeMap<nostr::filter::single_letter::SingleLetterTag, alloc::collections::btree::set::BTreeSet<alloc::string::String>>"} --> | Compiler-visible public field owned by `fava_query::FilterSelection`. |

### `Freshness` (Enum)

Compiler-visible enum `fava_query::Freshness`.
<!-- api-item {"kind":"Enum","item":"fava_query::Freshness","signature":"pub enum fava_query::Freshness","evidence":"cargo-public-api@0.52.0: pub enum fava_query::Freshness"} -->

| Item | Purpose |
| --- | --- |
| **`CacheOnly`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::Freshness::CacheOnly","signature":"pub fava_query::Freshness::CacheOnly","evidence":"cargo-public-api@0.52.0: pub fava_query::Freshness::CacheOnly"} --> | Compiler-visible enum variant owned by `fava_query::Freshness`. |
| **`Live`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::Freshness::Live","signature":"pub fava_query::Freshness::Live","evidence":"cargo-public-api@0.52.0: pub fava_query::Freshness::Live"} --> | Compiler-visible enum variant owned by `fava_query::Freshness`. |

### `ObservationId` (Struct)

Compiler-visible struct `fava_query::ObservationId`.
<!-- api-item {"kind":"Struct","item":"fava_query::ObservationId","signature":"pub struct fava_query::ObservationId(_)","evidence":"cargo-public-api@0.52.0: pub struct fava_query::ObservationId(_)"} -->

| Item | Purpose |
| --- | --- |
| **`get`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::ObservationId::get","signature":"pub const fn fava_query::ObservationId::get(self) -> core::num::nonzero::NonZeroU64","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::ObservationId::get(self) -> core::num::nonzero::NonZeroU64"} --> | Compiler-visible method owned by `fava_query::ObservationId`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::ObservationId::new","signature":"pub const fn fava_query::ObservationId::new(core::num::nonzero::NonZeroU64) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::ObservationId::new(core::num::nonzero::NonZeroU64) -> Self"} --> | Compiler-visible method owned by `fava_query::ObservationId`. |

### `ObservationIds` (Struct)

Compiler-visible struct `fava_query::ObservationIds`.
<!-- api-item {"kind":"Struct","item":"fava_query::ObservationIds","signature":"pub struct fava_query::ObservationIds","evidence":"cargo-public-api@0.52.0: pub struct fava_query::ObservationIds"} -->

| Item | Purpose |
| --- | --- |
| **`allocate`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::ObservationIds::allocate","signature":"pub fn fava_query::ObservationIds::allocate(&self) -> core::option::Option<fava_query::ObservationId>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::ObservationIds::allocate(&self) -> core::option::Option<fava_query::ObservationId>"} --> | Compiler-visible method owned by `fava_query::ObservationIds`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::ObservationIds::new","signature":"pub const fn fava_query::ObservationIds::new() -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::ObservationIds::new() -> Self"} --> | Compiler-visible method owned by `fava_query::ObservationIds`. |

### `OpenedQuerySource` (Struct)

Compiler-visible struct `fava_query::OpenedQuerySource`.
<!-- api-item {"kind":"Struct","item":"fava_query::OpenedQuerySource","signature":"pub struct fava_query::OpenedQuerySource","evidence":"cargo-public-api@0.52.0: pub struct fava_query::OpenedQuerySource"} -->

| Item | Purpose |
| --- | --- |
| **`changes`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::OpenedQuerySource::changes","signature":"pub fava_query::OpenedQuerySource::changes: alloc::boxed::Box<dyn fava_query::SourceChanges>","evidence":"cargo-public-api@0.52.0: pub fava_query::OpenedQuerySource::changes: alloc::boxed::Box<dyn fava_query::SourceChanges>"} --> | Compiler-visible public field owned by `fava_query::OpenedQuerySource`. |
| **`initial`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::OpenedQuerySource::initial","signature":"pub fava_query::OpenedQuerySource::initial: fava_query::SourceSnapshot","evidence":"cargo-public-api@0.52.0: pub fava_query::OpenedQuerySource::initial: fava_query::SourceSnapshot"} --> | Compiler-visible public field owned by `fava_query::OpenedQuerySource`. |

### `OperationGeneration` (Struct)

Compiler-visible struct `fava_query::OperationGeneration`.
<!-- api-item {"kind":"Struct","item":"fava_query::OperationGeneration","signature":"pub struct fava_query::OperationGeneration(pub u64)","evidence":"cargo-public-api@0.52.0: pub struct fava_query::OperationGeneration(pub u64)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::OperationGeneration::0","signature":"pub u64","evidence":"cargo-public-api@0.52.0: pub u64"} --> | Compiler-visible public field owned by `fava_query::OperationGeneration`. |
| **`next`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::OperationGeneration::next","signature":"pub const fn fava_query::OperationGeneration::next(self) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::OperationGeneration::next(self) -> Self"} --> | Compiler-visible method owned by `fava_query::OperationGeneration`. |

### `Query` (Struct)

Compiler-visible struct `fava_query::Query`.
<!-- api-item {"kind":"Struct","item":"fava_query::Query","signature":"pub struct fava_query::Query","evidence":"cargo-public-api@0.52.0: pub struct fava_query::Query"} -->

| Item | Purpose |
| --- | --- |
| **`access`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::access","signature":"pub const fn fava_query::Query::access(&self) -> &fava_state::RelayAccess","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::access(&self) -> &fava_state::RelayAccess"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`authors`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::authors","signature":"pub fn fava_query::Query::authors(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::key::public_key::PublicKey>) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::authors(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::key::public_key::PublicKey>) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`cache_only`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::cache_only","signature":"pub const fn fava_query::Query::cache_only(self) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::cache_only(self) -> Self"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`core::default::Default::default`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_query::Query as core::default::Default>::default","signature":"pub fn fava_query::Query::default() -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::default() -> Self"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`events`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::events","signature":"pub fn fava_query::Query::events() -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::events() -> Self"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`freshness`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::freshness","signature":"pub const fn fava_query::Query::freshness(&self) -> fava_query::Freshness","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::freshness(&self) -> fava_query::Freshness"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`from_relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::from_relays","signature":"pub fn fava_query::Query::from_relays(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::from_relays(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`ids`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::ids","signature":"pub fn fava_query::Query::ids(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::event::id::EventId>) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::ids(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::event::id::EventId>) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`intersect_tag_values`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::intersect_tag_values","signature":"pub fn fava_query::Query::intersect_tag_values<I, S>(self, nostr::filter::single_letter::SingleLetterTag, I) -> core::result::Result<Self, fava_query::QueryError> where I: core::iter::traits::collect::IntoIterator<Item = S>, S: core::convert::Into<alloc::string::String>","evidence":"crates/fava-query/src/selection.rs; crates/fava-query/tests/query_identity.rs; docs/issues/0027-query-tag-axis-composition.md"} --> | Narrows one exact tag axis by set intersection. An absent axis becomes the supplied set; disjoint or empty input remains present-empty match-nothing. Returns exact `TooManyTagValues` under the shared provisional query-input cap and preserves all other query dimensions. |
| **`kinds`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::kinds","signature":"pub fn fava_query::Query::kinds(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::event::kind::Kind>) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::kinds(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::event::kind::Kind>) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`limit`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::limit","signature":"pub fn fava_query::Query::limit(self, usize) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::limit(self, usize) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`oldest_first`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::oldest_first","signature":"pub const fn fava_query::Query::oldest_first(self) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::oldest_first(self) -> Self"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`only_from_relays`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::only_from_relays","signature":"pub fn fava_query::Query::only_from_relays(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>) -> core::result::Result<Self, fava_query::QueryError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::only_from_relays(self, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>) -> core::result::Result<Self, fava_query::QueryError>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`ordering`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::ordering","signature":"pub const fn fava_query::Query::ordering(&self) -> fava_query::QueryOrdering","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::ordering(&self) -> fava_query::QueryOrdering"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`result_limit`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::result_limit","signature":"pub const fn fava_query::Query::result_limit(&self) -> core::option::Option<core::num::nonzero::NonZeroUsize>","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::result_limit(&self) -> core::option::Option<core::num::nonzero::NonZeroUsize>"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`selection`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::selection","signature":"pub const fn fava_query::Query::selection(&self) -> &fava_query::FilterSelection","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::selection(&self) -> &fava_query::FilterSelection"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`source`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::source","signature":"pub const fn fava_query::Query::source(&self) -> &fava_query::QuerySourcePolicy","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::Query::source(&self) -> &fava_query::QuerySourcePolicy"} --> | Compiler-visible method owned by `fava_query::Query`. |
| **`tag_values`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::Query::tag_values","signature":"pub fn fava_query::Query::tag_values<I, S>(self, nostr::filter::single_letter::SingleLetterTag, I) -> core::result::Result<Self, fava_query::QueryError> where I: core::iter::traits::collect::IntoIterator<Item = S>, S: core::convert::Into<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::Query::tag_values<I, S>(self, nostr::filter::single_letter::SingleLetterTag, I) -> core::result::Result<Self, fava_query::QueryError> where I: core::iter::traits::collect::IntoIterator<Item = S>, S: core::convert::Into<alloc::string::String>"} --> | Compiler-visible method owned by `fava_query::Query`. |

### `QueryAcquisition` (Enum)

Compiler-visible enum `fava_query::QueryAcquisition`.
<!-- api-item {"kind":"Enum","item":"fava_query::QueryAcquisition","signature":"pub enum fava_query::QueryAcquisition","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QueryAcquisition"} -->

| Item | Purpose |
| --- | --- |
| **`Automatic`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryAcquisition::Automatic","signature":"pub fava_query::QueryAcquisition::Automatic","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryAcquisition::Automatic"} --> | Compiler-visible enum variant owned by `fava_query::QueryAcquisition`. |
| **`Explicit`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryAcquisition::Explicit","signature":"pub fava_query::QueryAcquisition::Explicit(alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>)","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryAcquisition::Explicit(alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>)"} --> | Compiler-visible enum variant owned by `fava_query::QueryAcquisition`. |
| **`Field `0` of `Explicit``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryAcquisition::Explicit::0","signature":"alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>","evidence":"cargo-public-api@0.52.0: alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>"} --> | Compiler-visible public field owned by `fava_query::QueryAcquisition`. |

### `QueryBounds` (Struct)

Compiler-visible struct `fava_query::QueryBounds`.
<!-- api-item {"kind":"Struct","item":"fava_query::QueryBounds","signature":"pub struct fava_query::QueryBounds","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QueryBounds"} -->

| Item | Purpose |
| --- | --- |
| **`limit`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryBounds::limit","signature":"pub fava_query::QueryBounds::limit: core::option::Option<core::num::nonzero::NonZeroU32>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryBounds::limit: core::option::Option<core::num::nonzero::NonZeroU32>"} --> | Compiler-visible public field owned by `fava_query::QueryBounds`. |
| **`since`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryBounds::since","signature":"pub fava_query::QueryBounds::since: core::option::Option<nostr::types::time::Timestamp>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryBounds::since: core::option::Option<nostr::types::time::Timestamp>"} --> | Compiler-visible public field owned by `fava_query::QueryBounds`. |
| **`until`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryBounds::until","signature":"pub fava_query::QueryBounds::until: core::option::Option<nostr::types::time::Timestamp>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryBounds::until: core::option::Option<nostr::types::time::Timestamp>"} --> | Compiler-visible public field owned by `fava_query::QueryBounds`. |

### `QueryBranchId` (Struct)

Compiler-visible struct `fava_query::QueryBranchId`.
<!-- api-item {"kind":"Struct","item":"fava_query::QueryBranchId","signature":"pub struct fava_query::QueryBranchId(pub u32)","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QueryBranchId(pub u32)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryBranchId::0","signature":"pub u32","evidence":"cargo-public-api@0.52.0: pub u32"} --> | Compiler-visible public field owned by `fava_query::QueryBranchId`. |
| **`ROOT`**<br><sub>Constant</sub><!-- api-item {"kind":"Constant","item":"fava_query::QueryBranchId::ROOT","signature":"pub const fava_query::QueryBranchId::ROOT: Self","evidence":"cargo-public-api@0.52.0: pub const fava_query::QueryBranchId::ROOT: Self"} --> | Compiler-visible constant owned by `fava_query::QueryBranchId`. |

### `QueryError` (Enum)

Compiler-visible enum `fava_query::QueryError`.
<!-- api-item {"kind":"Enum","item":"fava_query::QueryError","signature":"pub enum fava_query::QueryError","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QueryError"} -->

| Item | Purpose |
| --- | --- |
| **`EmptyExplicitRelays`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::EmptyExplicitRelays","signature":"pub fava_query::QueryError::EmptyExplicitRelays","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::EmptyExplicitRelays"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`TooManyAuthors`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::TooManyAuthors","signature":"pub fava_query::QueryError::TooManyAuthors","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyAuthors"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`Field `actual` of `TooManyAuthors``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyAuthors::actual","signature":"pub fava_query::QueryError::TooManyAuthors::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyAuthors::actual: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`Field `maximum` of `TooManyAuthors``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyAuthors::maximum","signature":"pub fava_query::QueryError::TooManyAuthors::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyAuthors::maximum: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`TooManyExplicitRelays`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::TooManyExplicitRelays","signature":"pub fava_query::QueryError::TooManyExplicitRelays","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyExplicitRelays"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`Field `actual` of `TooManyExplicitRelays``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyExplicitRelays::actual","signature":"pub fava_query::QueryError::TooManyExplicitRelays::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyExplicitRelays::actual: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`Field `maximum` of `TooManyExplicitRelays``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyExplicitRelays::maximum","signature":"pub fava_query::QueryError::TooManyExplicitRelays::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyExplicitRelays::maximum: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`TooManyIds`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::TooManyIds","signature":"pub fava_query::QueryError::TooManyIds","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyIds"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`Field `actual` of `TooManyIds``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyIds::actual","signature":"pub fava_query::QueryError::TooManyIds::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyIds::actual: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`Field `maximum` of `TooManyIds``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyIds::maximum","signature":"pub fava_query::QueryError::TooManyIds::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyIds::maximum: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`TooManyKinds`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::TooManyKinds","signature":"pub fava_query::QueryError::TooManyKinds","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyKinds"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`Field `actual` of `TooManyKinds``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyKinds::actual","signature":"pub fava_query::QueryError::TooManyKinds::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyKinds::actual: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`Field `maximum` of `TooManyKinds``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyKinds::maximum","signature":"pub fava_query::QueryError::TooManyKinds::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyKinds::maximum: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`TooManyTagValues`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::TooManyTagValues","signature":"pub fava_query::QueryError::TooManyTagValues","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyTagValues"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |
| **`Field `actual` of `TooManyTagValues``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyTagValues::actual","signature":"pub fava_query::QueryError::TooManyTagValues::actual: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyTagValues::actual: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`Field `maximum` of `TooManyTagValues``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryError::TooManyTagValues::maximum","signature":"pub fava_query::QueryError::TooManyTagValues::maximum: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::TooManyTagValues::maximum: usize"} --> | Compiler-visible public field owned by `fava_query::QueryError`. |
| **`ZeroLimit`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryError::ZeroLimit","signature":"pub fava_query::QueryError::ZeroLimit","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryError::ZeroLimit"} --> | Compiler-visible enum variant owned by `fava_query::QueryError`. |

### `QueryEvaluationError` (Enum)

Compiler-visible enum `fava_query::QueryEvaluationError`.
<!-- api-item {"kind":"Enum","item":"fava_query::QueryEvaluationError","signature":"pub enum fava_query::QueryEvaluationError","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QueryEvaluationError"} -->

| Item | Purpose |
| --- | --- |
| **`MissingEventId`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryEvaluationError::MissingEventId","signature":"pub fava_query::QueryEvaluationError::MissingEventId","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvaluationError::MissingEventId"} --> | Compiler-visible enum variant owned by `fava_query::QueryEvaluationError`. |
| **`Refused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryEvaluationError::Refused","signature":"pub fava_query::QueryEvaluationError::Refused(fava_query::BoundedText)","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvaluationError::Refused(fava_query::BoundedText)"} --> | Compiler-visible enum variant owned by `fava_query::QueryEvaluationError`. |
| **`Field `0` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryEvaluationError::Refused::0","signature":"fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::QueryEvaluationError`. |

### `QueryEvaluator` (Trait)

Compiler-visible trait `fava_query::QueryEvaluator`.
<!-- api-item {"kind":"Trait","item":"fava_query::QueryEvaluator","signature":"pub trait fava_query::QueryEvaluator: core::marker::Send + core::marker::Sync","evidence":"cargo-public-api@0.52.0: pub trait fava_query::QueryEvaluator: core::marker::Send + core::marker::Sync"} -->

| Item | Purpose |
| --- | --- |
| **`evaluate`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QueryEvaluator::evaluate","signature":"pub fn fava_query::QueryEvaluator::evaluate(&self, &fava_query::Query, &[fava_query::SourceSnapshot]) -> core::result::Result<fava_query::QuerySnapshot, fava_query::QueryEvaluationError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QueryEvaluator::evaluate(&self, &fava_query::Query, &[fava_query::SourceSnapshot]) -> core::result::Result<fava_query::QuerySnapshot, fava_query::QueryEvaluationError>"} --> | Compiler-visible method owned by `fava_query::QueryEvaluator`. |

### `QueryEvidence` (Struct)

Compiler-visible struct `fava_query::QueryEvidence`.
<!-- api-item {"kind":"Struct","item":"fava_query::QueryEvidence","signature":"pub struct fava_query::QueryEvidence","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QueryEvidence"} -->

| Item | Purpose |
| --- | --- |
| **`all_relays_stored_events_complete`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QueryEvidence::all_relays_stored_events_complete","signature":"pub fn fava_query::QueryEvidence::all_relays_stored_events_complete(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QueryEvidence::all_relays_stored_events_complete(&self) -> bool"} --> | Compiler-visible method owned by `fava_query::QueryEvidence`. |
| **`plan`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryEvidence::plan","signature":"pub fava_query::QueryEvidence::plan: core::option::Option<fava_query::DesiredPlanEvidence>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvidence::plan: core::option::Option<fava_query::DesiredPlanEvidence>"} --> | Compiler-visible public field owned by `fava_query::QueryEvidence`. |
| **`relay`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QueryEvidence::relay","signature":"pub fn fava_query::QueryEvidence::relay(&self, &fava_state::RelaySessionKey) -> core::option::Option<&fava_query::RelayQueryEvidence>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QueryEvidence::relay(&self, &fava_state::RelaySessionKey) -> core::option::Option<&fava_query::RelayQueryEvidence>"} --> | Compiler-visible method owned by `fava_query::QueryEvidence`. |
| **`relays`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryEvidence::relays","signature":"pub fava_query::QueryEvidence::relays: alloc::vec::Vec<fava_query::RelayQueryEvidence>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvidence::relays: alloc::vec::Vec<fava_query::RelayQueryEvidence>"} --> | Compiler-visible public field owned by `fava_query::QueryEvidence`. |
| **`relays_at`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QueryEvidence::relays_at","signature":"pub fn fava_query::QueryEvidence::relays_at<'a>(&'a self, &'a nostr::types::url::RelayUrl) -> impl core::iter::traits::iterator::Iterator<Item = &'a fava_query::RelayQueryEvidence>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QueryEvidence::relays_at<'a>(&'a self, &'a nostr::types::url::RelayUrl) -> impl core::iter::traits::iterator::Iterator<Item = &'a fava_query::RelayQueryEvidence>"} --> | Compiler-visible method owned by `fava_query::QueryEvidence`. |
| **`shortfalls`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryEvidence::shortfalls","signature":"pub fava_query::QueryEvidence::shortfalls: alloc::vec::Vec<fava_query::QueryShortfall>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvidence::shortfalls: alloc::vec::Vec<fava_query::QueryShortfall>"} --> | Compiler-visible public field owned by `fava_query::QueryEvidence`. |
| **`source`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QueryEvidence::source","signature":"pub fn fava_query::QueryEvidence::source(&self, &fava_query::SourceKind) -> core::option::Option<&fava_query::SourceEvidence>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QueryEvidence::source(&self, &fava_query::SourceKind) -> core::option::Option<&fava_query::SourceEvidence>"} --> | Compiler-visible method owned by `fava_query::QueryEvidence`. |
| **`sources`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryEvidence::sources","signature":"pub fava_query::QueryEvidence::sources: alloc::vec::Vec<fava_query::SourceEvidence>","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryEvidence::sources: alloc::vec::Vec<fava_query::SourceEvidence>"} --> | Compiler-visible public field owned by `fava_query::QueryEvidence`. |

### `QueryOrdering` (Enum)

Compiler-visible enum `fava_query::QueryOrdering`.
<!-- api-item {"kind":"Enum","item":"fava_query::QueryOrdering","signature":"pub enum fava_query::QueryOrdering","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QueryOrdering"} -->

| Item | Purpose |
| --- | --- |
| **`NewestFirst`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryOrdering::NewestFirst","signature":"pub fava_query::QueryOrdering::NewestFirst","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryOrdering::NewestFirst"} --> | Compiler-visible enum variant owned by `fava_query::QueryOrdering`. |
| **`OldestFirst`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryOrdering::OldestFirst","signature":"pub fava_query::QueryOrdering::OldestFirst","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryOrdering::OldestFirst"} --> | Compiler-visible enum variant owned by `fava_query::QueryOrdering`. |

### `QueryRevision` (Struct)

Compiler-visible struct `fava_query::QueryRevision`.
<!-- api-item {"kind":"Struct","item":"fava_query::QueryRevision","signature":"pub struct fava_query::QueryRevision(pub u64)","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QueryRevision(pub u64)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryRevision::0","signature":"pub u64","evidence":"cargo-public-api@0.52.0: pub u64"} --> | Compiler-visible public field owned by `fava_query::QueryRevision`. |

### `QueryShortfall` (Enum)

Compiler-visible enum `fava_query::QueryShortfall`.
<!-- api-item {"kind":"Enum","item":"fava_query::QueryShortfall","signature":"pub enum fava_query::QueryShortfall","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QueryShortfall"} -->

| Item | Purpose |
| --- | --- |
| **`CoalescedUpdates`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryShortfall::CoalescedUpdates","signature":"pub fava_query::QueryShortfall::CoalescedUpdates","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::CoalescedUpdates"} --> | Compiler-visible enum variant owned by `fava_query::QueryShortfall`. |
| **`Field `dropped` of `CoalescedUpdates``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryShortfall::CoalescedUpdates::dropped","signature":"pub fava_query::QueryShortfall::CoalescedUpdates::dropped: u64","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::CoalescedUpdates::dropped: u64"} --> | Compiler-visible public field owned by `fava_query::QueryShortfall`. |
| **`ResultLimitApplied`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryShortfall::ResultLimitApplied","signature":"pub fava_query::QueryShortfall::ResultLimitApplied","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::ResultLimitApplied"} --> | Compiler-visible enum variant owned by `fava_query::QueryShortfall`. |
| **`Field `limit` of `ResultLimitApplied``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryShortfall::ResultLimitApplied::limit","signature":"pub fava_query::QueryShortfall::ResultLimitApplied::limit: core::num::nonzero::NonZeroUsize","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::ResultLimitApplied::limit: core::num::nonzero::NonZeroUsize"} --> | Compiler-visible public field owned by `fava_query::QueryShortfall`. |
| **`SourceUnavailable`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QueryShortfall::SourceUnavailable","signature":"pub fava_query::QueryShortfall::SourceUnavailable","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::SourceUnavailable"} --> | Compiler-visible enum variant owned by `fava_query::QueryShortfall`. |
| **`Field `detail` of `SourceUnavailable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryShortfall::SourceUnavailable::detail","signature":"pub fava_query::QueryShortfall::SourceUnavailable::detail: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::SourceUnavailable::detail: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::QueryShortfall`. |
| **`Field `kind` of `SourceUnavailable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QueryShortfall::SourceUnavailable::kind","signature":"pub fava_query::QueryShortfall::SourceUnavailable::kind: fava_query::SourceKind","evidence":"cargo-public-api@0.52.0: pub fava_query::QueryShortfall::SourceUnavailable::kind: fava_query::SourceKind"} --> | Compiler-visible public field owned by `fava_query::QueryShortfall`. |

### `QuerySnapshot` (Struct)

Compiler-visible struct `fava_query::QuerySnapshot`.
<!-- api-item {"kind":"Struct","item":"fava_query::QuerySnapshot","signature":"pub struct fava_query::QuerySnapshot","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QuerySnapshot"} -->

| Item | Purpose |
| --- | --- |
| **`evaluated`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySnapshot::evaluated","signature":"pub fn fava_query::QuerySnapshot::evaluated(alloc::vec::Vec<fava_query::EventRecord>, &[fava_query::SourceSnapshot]) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QuerySnapshot::evaluated(alloc::vec::Vec<fava_query::EventRecord>, &[fava_query::SourceSnapshot]) -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySnapshot`. |
| **`events`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QuerySnapshot::events","signature":"pub fava_query::QuerySnapshot::events: alloc::sync::Arc<[fava_query::EventRecord]>","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySnapshot::events: alloc::sync::Arc<[fava_query::EventRecord]>"} --> | Compiler-visible public field owned by `fava_query::QuerySnapshot`. |
| **`evidence`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QuerySnapshot::evidence","signature":"pub fava_query::QuerySnapshot::evidence: fava_query::QueryEvidence","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySnapshot::evidence: fava_query::QueryEvidence"} --> | Compiler-visible public field owned by `fava_query::QuerySnapshot`. |
| **`revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QuerySnapshot::revision","signature":"pub fava_query::QuerySnapshot::revision: fava_query::QueryRevision","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySnapshot::revision: fava_query::QueryRevision"} --> | Compiler-visible public field owned by `fava_query::QuerySnapshot`. |

### `QuerySource` (Trait)

Compiler-visible trait `fava_query::QuerySource`.
<!-- api-item {"kind":"Trait","item":"fava_query::QuerySource","signature":"pub trait fava_query::QuerySource: core::marker::Send + core::marker::Sync","evidence":"cargo-public-api@0.52.0: pub trait fava_query::QuerySource: core::marker::Send + core::marker::Sync"} -->

| Item | Purpose |
| --- | --- |
| **`open`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySource::open","signature":"pub fn fava_query::QuerySource::open(&self, &fava_query::Query) -> core::result::Result<fava_query::OpenedQuerySource, fava_query::QuerySourceError>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QuerySource::open(&self, &fava_query::Query) -> core::result::Result<fava_query::OpenedQuerySource, fava_query::QuerySourceError>"} --> | Compiler-visible method owned by `fava_query::QuerySource`. |

### `QuerySourceClosed` (Struct)

Compiler-visible struct `fava_query::QuerySourceClosed`.
<!-- api-item {"kind":"Struct","item":"fava_query::QuerySourceClosed","signature":"pub struct fava_query::QuerySourceClosed","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QuerySourceClosed"} -->

| Item | Purpose |
| --- | --- |
| **`cause`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QuerySourceClosed::cause","signature":"pub fava_query::QuerySourceClosed::cause: fava_query::SourceTerminationCause","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySourceClosed::cause: fava_query::SourceTerminationCause"} --> | Compiler-visible public field owned by `fava_query::QuerySourceClosed`. |
| **`local_close`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::local_close","signature":"pub const fn fava_query::QuerySourceClosed::local_close() -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourceClosed::local_close() -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::new","signature":"pub const fn fava_query::QuerySourceClosed::new(fava_query::SourceTerminationCause) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourceClosed::new(fava_query::SourceTerminationCause) -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |
| **`provider_closed`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::provider_closed","signature":"pub const fn fava_query::QuerySourceClosed::provider_closed() -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourceClosed::provider_closed() -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |
| **`provider_failed`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::provider_failed","signature":"pub fn fava_query::QuerySourceClosed::provider_failed(impl core::convert::AsRef<str>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QuerySourceClosed::provider_failed(impl core::convert::AsRef<str>) -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |
| **`shutdown`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::shutdown","signature":"pub const fn fava_query::QuerySourceClosed::shutdown() -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourceClosed::shutdown() -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |
| **`status`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourceClosed::status","signature":"pub fn fava_query::QuerySourceClosed::status(&self) -> fava_query::SourceStatus","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QuerySourceClosed::status(&self) -> fava_query::SourceStatus"} --> | Compiler-visible method owned by `fava_query::QuerySourceClosed`. |

### `QuerySourceError` (Enum)

Compiler-visible enum `fava_query::QuerySourceError`.
<!-- api-item {"kind":"Enum","item":"fava_query::QuerySourceError","signature":"pub enum fava_query::QuerySourceError","evidence":"cargo-public-api@0.52.0: pub enum fava_query::QuerySourceError"} -->

| Item | Purpose |
| --- | --- |
| **`Closed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QuerySourceError::Closed","signature":"pub fava_query::QuerySourceError::Closed","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySourceError::Closed"} --> | Compiler-visible enum variant owned by `fava_query::QuerySourceError`. |
| **`Refused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::QuerySourceError::Refused","signature":"pub fava_query::QuerySourceError::Refused(fava_query::BoundedText)","evidence":"cargo-public-api@0.52.0: pub fava_query::QuerySourceError::Refused(fava_query::BoundedText)"} --> | Compiler-visible enum variant owned by `fava_query::QuerySourceError`. |
| **`Field `0` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::QuerySourceError::Refused::0","signature":"fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::QuerySourceError`. |

### `QuerySourcePolicy` (Struct)

Compiler-visible struct `fava_query::QuerySourcePolicy`.
<!-- api-item {"kind":"Struct","item":"fava_query::QuerySourcePolicy","signature":"pub struct fava_query::QuerySourcePolicy","evidence":"cargo-public-api@0.52.0: pub struct fava_query::QuerySourcePolicy"} -->

| Item | Purpose |
| --- | --- |
| **`acquisition`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourcePolicy::acquisition","signature":"pub const fn fava_query::QuerySourcePolicy::acquisition(&self) -> &fava_query::QueryAcquisition","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourcePolicy::acquisition(&self) -> &fava_query::QueryAcquisition"} --> | Compiler-visible method owned by `fava_query::QuerySourcePolicy`. |
| **`authority`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::QuerySourcePolicy::authority","signature":"pub const fn fava_query::QuerySourcePolicy::authority(&self) -> &fava_query::ResultAuthority","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::QuerySourcePolicy::authority(&self) -> &fava_query::ResultAuthority"} --> | Compiler-visible method owned by `fava_query::QuerySourcePolicy`. |
| **`core::default::Default::default`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_query::QuerySourcePolicy as core::default::Default>::default","signature":"pub fn fava_query::QuerySourcePolicy::default() -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::QuerySourcePolicy::default() -> Self"} --> | Compiler-visible method owned by `fava_query::QuerySourcePolicy`. |

### `RelayDeadline` (Enum)

Compiler-visible enum `fava_query::RelayDeadline`.
<!-- api-item {"kind":"Enum","item":"fava_query::RelayDeadline","signature":"pub enum fava_query::RelayDeadline","evidence":"cargo-public-api@0.52.0: pub enum fava_query::RelayDeadline"} -->

| Item | Purpose |
| --- | --- |
| **`Close`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayDeadline::Close","signature":"pub fava_query::RelayDeadline::Close","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayDeadline::Close"} --> | Compiler-visible enum variant owned by `fava_query::RelayDeadline`. |
| **`Establish`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayDeadline::Establish","signature":"pub fava_query::RelayDeadline::Establish","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayDeadline::Establish"} --> | Compiler-visible enum variant owned by `fava_query::RelayDeadline`. |
| **`Idle`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayDeadline::Idle","signature":"pub fava_query::RelayDeadline::Idle","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayDeadline::Idle"} --> | Compiler-visible enum variant owned by `fava_query::RelayDeadline`. |
| **`Write`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayDeadline::Write","signature":"pub fava_query::RelayDeadline::Write","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayDeadline::Write"} --> | Compiler-visible enum variant owned by `fava_query::RelayDeadline`. |

### `RelayQueryEvidence` (Struct)

Compiler-visible struct `fava_query::RelayQueryEvidence`.
<!-- api-item {"kind":"Struct","item":"fava_query::RelayQueryEvidence","signature":"pub struct fava_query::RelayQueryEvidence","evidence":"cargo-public-api@0.52.0: pub struct fava_query::RelayQueryEvidence"} -->

| Item | Purpose |
| --- | --- |
| **`branches`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::branches","signature":"pub fava_query::RelayQueryEvidence::branches: alloc::vec::Vec<fava_query::QueryBranchId>","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::branches: alloc::vec::Vec<fava_query::QueryBranchId>"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`generation`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::generation","signature":"pub fava_query::RelayQueryEvidence::generation: fava_query::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::generation: fava_query::OperationGeneration"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`is_live`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::RelayQueryEvidence::is_live","signature":"pub fn fava_query::RelayQueryEvidence::is_live(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_query::RelayQueryEvidence::is_live(&self) -> bool"} --> | Compiler-visible method owned by `fava_query::RelayQueryEvidence`. |
| **`plan_revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::plan_revision","signature":"pub fava_query::RelayQueryEvidence::plan_revision: u64","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::plan_revision: u64"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`route`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::route","signature":"pub fava_query::RelayQueryEvidence::route: fava_query::RouteOrigin","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::route: fava_query::RouteOrigin"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`session`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::session","signature":"pub fava_query::RelayQueryEvidence::session: fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::session: fava_state::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`shared_with`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::shared_with","signature":"pub fava_query::RelayQueryEvidence::shared_with: alloc::vec::Vec<fava_query::ObservationId>","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::shared_with: alloc::vec::Vec<fava_query::ObservationId>"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`shortfall`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::shortfall","signature":"pub fava_query::RelayQueryEvidence::shortfall: core::option::Option<fava_query::RelayShortfall>","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::shortfall: core::option::Option<fava_query::RelayShortfall>"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`state`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayQueryEvidence::state","signature":"pub fava_query::RelayQueryEvidence::state: fava_query::RelaySourceState","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayQueryEvidence::state: fava_query::RelaySourceState"} --> | Compiler-visible public field owned by `fava_query::RelayQueryEvidence`. |
| **`stored_events_complete`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::RelayQueryEvidence::stored_events_complete","signature":"pub fn fava_query::RelayQueryEvidence::stored_events_complete(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_query::RelayQueryEvidence::stored_events_complete(&self) -> bool"} --> | Compiler-visible method owned by `fava_query::RelayQueryEvidence`. |

### `RelayShortfall` (Struct)

Compiler-visible struct `fava_query::RelayShortfall`.
<!-- api-item {"kind":"Struct","item":"fava_query::RelayShortfall","signature":"pub struct fava_query::RelayShortfall","evidence":"cargo-public-api@0.52.0: pub struct fava_query::RelayShortfall"} -->

| Item | Purpose |
| --- | --- |
| **`branches`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayShortfall::branches","signature":"pub fava_query::RelayShortfall::branches: alloc::vec::Vec<fava_query::QueryBranchId>","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayShortfall::branches: alloc::vec::Vec<fava_query::QueryBranchId>"} --> | Compiler-visible public field owned by `fava_query::RelayShortfall`. |
| **`detail`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelayShortfall::detail","signature":"pub fava_query::RelayShortfall::detail: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayShortfall::detail: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::RelayShortfall`. |

### `RelaySourceState` (Enum)

Compiler-visible enum `fava_query::RelaySourceState`.
<!-- api-item {"kind":"Enum","item":"fava_query::RelaySourceState","signature":"pub enum fava_query::RelaySourceState","evidence":"cargo-public-api@0.52.0: pub enum fava_query::RelaySourceState"} -->

| Item | Purpose |
| --- | --- |
| **`AuthenticationRequired`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::AuthenticationRequired","signature":"pub fava_query::RelaySourceState::AuthenticationRequired","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::AuthenticationRequired"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `at` of `AuthenticationRequired``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::AuthenticationRequired::at","signature":"pub fava_query::RelaySourceState::AuthenticationRequired::at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::AuthenticationRequired::at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Field `state` of `AuthenticationRequired``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::AuthenticationRequired::state","signature":"pub fava_query::RelaySourceState::AuthenticationRequired::state: fava_query::AuthenticationState","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::AuthenticationRequired::state: fava_query::AuthenticationState"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Connecting`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Connecting","signature":"pub fava_query::RelaySourceState::Connecting","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Connecting"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Disconnected`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Disconnected","signature":"pub fava_query::RelaySourceState::Disconnected","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Disconnected"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `detail` of `Disconnected``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Disconnected::detail","signature":"pub fava_query::RelaySourceState::Disconnected::detail: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Disconnected::detail: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Open`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Open","signature":"pub fava_query::RelaySourceState::Open","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Open"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `requested_at` of `Open``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Open::requested_at","signature":"pub fava_query::RelaySourceState::Open::requested_at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Open::requested_at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Planned`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Planned","signature":"pub fava_query::RelaySourceState::Planned","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Planned"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Refused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Refused","signature":"pub fava_query::RelaySourceState::Refused","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Refused"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `at` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Refused::at","signature":"pub fava_query::RelaySourceState::Refused::at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Refused::at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Field `message` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Refused::message","signature":"pub fava_query::RelaySourceState::Refused::message: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Refused::message: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`StoredEventsComplete`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::StoredEventsComplete","signature":"pub fava_query::RelaySourceState::StoredEventsComplete","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::StoredEventsComplete"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `at` of `StoredEventsComplete``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::StoredEventsComplete::at","signature":"pub fava_query::RelaySourceState::StoredEventsComplete::at: nostr::types::time::Timestamp","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::StoredEventsComplete::at: nostr::types::time::Timestamp"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`TimedOut`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::TimedOut","signature":"pub fava_query::RelaySourceState::TimedOut","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::TimedOut"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `after_ms` of `TimedOut``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::TimedOut::after_ms","signature":"pub fava_query::RelaySourceState::TimedOut::after_ms: u64","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::TimedOut::after_ms: u64"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Field `deadline` of `TimedOut``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::TimedOut::deadline","signature":"pub fava_query::RelaySourceState::TimedOut::deadline: fava_query::RelayDeadline","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::TimedOut::deadline: fava_query::RelayDeadline"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Unreachable`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Unreachable","signature":"pub fava_query::RelaySourceState::Unreachable","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Unreachable"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `attempts` of `Unreachable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Unreachable::attempts","signature":"pub fava_query::RelaySourceState::Unreachable::attempts: usize","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Unreachable::attempts: usize"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Field `detail` of `Unreachable``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Unreachable::detail","signature":"pub fava_query::RelaySourceState::Unreachable::detail: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Unreachable::detail: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |
| **`Withdrawn`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelaySourceState::Withdrawn","signature":"pub fava_query::RelaySourceState::Withdrawn","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Withdrawn"} --> | Compiler-visible enum variant owned by `fava_query::RelaySourceState`. |
| **`Field `reason` of `Withdrawn``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RelaySourceState::Withdrawn::reason","signature":"pub fava_query::RelaySourceState::Withdrawn::reason: fava_query::RelayWithdrawal","evidence":"cargo-public-api@0.52.0: pub fava_query::RelaySourceState::Withdrawn::reason: fava_query::RelayWithdrawal"} --> | Compiler-visible public field owned by `fava_query::RelaySourceState`. |

### `RelayWithdrawal` (Enum)

Compiler-visible enum `fava_query::RelayWithdrawal`.
<!-- api-item {"kind":"Enum","item":"fava_query::RelayWithdrawal","signature":"pub enum fava_query::RelayWithdrawal","evidence":"cargo-public-api@0.52.0: pub enum fava_query::RelayWithdrawal"} -->

| Item | Purpose |
| --- | --- |
| **`ObservationClosed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayWithdrawal::ObservationClosed","signature":"pub fava_query::RelayWithdrawal::ObservationClosed","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayWithdrawal::ObservationClosed"} --> | Compiler-visible enum variant owned by `fava_query::RelayWithdrawal`. |
| **`RouteWithdrawn`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayWithdrawal::RouteWithdrawn","signature":"pub fava_query::RelayWithdrawal::RouteWithdrawn","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayWithdrawal::RouteWithdrawn"} --> | Compiler-visible enum variant owned by `fava_query::RelayWithdrawal`. |
| **`Shutdown`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RelayWithdrawal::Shutdown","signature":"pub fava_query::RelayWithdrawal::Shutdown","evidence":"cargo-public-api@0.52.0: pub fava_query::RelayWithdrawal::Shutdown"} --> | Compiler-visible enum variant owned by `fava_query::RelayWithdrawal`. |

### `ResultAuthority` (Enum)

Compiler-visible enum `fava_query::ResultAuthority`.
<!-- api-item {"kind":"Enum","item":"fava_query::ResultAuthority","signature":"pub enum fava_query::ResultAuthority","evidence":"cargo-public-api@0.52.0: pub enum fava_query::ResultAuthority"} -->

| Item | Purpose |
| --- | --- |
| **`AnyLocal`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::ResultAuthority::AnyLocal","signature":"pub fava_query::ResultAuthority::AnyLocal","evidence":"cargo-public-api@0.52.0: pub fava_query::ResultAuthority::AnyLocal"} --> | Compiler-visible enum variant owned by `fava_query::ResultAuthority`. |
| **`OnlyRelays`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::ResultAuthority::OnlyRelays","signature":"pub fava_query::ResultAuthority::OnlyRelays(alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>)","evidence":"cargo-public-api@0.52.0: pub fava_query::ResultAuthority::OnlyRelays(alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>)"} --> | Compiler-visible enum variant owned by `fava_query::ResultAuthority`. |
| **`Field `0` of `OnlyRelays``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::ResultAuthority::OnlyRelays::0","signature":"alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>","evidence":"cargo-public-api@0.52.0: alloc::collections::btree::set::BTreeSet<nostr::types::url::RelayUrl>"} --> | Compiler-visible public field owned by `fava_query::ResultAuthority`. |

### `RouteOrigin` (Enum)

Compiler-visible enum `fava_query::RouteOrigin`.
<!-- api-item {"kind":"Enum","item":"fava_query::RouteOrigin","signature":"pub enum fava_query::RouteOrigin","evidence":"cargo-public-api@0.52.0: pub enum fava_query::RouteOrigin"} -->

| Item | Purpose |
| --- | --- |
| **`Automatic`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RouteOrigin::Automatic","signature":"pub fava_query::RouteOrigin::Automatic","evidence":"cargo-public-api@0.52.0: pub fava_query::RouteOrigin::Automatic"} --> | Compiler-visible enum variant owned by `fava_query::RouteOrigin`. |
| **`Field `revision` of `Automatic``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::RouteOrigin::Automatic::revision","signature":"pub fava_query::RouteOrigin::Automatic::revision: u64","evidence":"cargo-public-api@0.52.0: pub fava_query::RouteOrigin::Automatic::revision: u64"} --> | Compiler-visible public field owned by `fava_query::RouteOrigin`. |
| **`Explicit`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::RouteOrigin::Explicit","signature":"pub fava_query::RouteOrigin::Explicit","evidence":"cargo-public-api@0.52.0: pub fava_query::RouteOrigin::Explicit"} --> | Compiler-visible enum variant owned by `fava_query::RouteOrigin`. |

### `SourceChanges` (Trait)

Compiler-visible trait `fava_query::SourceChanges`.
<!-- api-item {"kind":"Trait","item":"fava_query::SourceChanges","signature":"pub trait fava_query::SourceChanges: core::marker::Send","evidence":"cargo-public-api@0.52.0: pub trait fava_query::SourceChanges: core::marker::Send"} -->

| Item | Purpose |
| --- | --- |
| **`close`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceChanges::close","signature":"pub fn fava_query::SourceChanges::close(&mut self)","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceChanges::close(&mut self)"} --> | Compiler-visible method owned by `fava_query::SourceChanges`. |
| **`next_change`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceChanges::next_change","signature":"pub fn fava_query::SourceChanges::next_change(&mut self) -> fava_query::SourceChangeFuture<'_>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceChanges::next_change(&mut self) -> fava_query::SourceChangeFuture<'_>"} --> | Compiler-visible method owned by `fava_query::SourceChanges`. |

### `SourceEvent` (Enum)

Compiler-visible enum `fava_query::SourceEvent`.
<!-- api-item {"kind":"Enum","item":"fava_query::SourceEvent","signature":"pub enum fava_query::SourceEvent","evidence":"cargo-public-api@0.52.0: pub enum fava_query::SourceEvent"} -->

| Item | Purpose |
| --- | --- |
| **`Cached`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceEvent::Cached","signature":"pub fava_query::SourceEvent::Cached(fava_state::CachedEvent)","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvent::Cached(fava_state::CachedEvent)"} --> | Compiler-visible enum variant owned by `fava_query::SourceEvent`. |
| **`Field `0` of `Cached``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvent::Cached::0","signature":"fava_state::CachedEvent","evidence":"cargo-public-api@0.52.0: fava_state::CachedEvent"} --> | Compiler-visible public field owned by `fava_query::SourceEvent`. |
| **`Local`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceEvent::Local","signature":"pub fava_query::SourceEvent::Local(fava_write::receipt::LocalWriteEvent)","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvent::Local(fava_write::receipt::LocalWriteEvent)"} --> | Compiler-visible enum variant owned by `fava_query::SourceEvent`. |
| **`Field `0` of `Local``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvent::Local::0","signature":"fava_write::receipt::LocalWriteEvent","evidence":"cargo-public-api@0.52.0: fava_write::receipt::LocalWriteEvent"} --> | Compiler-visible public field owned by `fava_query::SourceEvent`. |

### `SourceEvidence` (Struct)

Compiler-visible struct `fava_query::SourceEvidence`.
<!-- api-item {"kind":"Struct","item":"fava_query::SourceEvidence","signature":"pub struct fava_query::SourceEvidence","evidence":"cargo-public-api@0.52.0: pub struct fava_query::SourceEvidence"} -->

| Item | Purpose |
| --- | --- |
| **`kind`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvidence::kind","signature":"pub fava_query::SourceEvidence::kind: fava_query::SourceKind","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvidence::kind: fava_query::SourceKind"} --> | Compiler-visible public field owned by `fava_query::SourceEvidence`. |
| **`retraction`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceEvidence::retraction","signature":"pub fn fava_query::SourceEvidence::retraction(&self, &nostr::event::id::EventId) -> core::option::Option<&fava_state::RetractionCause>","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceEvidence::retraction(&self, &nostr::event::id::EventId) -> core::option::Option<&fava_state::RetractionCause>"} --> | Compiler-visible method owned by `fava_query::SourceEvidence`. |
| **`retractions`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvidence::retractions","signature":"pub fava_query::SourceEvidence::retractions: alloc::vec::Vec<fava_query::SourceRetraction>","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvidence::retractions: alloc::vec::Vec<fava_query::SourceRetraction>"} --> | Compiler-visible public field owned by `fava_query::SourceEvidence`. |
| **`revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvidence::revision","signature":"pub fava_query::SourceEvidence::revision: fava_query::SourceRevision","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvidence::revision: fava_query::SourceRevision"} --> | Compiler-visible public field owned by `fava_query::SourceEvidence`. |
| **`status`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceEvidence::status","signature":"pub fava_query::SourceEvidence::status: fava_query::SourceStatus","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceEvidence::status: fava_query::SourceStatus"} --> | Compiler-visible public field owned by `fava_query::SourceEvidence`. |

### `SourceKind` (Enum)

Compiler-visible enum `fava_query::SourceKind`.
<!-- api-item {"kind":"Enum","item":"fava_query::SourceKind","signature":"pub enum fava_query::SourceKind","evidence":"cargo-public-api@0.52.0: pub enum fava_query::SourceKind"} -->

| Item | Purpose |
| --- | --- |
| **`EventCache`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceKind::EventCache","signature":"pub fava_query::SourceKind::EventCache","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceKind::EventCache"} --> | Compiler-visible enum variant owned by `fava_query::SourceKind`. |
| **`LiveRelay`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceKind::LiveRelay","signature":"pub fava_query::SourceKind::LiveRelay","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceKind::LiveRelay"} --> | Compiler-visible enum variant owned by `fava_query::SourceKind`. |
| **`Field `session` of `LiveRelay``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceKind::LiveRelay::session","signature":"pub fava_query::SourceKind::LiveRelay::session: fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceKind::LiveRelay::session: fava_state::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_query::SourceKind`. |
| **`WriteStore`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceKind::WriteStore","signature":"pub fava_query::SourceKind::WriteStore","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceKind::WriteStore"} --> | Compiler-visible enum variant owned by `fava_query::SourceKind`. |

### `SourceRetraction` (Struct)

Compiler-visible struct `fava_query::SourceRetraction`.
<!-- api-item {"kind":"Struct","item":"fava_query::SourceRetraction","signature":"pub struct fava_query::SourceRetraction","evidence":"cargo-public-api@0.52.0: pub struct fava_query::SourceRetraction"} -->

| Item | Purpose |
| --- | --- |
| **`cause`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceRetraction::cause","signature":"pub fava_query::SourceRetraction::cause: fava_state::RetractionCause","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceRetraction::cause: fava_state::RetractionCause"} --> | Compiler-visible public field owned by `fava_query::SourceRetraction`. |
| **`event_id`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceRetraction::event_id","signature":"pub fava_query::SourceRetraction::event_id: nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceRetraction::event_id: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_query::SourceRetraction`. |
| **`is_protocol_rule`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceRetraction::is_protocol_rule","signature":"pub const fn fava_query::SourceRetraction::is_protocol_rule(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::SourceRetraction::is_protocol_rule(&self) -> bool"} --> | Compiler-visible method owned by `fava_query::SourceRetraction`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceRetraction::new","signature":"pub const fn fava_query::SourceRetraction::new(nostr::event::id::EventId, fava_state::RetractionCause) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_query::SourceRetraction::new(nostr::event::id::EventId, fava_state::RetractionCause) -> Self"} --> | Compiler-visible method owned by `fava_query::SourceRetraction`. |

### `SourceRevision` (Struct)

Compiler-visible struct `fava_query::SourceRevision`.
<!-- api-item {"kind":"Struct","item":"fava_query::SourceRevision","signature":"pub struct fava_query::SourceRevision(pub u64)","evidence":"cargo-public-api@0.52.0: pub struct fava_query::SourceRevision(pub u64)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceRevision::0","signature":"pub u64","evidence":"cargo-public-api@0.52.0: pub u64"} --> | Compiler-visible public field owned by `fava_query::SourceRevision`. |

### `SourceSnapshot` (Struct)

Compiler-visible struct `fava_query::SourceSnapshot`.
<!-- api-item {"kind":"Struct","item":"fava_query::SourceSnapshot","signature":"pub struct fava_query::SourceSnapshot","evidence":"cargo-public-api@0.52.0: pub struct fava_query::SourceSnapshot"} -->

| Item | Purpose |
| --- | --- |
| **`current`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceSnapshot::current","signature":"pub fn fava_query::SourceSnapshot::current(fava_query::SourceKind, fava_query::SourceRevision, alloc::vec::Vec<fava_query::SourceEvent>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceSnapshot::current(fava_query::SourceKind, fava_query::SourceRevision, alloc::vec::Vec<fava_query::SourceEvent>) -> Self"} --> | Compiler-visible method owned by `fava_query::SourceSnapshot`. |
| **`empty`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_query::SourceSnapshot::empty","signature":"pub fn fava_query::SourceSnapshot::empty(fava_query::SourceKind) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceSnapshot::empty(fava_query::SourceKind) -> Self"} --> | Compiler-visible method owned by `fava_query::SourceSnapshot`. |
| **`events`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceSnapshot::events","signature":"pub fava_query::SourceSnapshot::events: alloc::vec::Vec<fava_query::SourceEvent>","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceSnapshot::events: alloc::vec::Vec<fava_query::SourceEvent>"} --> | Compiler-visible public field owned by `fava_query::SourceSnapshot`. |
| **`kind`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceSnapshot::kind","signature":"pub fava_query::SourceSnapshot::kind: fava_query::SourceKind","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceSnapshot::kind: fava_query::SourceKind"} --> | Compiler-visible public field owned by `fava_query::SourceSnapshot`. |
| **`retractions`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceSnapshot::retractions","signature":"pub fava_query::SourceSnapshot::retractions: alloc::vec::Vec<fava_query::SourceRetraction>","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceSnapshot::retractions: alloc::vec::Vec<fava_query::SourceRetraction>"} --> | Compiler-visible public field owned by `fava_query::SourceSnapshot`. |
| **`revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceSnapshot::revision","signature":"pub fava_query::SourceSnapshot::revision: fava_query::SourceRevision","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceSnapshot::revision: fava_query::SourceRevision"} --> | Compiler-visible public field owned by `fava_query::SourceSnapshot`. |
| **`status`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceSnapshot::status","signature":"pub fava_query::SourceSnapshot::status: fava_query::SourceStatus","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceSnapshot::status: fava_query::SourceStatus"} --> | Compiler-visible public field owned by `fava_query::SourceSnapshot`. |

### `SourceStatus` (Enum)

Compiler-visible enum `fava_query::SourceStatus`.
<!-- api-item {"kind":"Enum","item":"fava_query::SourceStatus","signature":"pub enum fava_query::SourceStatus","evidence":"cargo-public-api@0.52.0: pub enum fava_query::SourceStatus"} -->

| Item | Purpose |
| --- | --- |
| **`Closed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceStatus::Closed","signature":"pub fava_query::SourceStatus::Closed","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceStatus::Closed"} --> | Compiler-visible enum variant owned by `fava_query::SourceStatus`. |
| **`Field `cause` of `Closed``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceStatus::Closed::cause","signature":"pub fava_query::SourceStatus::Closed::cause: fava_query::SourceTerminationCause","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceStatus::Closed::cause: fava_query::SourceTerminationCause"} --> | Compiler-visible public field owned by `fava_query::SourceStatus`. |
| **`Open`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceStatus::Open","signature":"pub fava_query::SourceStatus::Open","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceStatus::Open"} --> | Compiler-visible enum variant owned by `fava_query::SourceStatus`. |

### `SourceTerminationCause` (Enum)

Compiler-visible enum `fava_query::SourceTerminationCause`.
<!-- api-item {"kind":"Enum","item":"fava_query::SourceTerminationCause","signature":"pub enum fava_query::SourceTerminationCause","evidence":"cargo-public-api@0.52.0: pub enum fava_query::SourceTerminationCause"} -->

| Item | Purpose |
| --- | --- |
| **`LocalClose`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceTerminationCause::LocalClose","signature":"pub fava_query::SourceTerminationCause::LocalClose","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceTerminationCause::LocalClose"} --> | Compiler-visible enum variant owned by `fava_query::SourceTerminationCause`. |
| **`ProviderClosed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceTerminationCause::ProviderClosed","signature":"pub fava_query::SourceTerminationCause::ProviderClosed","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceTerminationCause::ProviderClosed"} --> | Compiler-visible enum variant owned by `fava_query::SourceTerminationCause`. |
| **`ProviderFailed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceTerminationCause::ProviderFailed","signature":"pub fava_query::SourceTerminationCause::ProviderFailed","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceTerminationCause::ProviderFailed"} --> | Compiler-visible enum variant owned by `fava_query::SourceTerminationCause`. |
| **`Field `detail` of `ProviderFailed``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_query::SourceTerminationCause::ProviderFailed::detail","signature":"pub fava_query::SourceTerminationCause::ProviderFailed::detail: fava_query::BoundedText","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceTerminationCause::ProviderFailed::detail: fava_query::BoundedText"} --> | Compiler-visible public field owned by `fava_query::SourceTerminationCause`. |
| **`Shutdown`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_query::SourceTerminationCause::Shutdown","signature":"pub fava_query::SourceTerminationCause::Shutdown","evidence":"cargo-public-api@0.52.0: pub fava_query::SourceTerminationCause::Shutdown"} --> | Compiler-visible enum variant owned by `fava_query::SourceTerminationCause`. |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_query::SourceTerminationCause as core::fmt::Display>::fmt","signature":"pub fn fava_query::SourceTerminationCause::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"cargo-public-api@0.52.0: pub fn fava_query::SourceTerminationCause::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result"} --> | Compiler-visible method owned by `fava_query::SourceTerminationCause`. |
<!-- END crate-readme-api inventory -->
