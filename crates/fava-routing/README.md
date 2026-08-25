# fava-routing

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_routing` (Module)

Compiler-visible module `fava_routing`.
<!-- api-item {"kind":"Module","item":"fava_routing","signature":"pub mod fava_routing","evidence":"cargo-public-api@0.52.0: pub mod fava_routing"} -->

| Item | Purpose |
| --- | --- |
| **`open`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_routing::open","signature":"pub fn fava_routing::open(&[alloc::sync::Arc<dyn fava_routing::Router>], &fava_routing::RouteRequest) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::open(&[alloc::sync::Arc<dyn fava_routing::Router>], &fava_routing::RouteRequest) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>"} --> | Compiler-visible function owned by `fava_routing`. |
| **`preview`**<br><sub>Function</sub><!-- api-item {"kind":"Function","item":"fava_routing::preview","signature":"pub fn fava_routing::preview(&[alloc::sync::Arc<dyn fava_routing::Router>], &fava_routing::RouteRequest) -> core::result::Result<fava_routing::RoutePlan, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::preview(&[alloc::sync::Arc<dyn fava_routing::Router>], &fava_routing::RouteRequest) -> core::result::Result<fava_routing::RoutePlan, fava_routing::RouterError>"} --> | Compiler-visible function owned by `fava_routing`. |

### `CoverageState` (Enum)

Compiler-visible enum `fava_routing::CoverageState`.
<!-- api-item {"kind":"Enum","item":"fava_routing::CoverageState","signature":"pub enum fava_routing::CoverageState","evidence":"cargo-public-api@0.52.0: pub enum fava_routing::CoverageState"} -->

| Item | Purpose |
| --- | --- |
| **`Covered`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::CoverageState::Covered","signature":"pub fava_routing::CoverageState::Covered(alloc::collections::btree::set::BTreeSet<fava_state::RelaySessionKey>)","evidence":"cargo-public-api@0.52.0: pub fava_routing::CoverageState::Covered(alloc::collections::btree::set::BTreeSet<fava_state::RelaySessionKey>)"} --> | Compiler-visible enum variant owned by `fava_routing::CoverageState`. |
| **`Field `0` of `Covered``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::CoverageState::Covered::0","signature":"alloc::collections::btree::set::BTreeSet<fava_state::RelaySessionKey>","evidence":"cargo-public-api@0.52.0: alloc::collections::btree::set::BTreeSet<fava_state::RelaySessionKey>"} --> | Compiler-visible public field owned by `fava_routing::CoverageState`. |
| **`SettledAbsent`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::CoverageState::SettledAbsent","signature":"pub fava_routing::CoverageState::SettledAbsent","evidence":"cargo-public-api@0.52.0: pub fava_routing::CoverageState::SettledAbsent"} --> | Compiler-visible enum variant owned by `fava_routing::CoverageState`. |
| **`Unresolved`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::CoverageState::Unresolved","signature":"pub fava_routing::CoverageState::Unresolved","evidence":"cargo-public-api@0.52.0: pub fava_routing::CoverageState::Unresolved"} --> | Compiler-visible enum variant owned by `fava_routing::CoverageState`. |

### `PlannedRelay` (Struct)

Compiler-visible struct `fava_routing::PlannedRelay`.
<!-- api-item {"kind":"Struct","item":"fava_routing::PlannedRelay","signature":"pub struct fava_routing::PlannedRelay","evidence":"cargo-public-api@0.52.0: pub struct fava_routing::PlannedRelay"} -->

| Item | Purpose |
| --- | --- |
| **`reasons`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::PlannedRelay::reasons","signature":"pub fava_routing::PlannedRelay::reasons: alloc::collections::btree::set::BTreeSet<(alloc::string::String, alloc::string::String)>","evidence":"cargo-public-api@0.52.0: pub fava_routing::PlannedRelay::reasons: alloc::collections::btree::set::BTreeSet<(alloc::string::String, alloc::string::String)>"} --> | Compiler-visible public field owned by `fava_routing::PlannedRelay`. |
| **`session`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::PlannedRelay::session","signature":"pub fava_routing::PlannedRelay::session: fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_routing::PlannedRelay::session: fava_state::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_routing::PlannedRelay`. |
| **`targets`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::PlannedRelay::targets","signature":"pub fava_routing::PlannedRelay::targets: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>","evidence":"cargo-public-api@0.52.0: pub fava_routing::PlannedRelay::targets: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>"} --> | Compiler-visible public field owned by `fava_routing::PlannedRelay`. |

