//! Protected secret input that never reaches command parsing or history.

use std::io::{IsTerminal as _, stdin};

use zeroize::Zeroizing;

use crate::ShellError;

/// Opaque in-memory secret returned only by a protected interactive prompt.
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Prompt without echo when standard input is a real interactive terminal.
    ///
    /// # Errors
    ///
    /// Refuses script/non-terminal use rather than accepting secret material in
    /// a command file, environment variable, or history-bearing input stream.
    pub fn prompt(label: &str) -> Result<Self, ShellError> {
        if !stdin().is_terminal() {
            return Err(ShellError::NonInteractiveSecretPrompt);
        }
        rpassword::prompt_password(label)
            .map(|value| Self(Zeroizing::new(value)))
            .map_err(|error| ShellError::Output(error.to_string()))
    }

    /// Temporarily expose the secret to a single domain-owned conversion.
    ///
    /// The value is never serializable, printable, capturable, or retained by
    /// this package; callers must not retain a borrowed value beyond `use_it`.
    pub fn with_exposed<T>(&self, use_it: impl FnOnce(&str) -> T) -> T {
        use_it(&self.0)
    }
}
