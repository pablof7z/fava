//! Protected secret input that never reaches command parsing or history.

use std::io::{BufRead as _, IsTerminal as _, stdin};
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
        protected_terminal_input(label).map(Self)
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

#[cfg(unix)]
fn protected_terminal_input(label: &str) -> Result<Zeroizing<String>, ShellError> {
    use std::os::fd::AsFd as _;
    use std::{fs::File, io::Write as _};

    use nix::sys::termios::{LocalFlags, SetArg, tcgetattr, tcsetattr};
    use nix::unistd::dup;

    let input = stdin();
    let mut input = input.lock();
    let terminal = dup(input.as_fd()).map_err(|error| ShellError::Output(error.to_string()))?;
    let original = tcgetattr(&terminal).map_err(|error| ShellError::Output(error.to_string()))?;
    let mut hidden = original.clone();
    hidden.local_flags.remove(LocalFlags::ECHO);
    hidden.local_flags.insert(LocalFlags::ECHONL);
    tcsetattr(&terminal, SetArg::TCSANOW, &hidden)
        .map_err(|error| ShellError::Output(error.to_string()))?;

    let mut output =
        File::from(dup(&terminal).map_err(|error| ShellError::Output(error.to_string()))?);
    let prompt = output
        .write_all(label.as_bytes())
        .and_then(|()| output.flush());
    if let Err(error) = prompt {
        let _ = tcsetattr(&terminal, SetArg::TCSANOW, &original);
        return Err(ShellError::Output(error.to_string()));
    }

    let mut value = Zeroizing::new(String::new());
    let read = input.read_line(&mut value);
    let restore = tcsetattr(&terminal, SetArg::TCSANOW, &original);
    if let Err(error) = restore {
        return Err(ShellError::Output(error.to_string()));
    }
    read.map_err(|error| ShellError::Output(error.to_string()))?;
    while value.ends_with(['\n', '\r']) {
        value.pop();
    }
    Ok(value)
}

#[cfg(not(unix))]
fn protected_terminal_input(label: &str) -> Result<Zeroizing<String>, ShellError> {
    rpassword::prompt_password(label)
        .map(Zeroizing::new)
        .map_err(|error| ShellError::Output(error.to_string()))
}