### `RouteContribution` (Struct)

Compiler-visible struct `fava_routing::RouteContribution`.
<!-- api-item {"kind":"Struct","item":"fava_routing::RouteContribution","signature":"pub struct fava_routing::RouteContribution","evidence":"cargo-public-api@0.52.0: pub struct fava_routing::RouteContribution"} -->

| Item | Purpose |
| --- | --- |
| **`coverage`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteContribution::coverage","signature":"pub fava_routing::RouteContribution::coverage: alloc::collections::btree::map::BTreeMap<fava_routing::RouteTarget, fava_routing::CoverageState>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteContribution::coverage: alloc::collections::btree::map::BTreeMap<fava_routing::RouteTarget, fava_routing::CoverageState>"} --> | Compiler-visible public field owned by `fava_routing::RouteContribution`. |
| **`destinations`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteContribution::destinations","signature":"pub fava_routing::RouteContribution::destinations: alloc::vec::Vec<fava_routing::RouteDestination>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteContribution::destinations: alloc::vec::Vec<fava_routing::RouteDestination>"} --> | Compiler-visible public field owned by `fava_routing::RouteContribution`. |
| **`shortfalls`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteContribution::shortfalls","signature":"pub fava_routing::RouteContribution::shortfalls: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteContribution::shortfalls: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_routing::RouteContribution`. |
| **`unresolved`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteContribution::unresolved","signature":"pub fava_routing::RouteContribution::unresolved: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteContribution::unresolved: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>"} --> | Compiler-visible public field owned by `fava_routing::RouteContribution`. |

### `RouteDestination` (Struct)

Compiler-visible struct `fava_routing::RouteDestination`.
<!-- api-item {"kind":"Struct","item":"fava_routing::RouteDestination","signature":"pub struct fava_routing::RouteDestination","evidence":"cargo-public-api@0.52.0: pub struct fava_routing::RouteDestination"} -->

| Item | Purpose |
| --- | --- |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteDestination::new","signature":"pub fn fava_routing::RouteDestination::new(fava_state::RelaySessionKey, alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>, impl core::convert::Into<alloc::string::String>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouteDestination::new(fava_state::RelaySessionKey, alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>, impl core::convert::Into<alloc::string::String>) -> Self"} --> | Compiler-visible method owned by `fava_routing::RouteDestination`. |
| **`reason`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteDestination::reason","signature":"pub fava_routing::RouteDestination::reason: alloc::string::String","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteDestination::reason: alloc::string::String"} --> | Compiler-visible public field owned by `fava_routing::RouteDestination`. |
| **`router`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteDestination::router","signature":"pub fn fava_routing::RouteDestination::router(&self) -> &str","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouteDestination::router(&self) -> &str"} --> | Compiler-visible method owned by `fava_routing::RouteDestination`. |
| **`session`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteDestination::session","signature":"pub fava_routing::RouteDestination::session: fava_state::RelaySessionKey","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteDestination::session: fava_state::RelaySessionKey"} --> | Compiler-visible public field owned by `fava_routing::RouteDestination`. |
| **`targets`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteDestination::targets","signature":"pub fava_routing::RouteDestination::targets: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteDestination::targets: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>"} --> | Compiler-visible public field owned by `fava_routing::RouteDestination`. |

### `RoutePlan` (Struct)

Compiler-visible struct `fava_routing::RoutePlan`.
<!-- api-item {"kind":"Struct","item":"fava_routing::RoutePlan","signature":"pub struct fava_routing::RoutePlan","evidence":"cargo-public-api@0.52.0: pub struct fava_routing::RoutePlan"} -->

| Item | Purpose |
| --- | --- |
| **`coverage`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::coverage","signature":"pub fava_routing::RoutePlan::coverage: alloc::collections::btree::map::BTreeMap<fava_routing::RouteTarget, fava_routing::CoverageState>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::coverage: alloc::collections::btree::map::BTreeMap<fava_routing::RouteTarget, fava_routing::CoverageState>"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |
| **`destinations`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::destinations","signature":"pub fava_routing::RoutePlan::destinations: alloc::collections::btree::map::BTreeMap<fava_state::RelaySessionKey, fava_routing::PlannedRelay>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::destinations: alloc::collections::btree::map::BTreeMap<fava_state::RelaySessionKey, fava_routing::PlannedRelay>"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |
| **`explicit`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RoutePlan::explicit","signature":"pub fn fava_routing::RoutePlan::explicit(impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>, &fava_state::RelayAccess, &alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>) -> core::result::Result<Self, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RoutePlan::explicit(impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>, &fava_state::RelayAccess, &alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>) -> core::result::Result<Self, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_routing::RoutePlan`. |
| **`from_contribution`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RoutePlan::from_contribution","signature":"pub fn fava_routing::RoutePlan::from_contribution(u64, &fava_routing::RouteContribution) -> core::result::Result<Self, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RoutePlan::from_contribution(u64, &fava_routing::RouteContribution) -> core::result::Result<Self, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_routing::RoutePlan`. |
| **`revision`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::revision","signature":"pub fava_routing::RoutePlan::revision: u64","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::revision: u64"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |
| **`settled`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::settled","signature":"pub fava_routing::RoutePlan::settled: bool","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::settled: bool"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |
| **`shortfall`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RoutePlan::shortfall","signature":"pub fn fava_routing::RoutePlan::shortfall(u64, &fava_routing::RouteRequest, alloc::string::String) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RoutePlan::shortfall(u64, &fava_routing::RouteRequest, alloc::string::String) -> Self"} --> | Compiler-visible method owned by `fava_routing::RoutePlan`. |
| **`shortfalls`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::shortfalls","signature":"pub fava_routing::RoutePlan::shortfalls: alloc::vec::Vec<alloc::string::String>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::shortfalls: alloc::vec::Vec<alloc::string::String>"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |
| **`unresolved`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RoutePlan::unresolved","signature":"pub fava_routing::RoutePlan::unresolved: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>","evidence":"cargo-public-api@0.52.0: pub fava_routing::RoutePlan::unresolved: alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>"} --> | Compiler-visible public field owned by `fava_routing::RoutePlan`. |

### `RouteRequest` (Enum)

Compiler-visible enum `fava_routing::RouteRequest`.
<!-- api-item {"kind":"Enum","item":"fava_routing::RouteRequest","signature":"pub enum fava_routing::RouteRequest","evidence":"cargo-public-api@0.52.0: pub enum fava_routing::RouteRequest"} -->

| Item | Purpose |
| --- | --- |
| **`Read`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteRequest::Read","signature":"pub fava_routing::RouteRequest::Read(fava_query::Query)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteRequest::Read(fava_query::Query)"} --> | Compiler-visible enum variant owned by `fava_routing::RouteRequest`. |
| **`Field `0` of `Read``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteRequest::Read::0","signature":"fava_query::Query","evidence":"cargo-public-api@0.52.0: fava_query::Query"} --> | Compiler-visible public field owned by `fava_routing::RouteRequest`. |
| **`Write`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteRequest::Write","signature":"pub fava_routing::RouteRequest::Write(fava_write::EventValue)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteRequest::Write(fava_write::EventValue)"} --> | Compiler-visible enum variant owned by `fava_routing::RouteRequest`. |
| **`Field `0` of `Write``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteRequest::Write::0","signature":"fava_write::EventValue","evidence":"cargo-public-api@0.52.0: fava_write::EventValue"} --> | Compiler-visible public field owned by `fava_routing::RouteRequest`. |
| **`access`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteRequest::access","signature":"pub fn fava_routing::RouteRequest::access(&self) -> fava_state::RelayAccess","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouteRequest::access(&self) -> fava_state::RelayAccess"} --> | Compiler-visible method owned by `fava_routing::RouteRequest`. |
| **`event`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteRequest::event","signature":"pub const fn fava_routing::RouteRequest::event(&self) -> core::option::Option<&fava_write::EventValue>","evidence":"cargo-public-api@0.52.0: pub const fn fava_routing::RouteRequest::event(&self) -> core::option::Option<&fava_write::EventValue>"} --> | Compiler-visible method owned by `fava_routing::RouteRequest`. |
| **`is_read`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteRequest::is_read","signature":"pub const fn fava_routing::RouteRequest::is_read(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub const fn fava_routing::RouteRequest::is_read(&self) -> bool"} --> | Compiler-visible method owned by `fava_routing::RouteRequest`. |
| **`is_write`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteRequest::is_write","signature":"pub const fn fava_routing::RouteRequest::is_write(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub const fn fava_routing::RouteRequest::is_write(&self) -> bool"} --> | Compiler-visible method owned by `fava_routing::RouteRequest`. |
| **`targets`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouteRequest::targets","signature":"pub fn fava_routing::RouteRequest::targets(&self) -> alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouteRequest::targets(&self) -> alloc::collections::btree::set::BTreeSet<fava_routing::RouteTarget>"} --> | Compiler-visible method owned by `fava_routing::RouteRequest`. |

### `RouteTarget` (Enum)

Compiler-visible enum `fava_routing::RouteTarget`.
<!-- api-item {"kind":"Enum","item":"fava_routing::RouteTarget","signature":"pub enum fava_routing::RouteTarget","evidence":"cargo-public-api@0.52.0: pub enum fava_routing::RouteTarget"} -->

| Item | Purpose |
| --- | --- |
| **`Author`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteTarget::Author","signature":"pub fava_routing::RouteTarget::Author(nostr::key::public_key::PublicKey)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteTarget::Author(nostr::key::public_key::PublicKey)"} --> | Compiler-visible enum variant owned by `fava_routing::RouteTarget`. |
| **`Field `0` of `Author``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteTarget::Author::0","signature":"nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: nostr::key::public_key::PublicKey"} --> | Compiler-visible public field owned by `fava_routing::RouteTarget`. |
| **`Recipient`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteTarget::Recipient","signature":"pub fava_routing::RouteTarget::Recipient(nostr::key::public_key::PublicKey)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteTarget::Recipient(nostr::key::public_key::PublicKey)"} --> | Compiler-visible enum variant owned by `fava_routing::RouteTarget`. |
| **`Field `0` of `Recipient``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteTarget::Recipient::0","signature":"nostr::key::public_key::PublicKey","evidence":"cargo-public-api@0.52.0: nostr::key::public_key::PublicKey"} --> | Compiler-visible public field owned by `fava_routing::RouteTarget`. |
| **`ReferencedEvent`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteTarget::ReferencedEvent","signature":"pub fava_routing::RouteTarget::ReferencedEvent(nostr::event::id::EventId)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteTarget::ReferencedEvent(nostr::event::id::EventId)"} --> | Compiler-visible enum variant owned by `fava_routing::RouteTarget`. |
| **`Field `0` of `ReferencedEvent``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouteTarget::ReferencedEvent::0","signature":"nostr::event::id::EventId","evidence":"cargo-public-api@0.52.0: nostr::event::id::EventId"} --> | Compiler-visible public field owned by `fava_routing::RouteTarget`. |
| **`WholeRequest`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouteTarget::WholeRequest","signature":"pub fava_routing::RouteTarget::WholeRequest","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouteTarget::WholeRequest"} --> | Compiler-visible enum variant owned by `fava_routing::RouteTarget`. |

### `Router` (Trait)

Compiler-visible trait `fava_routing::Router`.
<!-- api-item {"kind":"Trait","item":"fava_routing::Router","signature":"pub trait fava_routing::Router: core::marker::Send + core::marker::Sync","evidence":"cargo-public-api@0.52.0: pub trait fava_routing::Router: core::marker::Send + core::marker::Sync"} -->

| Item | Purpose |
| --- | --- |
| **`name`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::Router::name","signature":"pub fn fava_routing::Router::name(&self) -> &str","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::Router::name(&self) -> &str"} --> | Compiler-visible method owned by `fava_routing::Router`. |
| **`open`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::Router::open","signature":"pub fn fava_routing::Router::open(&self, fava_routing::RouteRequest, tokio::sync::watch::Receiver<alloc::sync::Arc<fava_routing::RoutePlan>>) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::Router::open(&self, fava_routing::RouteRequest, tokio::sync::watch::Receiver<alloc::sync::Arc<fava_routing::RoutePlan>>) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_routing::Router`. |
| **`preview`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::Router::preview","signature":"pub fn fava_routing::Router::preview(&self, &fava_routing::RouteRequest, &fava_routing::RoutePlan) -> core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::Router::preview(&self, &fava_routing::RouteRequest, &fava_routing::RoutePlan) -> core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_routing::Router`. |

### `RouterError` (Enum)

Compiler-visible enum `fava_routing::RouterError`.
<!-- api-item {"kind":"Enum","item":"fava_routing::RouterError","signature":"pub enum fava_routing::RouterError","evidence":"cargo-public-api@0.52.0: pub enum fava_routing::RouterError"} -->

| Item | Purpose |
| --- | --- |
| **`Closed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouterError::Closed","signature":"pub fava_routing::RouterError::Closed","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouterError::Closed"} --> | Compiler-visible enum variant owned by `fava_routing::RouterError`. |
| **`Refused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_routing::RouterError::Refused","signature":"pub fava_routing::RouterError::Refused(alloc::string::String)","evidence":"cargo-public-api@0.52.0: pub fava_routing::RouterError::Refused(alloc::string::String)"} --> | Compiler-visible enum variant owned by `fava_routing::RouterError`. |
| **`Field `0` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_routing::RouterError::Refused::0","signature":"alloc::string::String","evidence":"cargo-public-api@0.52.0: alloc::string::String"} --> | Compiler-visible public field owned by `fava_routing::RouterError`. |

