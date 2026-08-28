//! Private, bounded command-shell support for runnable Fava E2E examples.
//!
//! Domain examples own their command grammars and Fava workflows. This package
//! owns only common terminal/script ingress and its retained application state.

mod account;
mod error;
mod limits;
mod result;
mod secret;
mod session;

pub use account::{Account, parse_public_key};
pub use error::ShellError;
pub use limits::Limits;
pub use result::{CommandResult, OutputFormat, ResultStatus};
pub use secret::Secret;
pub use session::{E2eSession, InputMode};
