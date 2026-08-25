# fava-runtime

## Complete public API inventory

Generated from rustdoc with `python3 tools/crate_readme_api.py update <crate>`.
Purposes and evidence are preserved across updates. Compiler-derived identities
and signatures are refreshed on every run. Re-exports appear at their exported
path and are classified by the re-exported item's kind.

<!-- BEGIN crate-readme-api inventory -->
### `fava_runtime` (Module)

Compiler-visible module `fava_runtime`.
<!-- api-item {"kind":"Module","item":"fava_runtime","signature":"pub mod fava_runtime","evidence":"cargo-public-api@0.52.0: pub mod fava_runtime"} -->

| Item | Purpose |
| --- | --- |
| **`OperationGeneration`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::OperationGeneration","signature":"pub use fava_runtime::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub use fava_runtime::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime`. |

### `CancellationToken` (Struct)

Compiler-visible struct `fava_runtime::CancellationToken`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::CancellationToken","signature":"pub struct fava_runtime::CancellationToken","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::CancellationToken"} -->

| Item | Purpose |
| --- | --- |
| **`cancel`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::CancellationToken::cancel","signature":"pub fn fava_runtime::CancellationToken::cancel(&self)","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::CancellationToken::cancel(&self)"} --> | Compiler-visible method owned by `fava_runtime::CancellationToken`. |
| **`cancelled`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::CancellationToken::cancelled","signature":"pub async fn fava_runtime::CancellationToken::cancelled(&self)","evidence":"cargo-public-api@0.52.0: pub async fn fava_runtime::CancellationToken::cancelled(&self)"} --> | Compiler-visible method owned by `fava_runtime::CancellationToken`. |
| **`child`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::CancellationToken::child","signature":"pub fn fava_runtime::CancellationToken::child(&self) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::CancellationToken::child(&self) -> Self"} --> | Compiler-visible method owned by `fava_runtime::CancellationToken`. |
| **`is_cancelled`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::CancellationToken::is_cancelled","signature":"pub fn fava_runtime::CancellationToken::is_cancelled(&self) -> bool","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::CancellationToken::is_cancelled(&self) -> bool"} --> | Compiler-visible method owned by `fava_runtime::CancellationToken`. |

### `OperationName` (Struct)

Compiler-visible struct `fava_runtime::OperationName`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::OperationName","signature":"pub struct fava_runtime::OperationName(pub &'static str)","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::OperationName(pub &'static str)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::OperationName::0","signature":"pub &'static str","evidence":"cargo-public-api@0.52.0: pub &'static str"} --> | Compiler-visible public field owned by `fava_runtime::OperationName`. |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_runtime::OperationName as core::fmt::Display>::fmt","signature":"pub fn fava_runtime::OperationName::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::OperationName::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result"} --> | Compiler-visible method owned by `fava_runtime::OperationName`. |

### `ProviderCompletion` (Function)

Compiler-visible function `fava_runtime::ProviderCompletion`.
<!-- api-item {"kind":"Function","item":"fava_runtime::ProviderCompletion","signature":"pub fn fava_runtime::ProviderCompletion<T>::generation(&self) -> fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::ProviderCompletion<T>::generation(&self) -> fava_query::identity::OperationGeneration"} -->

| Item | Purpose |
| --- | --- |
| **`Cancelled`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::ProviderCompletion::Cancelled","signature":"pub fava_runtime::ProviderCompletion::Cancelled","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Cancelled"} --> | Compiler-visible enum variant owned by `fava_runtime::ProviderCompletion`. |
| **`Field `generation` of `Cancelled``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Cancelled::generation","signature":"pub fava_runtime::ProviderCompletion::Cancelled::generation: fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Cancelled::generation: fava_query::identity::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `operation` of `Cancelled``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Cancelled::operation","signature":"pub fava_runtime::ProviderCompletion::Cancelled::operation: fava_runtime::OperationName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Cancelled::operation: fava_runtime::OperationName"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Completed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::ProviderCompletion::Completed","signature":"pub fava_runtime::ProviderCompletion::Completed","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Completed"} --> | Compiler-visible enum variant owned by `fava_runtime::ProviderCompletion`. |
| **`Field `generation` of `Completed``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Completed::generation","signature":"pub fava_runtime::ProviderCompletion::Completed::generation: fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Completed::generation: fava_query::identity::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `operation` of `Completed``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Completed::operation","signature":"pub fava_runtime::ProviderCompletion::Completed::operation: fava_runtime::OperationName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Completed::operation: fava_runtime::OperationName"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `value` of `Completed``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Completed::value","signature":"pub fava_runtime::ProviderCompletion::Completed::value: T","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Completed::value: T"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Panicked`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::ProviderCompletion::Panicked","signature":"pub fava_runtime::ProviderCompletion::Panicked","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Panicked"} --> | Compiler-visible enum variant owned by `fava_runtime::ProviderCompletion`. |
| **`Field `detail` of `Panicked``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Panicked::detail","signature":"pub fava_runtime::ProviderCompletion::Panicked::detail: alloc::string::String","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Panicked::detail: alloc::string::String"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `generation` of `Panicked``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Panicked::generation","signature":"pub fava_runtime::ProviderCompletion::Panicked::generation: fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Panicked::generation: fava_query::identity::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `operation` of `Panicked``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Panicked::operation","signature":"pub fava_runtime::ProviderCompletion::Panicked::operation: fava_runtime::OperationName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Panicked::operation: fava_runtime::OperationName"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Refused`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::ProviderCompletion::Refused","signature":"pub fava_runtime::ProviderCompletion::Refused","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Refused"} --> | Compiler-visible enum variant owned by `fava_runtime::ProviderCompletion`. |
| **`Field `generation` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Refused::generation","signature":"pub fava_runtime::ProviderCompletion::Refused::generation: fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Refused::generation: fava_query::identity::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `operation` of `Refused``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::Refused::operation","signature":"pub fava_runtime::ProviderCompletion::Refused::operation: fava_runtime::OperationName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::Refused::operation: fava_runtime::OperationName"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`TimedOut`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::ProviderCompletion::TimedOut","signature":"pub fava_runtime::ProviderCompletion::TimedOut","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::TimedOut"} --> | Compiler-visible enum variant owned by `fava_runtime::ProviderCompletion`. |
| **`Field `after` of `TimedOut``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::TimedOut::after","signature":"pub fava_runtime::ProviderCompletion::TimedOut::after: core::time::Duration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::TimedOut::after: core::time::Duration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `generation` of `TimedOut``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::TimedOut::generation","signature":"pub fava_runtime::ProviderCompletion::TimedOut::generation: fava_query::identity::OperationGeneration","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::TimedOut::generation: fava_query::identity::OperationGeneration"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |
| **`Field `operation` of `TimedOut``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::ProviderCompletion::TimedOut::operation","signature":"pub fava_runtime::ProviderCompletion::TimedOut::operation: fava_runtime::OperationName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::ProviderCompletion::TimedOut::operation: fava_runtime::OperationName"} --> | Compiler-visible public field owned by `fava_runtime::ProviderCompletion`. |

