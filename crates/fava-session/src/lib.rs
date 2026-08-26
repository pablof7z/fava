//! Runtime signer attachment for exact account public keys.

use std::collections::BTreeMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, PublicKey, UnsignedEvent};
use thiserror::Error;
use tokio::sync::watch;

/// Runtime authority for signer attachment to exact account public keys.
#[derive(Clone)]
pub struct Session {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    revision: watch::Sender<u64>,
}

struct State {
    signers: BTreeMap<PublicKey, Attachment>,
    generation: u64,
}

struct Attachment {
    generation: u64,
    signer: Arc<dyn Signer>,
}

/// Typed refusal from the runtime signer owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionError {
    /// A signer is already attached for this exact public key.
    #[error("signer already attached for {0}")]
    DuplicateSigner(PublicKey),
    /// No signer is attached for this exact public key.
    #[error("no signer attached for {0}")]
    MissingSigner(PublicKey),
    /// A fresh attachment generation could not be represented.
    #[error("signer attachment generation exhausted")]
    GenerationExhausted,
}

impl Session {
    /// Construct one session from uniquely keyed signer attachments.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for duplicate keys or generation exhaustion
    /// without publishing a partially built session.
    pub fn new(signers: impl IntoIterator<Item = Arc<dyn Signer>>) -> Result<Self, SessionError> {
        let mut indexed = BTreeMap::new();
        let mut generation = 0_u64;
        for signer in signers {
            let public_key = signer.public_key();
            if indexed.contains_key(&public_key) {
                return Err(SessionError::DuplicateSigner(public_key));
            }
            generation = generation
                .checked_add(1)
                .ok_or(SessionError::GenerationExhausted)?;
            indexed.insert(public_key, Attachment { generation, signer });
        }
        let (revision, _) = watch::channel(generation);
        Ok(Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    signers: indexed,
                    generation,
                }),
                revision,
            }),
        })
    }

    /// Attach one signer for its exact public key.
    ///
    /// # Errors
    ///
    /// Refuses duplicate keys or generation exhaustion without changing current state.
    pub fn add_signer(&self, signer: Arc<dyn Signer>) -> Result<(), SessionError> {
        let public_key = signer.public_key();
        let mut state = self.lock_state();
        if state.signers.contains_key(&public_key) {
            return Err(SessionError::DuplicateSigner(public_key));
        }
        let generation = next_generation(state.generation)?;
        state
            .signers
            .insert(public_key, Attachment { generation, signer });
        state.generation = generation;
        self.inner.revision.send_replace(generation);
        Ok(())
    }

    /// Explicitly replace the signer attached to its exact public key.
    ///
    /// # Errors
    ///
    /// Refuses a missing attachment or generation exhaustion without mutation.
    pub fn replace_signer(&self, signer: Arc<dyn Signer>) -> Result<(), SessionError> {
        let public_key = signer.public_key();
        let mut state = self.lock_state();
        if !state.signers.contains_key(&public_key) {
            return Err(SessionError::MissingSigner(public_key));
        }
        let generation = next_generation(state.generation)?;
        state
            .signers
            .insert(public_key, Attachment { generation, signer });
        state.generation = generation;
        self.inner.revision.send_replace(generation);
        Ok(())
    }

    /// Remove the signer attached to one exact public key.
    ///
    /// # Errors
    ///
    /// Refuses a missing attachment or generation exhaustion without mutation.
    pub fn remove_signer(&self, public_key: PublicKey) -> Result<(), SessionError> {
        let mut state = self.lock_state();
        if !state.signers.contains_key(&public_key) {
            return Err(SessionError::MissingSigner(public_key));
        }
        let generation = next_generation(state.generation)?;
        state.signers.remove(&public_key);
        state.generation = generation;
        self.inner.revision.send_replace(generation);
        Ok(())
    }

    /// Snapshot the current exact attachment generation and availability.
    #[must_use]
    pub fn signer(&self, public_key: PublicKey) -> Option<(u64, SignerAvailability)> {
        let (generation, signer) = self
            .lock_state()
            .signers
            .get(&public_key)
            .map(|attachment| (attachment.generation, Arc::clone(&attachment.signer)))?;
        Some((generation, signer.availability()))
    }

    /// Invoke one exact attachment generation while replacement and removal are excluded.
    ///
    /// The session lock is held only through provider method invocation. The
    /// returned future owns its provider and is awaited after the lock is released.
    #[must_use]
    #[allow(clippy::type_complexity)] // Reuse the Signer future shape without a wrapper noun.
    pub fn invoke_signer(
        &self,
        public_key: PublicKey,
        generation: u64,
        event: UnsignedEvent,
        cancel: watch::Receiver<bool>,
    ) -> Option<Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>>> {
        let state = self.lock_state();
        let attachment = state
            .signers
            .get(&public_key)
            .filter(|attachment| attachment.generation == generation)?;
        Some(
            catch_unwind(AssertUnwindSafe(|| {
                Arc::clone(&attachment.signer).sign_event(event, cancel)
            }))
            .unwrap_or_else(|_| {
                Box::pin(std::future::ready(Err(SignerError::Unavailable(format!(
                    "signer attachment generation {generation} for {public_key} panicked during provider invocation"
                )))))
            }),
        )
    }

    /// Return whether one exact signer attachment generation is still current.
    #[must_use]
    pub fn is_current(&self, public_key: PublicKey, generation: u64) -> bool {
        self.lock_state()
            .signers
            .get(&public_key)
            .is_some_and(|attachment| attachment.generation == generation)
    }

    /// Subscribe to coalescible signer attachment change signals.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.revision.subscribe()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn next_generation(current: u64) -> Result<u64, SessionError> {
    current
        .checked_add(1)
        .ok_or(SessionError::GenerationExhausted)
}