### `RouterSession` (Trait)

Compiler-visible trait `fava_routing::RouterSession`.
<!-- api-item {"kind":"Trait","item":"fava_routing::RouterSession","signature":"pub trait fava_routing::RouterSession: core::marker::Send","evidence":"cargo-public-api@0.52.0: pub trait fava_routing::RouterSession: core::marker::Send"} -->

| Item | Purpose |
| --- | --- |
| **`close`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouterSession::close","signature":"pub fn fava_routing::RouterSession::close(&mut self)","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouterSession::close(&mut self)"} --> | Compiler-visible method owned by `fava_routing::RouterSession`. |
| **`current`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouterSession::current","signature":"pub fn fava_routing::RouterSession::current(&self) -> fava_routing::RouteContribution","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouterSession::current(&self) -> fava_routing::RouteContribution"} --> | Compiler-visible method owned by `fava_routing::RouterSession`. |
| **`next_change`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_routing::RouterSession::next_change","signature":"pub fn fava_routing::RouterSession::next_change(&mut self) -> core::pin::Pin<alloc::boxed::Box<(dyn core::future::future::Future<Output = core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>> + core::marker::Send + '_)>>","evidence":"cargo-public-api@0.52.0: pub fn fava_routing::RouterSession::next_change(&mut self) -> core::pin::Pin<alloc::boxed::Box<(dyn core::future::future::Future<Output = core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>> + core::marker::Send + '_)>>"} --> | Compiler-visible method owned by `fava_routing::RouterSession`. |
<!-- END crate-readme-api inventory -->