### `Receiver` (Struct)

Compiler-visible struct `fava_runtime::Receiver`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::Receiver","signature":"pub struct fava_runtime::Receiver<T>","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::Receiver<T>"} -->

### `Runtime` (Struct)

Compiler-visible struct `fava_runtime::Runtime`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::Runtime","signature":"pub struct fava_runtime::Runtime","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::Runtime"} -->

| Item | Purpose |
| --- | --- |
| **`call_provider`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::call_provider","signature":"pub async fn fava_runtime::Runtime::call_provider<T, F>(&self, fava_runtime::OperationName, fava_query::identity::OperationGeneration, core::time::Duration, F) -> fava_runtime::ProviderCompletion<T> where F: core::future::future::Future<Output = T> + core::marker::Send + 'static, T: core::marker::Send + 'static","evidence":"cargo-public-api@0.52.0: pub async fn fava_runtime::Runtime::call_provider<T, F>(&self, fava_runtime::OperationName, fava_query::identity::OperationGeneration, core::time::Duration, F) -> fava_runtime::ProviderCompletion<T> where F: core::future::future::Future<Output = T> + core::marker::Send + 'static, T: core::marker::Send + 'static"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`cancellation_token`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::cancellation_token","signature":"pub fn fava_runtime::Runtime::cancellation_token(&self) -> fava_runtime::CancellationToken","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::cancellation_token(&self) -> fava_runtime::CancellationToken"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`channel`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::channel","signature":"pub fn fava_runtime::Runtime::channel<T: core::marker::Send + 'static>(&self, core::num::nonzero::NonZeroUsize) -> (fava_runtime::Sender<T>, fava_runtime::Receiver<T>)","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::channel<T: core::marker::Send + 'static>(&self, core::num::nonzero::NonZeroUsize) -> (fava_runtime::Sender<T>, fava_runtime::Receiver<T>)"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`config`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::config","signature":"pub fn fava_runtime::Runtime::config(&self) -> fava_runtime::RuntimeConfig","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::config(&self) -> fava_runtime::RuntimeConfig"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`default_channel`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::default_channel","signature":"pub fn fava_runtime::Runtime::default_channel<T: core::marker::Send + 'static>(&self) -> (fava_runtime::Sender<T>, fava_runtime::Receiver<T>)","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::default_channel<T: core::marker::Send + 'static>(&self) -> (fava_runtime::Sender<T>, fava_runtime::Receiver<T>)"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`new`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::new","signature":"pub fn fava_runtime::Runtime::new(fava_runtime::RuntimeConfig) -> Self","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::new(fava_runtime::RuntimeConfig) -> Self"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`outstanding_tasks`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::outstanding_tasks","signature":"pub fn fava_runtime::Runtime::outstanding_tasks(&self) -> alloc::vec::Vec<fava_runtime::TaskName>","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::outstanding_tasks(&self) -> alloc::vec::Vec<fava_runtime::TaskName>"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`running_provider_operations`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::running_provider_operations","signature":"pub fn fava_runtime::Runtime::running_provider_operations(&self) -> usize","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::running_provider_operations(&self) -> usize"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`shutdown`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::shutdown","signature":"pub async fn fava_runtime::Runtime::shutdown(&self, core::time::Duration) -> core::result::Result<(), fava_runtime::RuntimeError>","evidence":"cargo-public-api@0.52.0: pub async fn fava_runtime::Runtime::shutdown(&self, core::time::Duration) -> core::result::Result<(), fava_runtime::RuntimeError>"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`sleep`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::sleep","signature":"pub async fn fava_runtime::Runtime::sleep(&self, core::time::Duration)","evidence":"cargo-public-api@0.52.0: pub async fn fava_runtime::Runtime::sleep(&self, core::time::Duration)"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`spawn`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::spawn","signature":"pub fn fava_runtime::Runtime::spawn<F>(&self, fava_runtime::TaskName, F) -> core::result::Result<fava_runtime::TaskHandle<<F as core::future::future::Future>::Output>, fava_runtime::RuntimeError> where F: core::future::future::Future + core::marker::Send + 'static, <F as core::future::future::Future>::Output: core::marker::Send + 'static","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::spawn<F>(&self, fava_runtime::TaskName, F) -> core::result::Result<fava_runtime::TaskHandle<<F as core::future::future::Future>::Output>, fava_runtime::RuntimeError> where F: core::future::future::Future + core::marker::Send + 'static, <F as core::future::future::Future>::Output: core::marker::Send + 'static"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |
| **`spawn_cancellable`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"fava_runtime::Runtime::spawn_cancellable","signature":"pub fn fava_runtime::Runtime::spawn_cancellable<F>(&self, fava_runtime::TaskName, fava_runtime::CancellationToken, F) -> core::result::Result<fava_runtime::TaskHandle<core::option::Option<<F as core::future::future::Future>::Output>>, fava_runtime::RuntimeError> where F: core::future::future::Future + core::marker::Send + 'static, <F as core::future::future::Future>::Output: core::marker::Send + 'static","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::Runtime::spawn_cancellable<F>(&self, fava_runtime::TaskName, fava_runtime::CancellationToken, F) -> core::result::Result<fava_runtime::TaskHandle<core::option::Option<<F as core::future::future::Future>::Output>>, fava_runtime::RuntimeError> where F: core::future::future::Future + core::marker::Send + 'static, <F as core::future::future::Future>::Output: core::marker::Send + 'static"} --> | Compiler-visible method owned by `fava_runtime::Runtime`. |

