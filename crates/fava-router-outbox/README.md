# fava-router-outbox

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_router_outbox` (Module)

Compiler-visible module `fava_router_outbox`.
<!-- api-item {"kind":"Module","item":"fava_router_outbox","signature":"pub mod fava_router_outbox","evidence":"cargo-public-api@0.52.0: pub mod fava_router_outbox"} -->

### `OutboxRouter` (Struct)

Compiler-visible struct `fava_router_outbox::OutboxRouter`.
<!-- api-item {"kind":"Struct","item":"fava_router_outbox::OutboxRouter","signature":"pub struct fava_router_outbox::OutboxRouter","evidence":"cargo-public-api@0.52.0: pub struct fava_router_outbox::OutboxRouter"} -->

| Item | Purpose |
| --- | --- |
| **`fava_routing::Router::name`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_router_outbox::OutboxRouter as fava_routing::Router>::name","signature":"pub fn fava_router_outbox::OutboxRouter::name(&self) -> &str","evidence":"cargo-public-api@0.52.0: pub fn fava_router_outbox::OutboxRouter::name(&self) -> &str"} --> | Compiler-visible method owned by `fava_router_outbox::OutboxRouter`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_router_outbox::OutboxRouter::new","signature":"pub fn fava_router_outbox::OutboxRouter::new(impl core::convert::Into<alloc::string::String>, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>, alloc::sync::Arc<dyn fava_query::QuerySource>) -> core::result::Result<Self, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_router_outbox::OutboxRouter::new(impl core::convert::Into<alloc::string::String>, impl core::iter::traits::collect::IntoIterator<Item = nostr::types::url::RelayUrl>, alloc::sync::Arc<dyn fava_query::QuerySource>) -> core::result::Result<Self, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_router_outbox::OutboxRouter`. |
| **`fava_routing::Router::open`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_router_outbox::OutboxRouter as fava_routing::Router>::open","signature":"pub fn fava_router_outbox::OutboxRouter::open(&self, fava_routing::RouteRequest, tokio::sync::watch::Receiver<alloc::sync::Arc<fava_routing::RoutePlan>>) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_router_outbox::OutboxRouter::open(&self, fava_routing::RouteRequest, tokio::sync::watch::Receiver<alloc::sync::Arc<fava_routing::RoutePlan>>) -> core::result::Result<alloc::boxed::Box<dyn fava_routing::RouterSession>, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_router_outbox::OutboxRouter`. |
| **`fava_routing::Router::preview`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_router_outbox::OutboxRouter as fava_routing::Router>::preview","signature":"pub fn fava_router_outbox::OutboxRouter::preview(&self, &fava_routing::RouteRequest, &fava_routing::RoutePlan) -> core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>","evidence":"cargo-public-api@0.52.0: pub fn fava_router_outbox::OutboxRouter::preview(&self, &fava_routing::RouteRequest, &fava_routing::RoutePlan) -> core::result::Result<fava_routing::RouteContribution, fava_routing::RouterError>"} --> | Compiler-visible method owned by `fava_router_outbox::OutboxRouter`. |
| **`remember`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_router_outbox::OutboxRouter::remember","signature":"pub fn fava_router_outbox::OutboxRouter::remember(&self, &fava_write::EventValue) -> core::result::Result<bool, fava_nip65::RelayListError>","evidence":"cargo-public-api@0.52.0: pub fn fava_router_outbox::OutboxRouter::remember(&self, &fava_write::EventValue) -> core::result::Result<bool, fava_nip65::RelayListError>"} --> | Compiler-visible method owned by `fava_router_outbox::OutboxRouter`. |
<!-- END crate-readme-api inventory -->
