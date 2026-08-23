//! Runtime signer attachment facade.

use std::sync::Arc;

use fava_session::SessionError;
use fava_signer::Signer;
use fava_write::PublicKey;

use crate::Fava;

impl Fava {
    /// Attach one runtime signer for its exact public key.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current attachments.
    pub fn add_signer(&self, signer: Arc<dyn Signer>) -> Result<(), SessionError> {
        self.session.add_signer(signer)
    }

    /// Explicitly replace the runtime signer for its exact public key.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current attachments.
    pub fn replace_signer(&self, signer: Arc<dyn Signer>) -> Result<(), SessionError> {
        self.session.replace_signer(signer)
    }

    /// Remove the runtime signer for one exact public key.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current attachments.
    pub fn remove_signer(&self, public_key: PublicKey) -> Result<(), SessionError> {
        self.session.remove_signer(public_key)
    }
}