### `RuntimeConfig` (Struct)

Compiler-visible struct `fava_runtime::RuntimeConfig`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::RuntimeConfig","signature":"pub struct fava_runtime::RuntimeConfig","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::RuntimeConfig"} -->

| Item | Purpose |
| --- | --- |
| **`default_channel_depth`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeConfig::default_channel_depth","signature":"pub fava_runtime::RuntimeConfig::default_channel_depth: core::num::nonzero::NonZeroUsize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeConfig::default_channel_depth: core::num::nonzero::NonZeroUsize"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeConfig`. |
| **`max_provider_operations`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeConfig::max_provider_operations","signature":"pub fava_runtime::RuntimeConfig::max_provider_operations: core::num::nonzero::NonZeroUsize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeConfig::max_provider_operations: core::num::nonzero::NonZeroUsize"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeConfig`. |
| **`max_tasks`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeConfig::max_tasks","signature":"pub fava_runtime::RuntimeConfig::max_tasks: core::num::nonzero::NonZeroUsize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeConfig::max_tasks: core::num::nonzero::NonZeroUsize"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeConfig`. |

### `RuntimeError` (Enum)

Compiler-visible enum `fava_runtime::RuntimeError`.
<!-- api-item {"kind":"Enum","item":"fava_runtime::RuntimeError","signature":"pub enum fava_runtime::RuntimeError","evidence":"cargo-public-api@0.52.0: pub enum fava_runtime::RuntimeError"} -->

