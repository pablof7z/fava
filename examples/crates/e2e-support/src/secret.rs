//! Protected secret input that never reaches command parsing or history.

use std::io::{IsTerminal as _, stdin};
use std::sync::Arc;

use fava::{Fava, PublicKey};
use fava_signer_local::LocalSigner;
use nostr::key::Keys;
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
    pub(crate) fn prompt(label: &str) -> Result<Self, ShellError> {
        if !stdin().is_terminal() {
            return Err(ShellError::NonInteractiveSecretPrompt);
        }
        rpassword::prompt_password(label)
            .map(|value| Self(Zeroizing::new(value)))
            .map_err(|error| ShellError::Output(error.to_string()))
    }

    /// Parse the protected input and attach its local signer through Fava.
    ///
    /// Consuming `self` prevents the protected text from escaping into caller
    /// state; the only returned fact is the public author key.
    pub(crate) fn attach_local_signer(self, fava: &Fava) -> Result<PublicKey, ShellError> {
        let keys = Keys::parse(&self.0).map_err(|_| ShellError::InvalidImportedAccount)?;
        let public_key = keys.public_key();
        fava.add_signer(Arc::new(LocalSigner::new(keys)))
            .map_err(|error| ShellError::AccountSigner(error.to_string()))?;
        Ok(public_key)
    }
}
