//! Runtime signer attachment facade.

use std::sync::Arc;

use fava_session::SessionError;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, PublicKey, UnsignedEvent};
use tokio::sync::watch;

use crate::Fava;

impl Fava {
    /// Retain one public-key-only account in this running session.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing account selection or
    /// signer attachments.
    pub fn add_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        self.session.add_account(public_key)
    }

    /// Snapshot every retained session account in protocol order.
    #[must_use]
    pub fn accounts(&self) -> Vec<PublicKey> {
        self.session.accounts()
    }

    /// Select one retained account as current for this running session.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current selection.
    pub fn select_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        self.session.select_account(public_key)
    }

    /// Clear this session's current-account selection.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current selection.
    pub fn clear_current_account(&self) -> Result<(), SessionError> {
        self.session.clear_current_account()
    }

    /// Snapshot this session's selected account, if any.
    #[must_use]
    pub fn current_account(&self) -> Option<PublicKey> {
        self.session.current_account()
    }

    /// Atomically snapshot this session's current account and revision.
    #[must_use]
    pub fn current_account_snapshot(&self) -> (Option<PublicKey>, u64) {
        self.session.current_account_snapshot()
    }

    /// Remove one session account and atomically detach its signer.
    ///
    /// # Errors
    ///
    /// Returns a typed session refusal without changing current selection or
    /// signer attachments.
    pub fn remove_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        self.session.remove_account(public_key)
    }

    /// Return the latest committed session revision.
    #[must_use]
    pub fn session_revision(&self) -> u64 {
        self.session.revision()
    }

    /// Sign one exact unsigned event with its currently attached author signer.
    ///
    /// This is for Nostr protocols that require a signed artifact without a
    /// publication obligation. Publication continues to own durable signing
    /// and relay delivery for every event that is to be sent to relays.
    ///
    /// # Errors
    ///
    /// Returns [`SignerError::Unavailable`] when no current signer can accept
    /// work for the event author, or the provider's exact refusal otherwise.
    pub async fn sign(&self, event: UnsignedEvent) -> Result<Event, SignerError> {
        let author = event.pubkey;
        let (generation, availability) = self
            .session
            .signer(author)
            .ok_or_else(|| SignerError::Unavailable(format!("no signer attached for {author}")))?;
        if !matches!(availability, SignerAvailability::Available) {
            return Err(SignerError::Unavailable(format!(
                "signer attached for {author} is unavailable"
            )));
        }
        let (_cancel_tx, cancel) = watch::channel(false);
        let signing = self
            .session
            .invoke_signer(author, generation, event, cancel)
            .ok_or_else(|| {
                SignerError::Unavailable(format!("signer attachment changed for {author}"))
            })?;
        signing.await
    }

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