| Item | Purpose |
| --- | --- |
| **`ProviderOperationLimit`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::RuntimeError::ProviderOperationLimit","signature":"pub fava_runtime::RuntimeError::ProviderOperationLimit","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::ProviderOperationLimit"} --> | Compiler-visible enum variant owned by `fava_runtime::RuntimeError`. |
| **`Field `limit` of `ProviderOperationLimit``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeError::ProviderOperationLimit::limit","signature":"pub fava_runtime::RuntimeError::ProviderOperationLimit::limit: usize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::ProviderOperationLimit::limit: usize"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeError`. |
| **`ShutdownIncomplete`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::RuntimeError::ShutdownIncomplete","signature":"pub fava_runtime::RuntimeError::ShutdownIncomplete","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::ShutdownIncomplete"} --> | Compiler-visible enum variant owned by `fava_runtime::RuntimeError`. |
| **`Field `tasks` of `ShutdownIncomplete``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeError::ShutdownIncomplete::tasks","signature":"pub fava_runtime::RuntimeError::ShutdownIncomplete::tasks: alloc::vec::Vec<fava_runtime::TaskName>","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::ShutdownIncomplete::tasks: alloc::vec::Vec<fava_runtime::TaskName>"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeError`. |
| **`ShuttingDown`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::RuntimeError::ShuttingDown","signature":"pub fava_runtime::RuntimeError::ShuttingDown","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::ShuttingDown"} --> | Compiler-visible enum variant owned by `fava_runtime::RuntimeError`. |
| **`TaskLimit`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::RuntimeError::TaskLimit","signature":"pub fava_runtime::RuntimeError::TaskLimit","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::TaskLimit"} --> | Compiler-visible enum variant owned by `fava_runtime::RuntimeError`. |
| **`Field `limit` of `TaskLimit``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::RuntimeError::TaskLimit::limit","signature":"pub fava_runtime::RuntimeError::TaskLimit::limit: usize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::RuntimeError::TaskLimit::limit: usize"} --> | Compiler-visible public field owned by `fava_runtime::RuntimeError`. |

### `SendRefusal` (Enum)

Compiler-visible enum `fava_runtime::SendRefusal`.
<!-- api-item {"kind":"Enum","item":"fava_runtime::SendRefusal","signature":"pub enum fava_runtime::SendRefusal","evidence":"cargo-public-api@0.52.0: pub enum fava_runtime::SendRefusal"} -->

| Item | Purpose |
| --- | --- |
| **`Closed`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::SendRefusal::Closed","signature":"pub fava_runtime::SendRefusal::Closed","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefusal::Closed"} --> | Compiler-visible enum variant owned by `fava_runtime::SendRefusal`. |
| **`DeadlineExpired`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::SendRefusal::DeadlineExpired","signature":"pub fava_runtime::SendRefusal::DeadlineExpired","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefusal::DeadlineExpired"} --> | Compiler-visible enum variant owned by `fava_runtime::SendRefusal`. |
| **`Full`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::SendRefusal::Full","signature":"pub fava_runtime::SendRefusal::Full","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefusal::Full"} --> | Compiler-visible enum variant owned by `fava_runtime::SendRefusal`. |
| **`Field `depth` of `Full``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::SendRefusal::Full::depth","signature":"pub fava_runtime::SendRefusal::Full::depth: usize","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefusal::Full::depth: usize"} --> | Compiler-visible public field owned by `fava_runtime::SendRefusal`. |

### `SendRefused` (Struct)

Compiler-visible struct `fava_runtime::SendRefused`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::SendRefused","signature":"pub struct fava_runtime::SendRefused<T>","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::SendRefused<T>"} -->

