# fava-observe

`Observation::wait_until(timeout, predicate)` is the bounded predicate wait
for an already-open query. It examines the installed current snapshot before
awaiting later delivery from that exact handle. `Some` is the matching
snapshot, `None` is expiry of the caller-supplied bound, and
`ObservationClosed` remains the error; timing out leaves the observation, its
demand, and a later completion intact.

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_observe` (Module)

Compiler-visible module `fava_observe`.
<!-- api-item {"kind":"Module","item":"fava_observe","signature":"pub mod fava_observe","evidence":"cargo-public-api@0.52.0: pub mod fava_observe"} -->

| Item | Purpose |
| --- | --- |
| **`ObservationId`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObservationId","signature":"pub use fava_observe::ObservationId","evidence":"cargo-public-api@0.52.0: pub use fava_observe::ObservationId"} --> | Compiler-visible public field owned by `fava_observe`. |
| **`QueryBranchId`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::QueryBranchId","signature":"pub use fava_observe::QueryBranchId","evidence":"cargo-public-api@0.52.0: pub use fava_observe::QueryBranchId"} --> | Compiler-visible public field owned by `fava_observe`. |

### `Observation` (Struct)

Compiler-visible struct `fava_observe::Observation`.
<!-- api-item {"kind":"Struct","item":"fava_observe::Observation","signature":"pub struct fava_observe::Observation","evidence":"cargo-public-api@0.52.0: pub struct fava_observe::Observation"} -->

| Item | Purpose |
| --- | --- |
| **`changed`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observation::changed","signature":"pub async fn fava_observe::Observation::changed(&mut self) -> core::result::Result<alloc::sync::Arc<fava_query::QuerySnapshot>, fava_observe::ObservationClosed>","evidence":"cargo-public-api@0.52.0: pub async fn fava_observe::Observation::changed(&mut self) -> core::result::Result<alloc::sync::Arc<fava_query::QuerySnapshot>, fava_observe::ObservationClosed>"} --> | Compiler-visible method owned by `fava_observe::Observation`. |
| **`close`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observation::close","signature":"pub fn fava_observe::Observation::close(&self)","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observation::close(&self)"} --> | Compiler-visible method owned by `fava_observe::Observation`. |
| **`current`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observation::current","signature":"pub fn fava_observe::Observation::current(&self) -> alloc::sync::Arc<fava_query::QuerySnapshot>","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observation::current(&self) -> alloc::sync::Arc<fava_query::QuerySnapshot>"} --> | Compiler-visible method owned by `fava_observe::Observation`. |
| **`core::ops::drop::Drop::drop`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_observe::Observation as core::ops::drop::Drop>::drop","signature":"pub fn fava_observe::Observation::drop(&mut self)","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observation::drop(&mut self)"} --> | Compiler-visible method owned by `fava_observe::Observation`. |
| **`id`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observation::id","signature":"pub const fn fava_observe::Observation::id(&self) -> fava_query::identity::ObservationId","evidence":"cargo-public-api@0.52.0: pub const fn fava_observe::Observation::id(&self) -> fava_query::identity::ObservationId"} --> | Compiler-visible method owned by `fava_observe::Observation`. |
| **`wait_until`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observation::wait_until","signature":"pub async fn fava_observe::Observation::wait_until(&mut self, core::time::Duration, impl core::ops::function::FnMut(&fava_query::QuerySnapshot) -> bool) -> core::result::Result<core::option::Option<alloc::sync::Arc<fava_query::QuerySnapshot>>, fava_observe::ObservationClosed>","evidence":"cargo-public-api@0.52.0: pub async fn fava_observe::Observation::wait_until(&mut self, core::time::Duration, impl core::ops::function::FnMut(&fava_query::QuerySnapshot) -> bool) -> core::result::Result<core::option::Option<alloc::sync::Arc<fava_query::QuerySnapshot>>, fava_observe::ObservationClosed>"} --> | Compiler-visible method owned by `fava_observe::Observation`. |

### `ObservationClosed` (Struct)

Compiler-visible struct `fava_observe::ObservationClosed`.
<!-- api-item {"kind":"Struct","item":"fava_observe::ObservationClosed","signature":"pub struct fava_observe::ObservationClosed","evidence":"cargo-public-api@0.52.0: pub struct fava_observe::ObservationClosed"} -->

### `ObserveError` (Enum)

Compiler-visible enum `fava_observe::ObserveError`.
<!-- api-item {"kind":"Enum","item":"fava_observe::ObserveError","signature":"pub enum fava_observe::ObserveError","evidence":"cargo-public-api@0.52.0: pub enum fava_observe::ObserveError"} -->

| Item | Purpose |
| --- | --- |
| **`EngineClosed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::EngineClosed","signature":"pub fava_observe::ObserveError::EngineClosed","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::EngineClosed"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Evaluation`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::Evaluation","signature":"pub fava_observe::ObserveError::Evaluation(fava_query::QueryEvaluationError)","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::Evaluation(fava_query::QueryEvaluationError)"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Field `0` of `Evaluation``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::Evaluation::0","signature":"fava_query::QueryEvaluationError","evidence":"cargo-public-api@0.52.0: fava_query::QueryEvaluationError"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |
| **`OperationGenerationExhausted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::OperationGenerationExhausted","signature":"pub fava_observe::ObserveError::OperationGenerationExhausted(fava_query::identity::OperationGenerationExhausted)","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::OperationGenerationExhausted(fava_query::identity::OperationGenerationExhausted)"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Field `0` of `OperationGenerationExhausted``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::OperationGenerationExhausted::0","signature":"fava_query::identity::OperationGenerationExhausted","evidence":"cargo-public-api@0.52.0: fava_query::identity::OperationGenerationExhausted"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |
| **`PlanRevisionExhausted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::PlanRevisionExhausted","signature":"pub fava_observe::ObserveError::PlanRevisionExhausted(fava_subscriptions::plan::PlanRevisionExhausted)","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::PlanRevisionExhausted(fava_subscriptions::plan::PlanRevisionExhausted)"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Field `0` of `PlanRevisionExhausted``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::PlanRevisionExhausted::0","signature":"fava_subscriptions::plan::PlanRevisionExhausted","evidence":"cargo-public-api@0.52.0: fava_subscriptions::plan::PlanRevisionExhausted"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |
| **`Relay`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::Relay","signature":"pub fava_observe::ObserveError::Relay(alloc::string::String)","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::Relay(alloc::string::String)"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Field `0` of `Relay``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::Relay::0","signature":"alloc::string::String","evidence":"cargo-public-api@0.52.0: alloc::string::String"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |
| **`SourceOpen`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_observe::ObserveError::SourceOpen","signature":"pub fava_observe::ObserveError::SourceOpen","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::SourceOpen"} --> | Compiler-visible enum variant owned by `fava_observe::ObserveError`. |
| **`Field `error` of `SourceOpen``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::SourceOpen::error","signature":"pub fava_observe::ObserveError::SourceOpen::error: fava_query::QuerySourceError","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::SourceOpen::error: fava_query::QuerySourceError"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |
| **`Field `role` of `SourceOpen``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_observe::ObserveError::SourceOpen::role","signature":"pub fava_observe::ObserveError::SourceOpen::role: fava_query::evidence::SourceKind","evidence":"cargo-public-api@0.52.0: pub fava_observe::ObserveError::SourceOpen::role: fava_query::evidence::SourceKind"} --> | Compiler-visible public field owned by `fava_observe::ObserveError`. |

