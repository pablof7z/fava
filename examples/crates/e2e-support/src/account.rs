//! Explicit selected-account state for an example shell.

use fava::PublicKey;

use crate::ShellError;

/// One application-owned signer identity available to a command session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    alias: String,
    public_key: PublicKey,
}

impl Account {
    /// Construct one named account whose signer was registered by the domain app.
    #[must_use]
    pub fn new(alias: impl Into<String>, public_key: PublicKey) -> Self {
        Self {
            alias: alias.into(),
            public_key,
        }
    }

    /// Return this account's application-local alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Return the exact public key selected for Fava event construction.
    #[must_use]
    pub const fn public_key(&self) -> PublicKey {
        self.public_key
    }
}

/// Parse an exact hex public key through the public Fava facade type.
///
/// # Errors
///
/// Returns [`ShellError::InvalidPublicKey`] when `input` is not an exact Nostr
/// public key.
pub fn parse_public_key(input: &str) -> Result<PublicKey, ShellError> {
    input
        .parse::<PublicKey>()
        .map_err(|error| ShellError::InvalidPublicKey(error.to_string()))
}