| Item | Purpose |
| --- | --- |
| **`reason`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::SendRefused::reason","signature":"pub fava_runtime::SendRefused::reason: fava_runtime::SendRefusal","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefused::reason: fava_runtime::SendRefusal"} --> | Compiler-visible public field owned by `fava_runtime::SendRefused`. |
| **`value`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::SendRefused::value","signature":"pub fava_runtime::SendRefused::value: T","evidence":"cargo-public-api@0.52.0: pub fava_runtime::SendRefused::value: T"} --> | Compiler-visible public field owned by `fava_runtime::SendRefused`. |

### `Sender` (Struct)

Compiler-visible struct `fava_runtime::Sender`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::Sender","signature":"pub struct fava_runtime::Sender<T>","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::Sender<T>"} -->

### `TaskFailure` (Enum)

Compiler-visible enum `fava_runtime::TaskFailure`.
<!-- api-item {"kind":"Enum","item":"fava_runtime::TaskFailure","signature":"pub enum fava_runtime::TaskFailure","evidence":"cargo-public-api@0.52.0: pub enum fava_runtime::TaskFailure"} -->

| Item | Purpose |
| --- | --- |
| **`Aborted`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::TaskFailure::Aborted","signature":"pub fava_runtime::TaskFailure::Aborted","evidence":"cargo-public-api@0.52.0: pub fava_runtime::TaskFailure::Aborted"} --> | Compiler-visible enum variant owned by `fava_runtime::TaskFailure`. |
| **`Field `name` of `Aborted``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::TaskFailure::Aborted::name","signature":"pub fava_runtime::TaskFailure::Aborted::name: fava_runtime::TaskName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::TaskFailure::Aborted::name: fava_runtime::TaskName"} --> | Compiler-visible public field owned by `fava_runtime::TaskFailure`. |
| **`Panicked`**<br><sub>Enum variant</sub><!-- api-item {"kind":"Enum variant","item":"fava_runtime::TaskFailure::Panicked","signature":"pub fava_runtime::TaskFailure::Panicked","evidence":"cargo-public-api@0.52.0: pub fava_runtime::TaskFailure::Panicked"} --> | Compiler-visible enum variant owned by `fava_runtime::TaskFailure`. |
| **`Field `detail` of `Panicked``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::TaskFailure::Panicked::detail","signature":"pub fava_runtime::TaskFailure::Panicked::detail: alloc::string::String","evidence":"cargo-public-api@0.52.0: pub fava_runtime::TaskFailure::Panicked::detail: alloc::string::String"} --> | Compiler-visible public field owned by `fava_runtime::TaskFailure`. |
| **`Field `name` of `Panicked``**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::TaskFailure::Panicked::name","signature":"pub fava_runtime::TaskFailure::Panicked::name: fava_runtime::TaskName","evidence":"cargo-public-api@0.52.0: pub fava_runtime::TaskFailure::Panicked::name: fava_runtime::TaskName"} --> | Compiler-visible public field owned by `fava_runtime::TaskFailure`. |

### `TaskHandle` (Struct)

Compiler-visible struct `fava_runtime::TaskHandle`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::TaskHandle","signature":"pub struct fava_runtime::TaskHandle<T>","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::TaskHandle<T>"} -->

### `TaskName` (Struct)

Compiler-visible struct `fava_runtime::TaskName`.
<!-- api-item {"kind":"Struct","item":"fava_runtime::TaskName","signature":"pub struct fava_runtime::TaskName(pub &'static str)","evidence":"cargo-public-api@0.52.0: pub struct fava_runtime::TaskName(pub &'static str)"} -->

| Item | Purpose |
| --- | --- |
| **`0`**<br><sub>Public field</sub><!-- api-item {"kind":"Public field","item":"fava_runtime::TaskName::0","signature":"pub &'static str","evidence":"cargo-public-api@0.52.0: pub &'static str"} --> | Compiler-visible public field owned by `fava_runtime::TaskName`. |
| **`core::fmt::Display::fmt`**<br><sub>Method</sub><!-- api-item {"kind":"Method","item":"<fava_runtime::TaskName as core::fmt::Display>::fmt","signature":"pub fn fava_runtime::TaskName::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result","evidence":"cargo-public-api@0.52.0: pub fn fava_runtime::TaskName::fmt(&self, &mut core::fmt::Formatter<'_>) -> core::fmt::Result"} --> | Compiler-visible method owned by `fava_runtime::TaskName`. |
<!-- END crate-readme-api inventory -->