### `Observer` (Struct)

Compiler-visible struct `fava_observe::Observer`.
<!-- api-item {"kind":"Struct","item":"fava_observe::Observer","signature":"pub struct fava_observe::Observer","evidence":"cargo-public-api@0.52.0: pub struct fava_observe::Observer"} -->

| Item | Purpose |
| --- | --- |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::new","signature":"pub fn fava_observe::Observer::new(alloc::sync::Arc<dyn fava_query::QuerySource>, alloc::sync::Arc<dyn fava_query::QuerySource>, alloc::sync::Arc<dyn fava_query::QueryEvaluator>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::new(alloc::sync::Arc<dyn fava_query::QuerySource>, alloc::sync::Arc<dyn fava_query::QuerySource>, alloc::sync::Arc<dyn fava_query::QueryEvaluator>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`open`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::open","signature":"pub fn fava_observe::Observer::open(&self, fava_query::Query) -> core::result::Result<fava_observe::Observation, fava_observe::ObserveError>","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::open(&self, fava_query::Query) -> core::result::Result<fava_observe::Observation, fava_observe::ObserveError>"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_admission_window`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_admission_window","signature":"pub const fn fava_observe::Observer::with_admission_window(self, core::time::Duration) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_observe::Observer::with_admission_window(self, core::time::Duration) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_bounds`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_bounds","signature":"pub const fn fava_observe::Observer::with_bounds(self, fava_transport::request::TransportBounds) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_observe::Observer::with_bounds(self, fava_transport::request::TransportBounds) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_coalescing`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_coalescing","signature":"pub fn fava_observe::Observer::with_coalescing(self, alloc::sync::Arc<(dyn core::ops::function::Fn(u64) + core::marker::Send + core::marker::Sync)>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_coalescing(self, alloc::sync::Arc<(dyn core::ops::function::Fn(u64) + core::marker::Send + core::marker::Sync)>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_deadlines`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_deadlines","signature":"pub const fn fava_observe::Observer::with_deadlines(self, fava_transport::request::TransportDeadlines) -> Self","evidence":"cargo-public-api@0.52.0: pub const fn fava_observe::Observer::with_deadlines(self, fava_transport::request::TransportDeadlines) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_diagnostics`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_diagnostics","signature":"pub fn fava_observe::Observer::with_diagnostics(self, alloc::sync::Arc<fava_diagnostics::Diagnostics>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_diagnostics(self, alloc::sync::Arc<fava_diagnostics::Diagnostics>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_event_cache`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_event_cache","signature":"pub fn fava_observe::Observer::with_event_cache(self, alloc::sync::Arc<dyn fava_event_cache::EventCache>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_event_cache(self, alloc::sync::Arc<dyn fava_event_cache::EventCache>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_routers`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_routers","signature":"pub fn fava_observe::Observer::with_routers(self, alloc::vec::Vec<alloc::sync::Arc<dyn fava_routing::Router>>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_routers(self, alloc::vec::Vec<alloc::sync::Arc<dyn fava_routing::Router>>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_runtime`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_runtime","signature":"pub fn fava_observe::Observer::with_runtime(self, fava_runtime::runtime::Runtime) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_runtime(self, fava_runtime::runtime::Runtime) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_subscription_planner`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_subscription_planner","signature":"pub fn fava_observe::Observer::with_subscription_planner(self, alloc::sync::Arc<dyn fava_subscriptions::planner::SubscriptionPlanner>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_subscription_planner(self, alloc::sync::Arc<dyn fava_subscriptions::planner::SubscriptionPlanner>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
| **`with_transport`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_observe::Observer::with_transport","signature":"pub fn fava_observe::Observer::with_transport(self, alloc::sync::Arc<dyn fava_transport::Transport>) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_observe::Observer::with_transport(self, alloc::sync::Arc<dyn fava_transport::Transport>) -> Self"} --> | Compiler-visible method owned by `fava_observe::Observer`. |
<!-- END crate-readme-api inventory -->
