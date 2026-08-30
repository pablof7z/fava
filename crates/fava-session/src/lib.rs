//! Bounded runtime accounts, current selection, and exact-key signer attachment.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_write::{Event, PublicKey, UnsignedEvent};
use thiserror::Error;
use tokio::sync::watch;

/// Runtime authority for accounts, current selection, and exact-key signer attachment.
#[derive(Clone)]
pub struct Session {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    revision: watch::Sender<u64>,
}

/// The bounded account set, its optional current selection, exact signer
/// attachments, and the revision that advances with every committed mutation.
struct State {
    accounts: BTreeSet<PublicKey>,
    signers: BTreeMap<PublicKey, Attachment>,
    current_account: Option<PublicKey>,
    revision: u64,
}

struct Attachment {
    generation: u64,
    signer: Arc<dyn Signer>,
}

/// Typed refusal from the runtime signer owner.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SessionError {
    /// An account is already retained for this exact public key.
    #[error("account already retained for {0}")]
    DuplicateAccount(PublicKey),
    /// No account is retained for this exact public key.
    #[error("no account retained for {0}")]
    MissingAccount(PublicKey),
    /// Adding a distinct account would exceed the session bound.
    #[error("session account capacity {limit} exceeded")]
    AccountCapacityExceeded {
        /// Maximum retained account count.
        limit: usize,
    },
    /// A signer is already attached for this exact public key.
    #[error("signer already attached for {0}")]
    DuplicateSigner(PublicKey),
    /// No signer is attached for this exact public key.
    #[error("no signer attached for {0}")]
    MissingSigner(PublicKey),
    /// A fresh session revision and attachment generation could not be represented.
    #[error("session revision exhausted")]
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
        let mut accounts = BTreeSet::new();
        let mut indexed = BTreeMap::new();
        let mut revision = 0_u64;
        for signer in signers {
            let public_key = signer.public_key();
            if indexed.contains_key(&public_key) {
                return Err(SessionError::DuplicateSigner(public_key));
            }
            if accounts.len() == ACCOUNT_CAPACITY {
                return Err(SessionError::AccountCapacityExceeded {
                    limit: ACCOUNT_CAPACITY,
                });
            }
            revision = next_revision(revision)?;
            accounts.insert(public_key);
            indexed.insert(
                public_key,
                Attachment {
                    generation: revision,
                    signer,
                },
            );
        }
        let (revision_signal, _) = watch::channel(revision);
        Ok(Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    accounts,
                    signers: indexed,
                    current_account: None,
                    revision,
                }),
                revision: revision_signal,
            }),
        })
    }

    /// Retain one public-key-only account.
    ///
    /// # Errors
    ///
    /// Refuses a duplicate account, capacity overflow, or revision exhaustion
    /// without changing current state.
    pub fn add_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        let mut state = self.lock_state();
        if state.accounts.contains(&public_key) {
            return Err(SessionError::DuplicateAccount(public_key));
        }
        if state.accounts.len() == ACCOUNT_CAPACITY {
            return Err(SessionError::AccountCapacityExceeded {
                limit: ACCOUNT_CAPACITY,
            });
        }
        let revision = next_revision(state.revision)?;
        state.accounts.insert(public_key);
        commit_revision(&mut state, &self.inner.revision, revision);
        Ok(())
    }

    /// Snapshot retained account public keys in protocol order.
    #[must_use]
    pub fn accounts(&self) -> Vec<PublicKey> {
        self.lock_state().accounts.iter().copied().collect()
    }

    /// Select one retained account as current.
    ///
    /// # Errors
    ///
    /// Refuses a missing account or revision exhaustion without changing
    /// current state. Selecting the already-current account is idempotent.
    pub fn select_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        let mut state = self.lock_state();
        if !state.accounts.contains(&public_key) {
            return Err(SessionError::MissingAccount(public_key));
        }
        if state.current_account == Some(public_key) {
            return Ok(());
        }
        let revision = next_revision(state.revision)?;
        state.current_account = Some(public_key);
        commit_revision(&mut state, &self.inner.revision, revision);
        Ok(())
    }

    /// Clear the current-account selection.
    ///
    /// # Errors
    ///
    /// Refuses revision exhaustion without changing current state. Clearing an
    /// already-empty selection is idempotent.
    pub fn clear_current_account(&self) -> Result<(), SessionError> {
        let mut state = self.lock_state();
        if state.current_account.is_none() {
            return Ok(());
        }
        let revision = next_revision(state.revision)?;
        state.current_account = None;
        commit_revision(&mut state, &self.inner.revision, revision);
        Ok(())
    }

    /// Snapshot the selected account, if any.
    #[must_use]
    pub fn current_account(&self) -> Option<PublicKey> {
        self.lock_state().current_account
    }

    /// Atomically snapshot the current account and its session revision.
    #[must_use]
    pub fn current_account_snapshot(&self) -> (Option<PublicKey>, u64) {
        let state = self.lock_state();
        (state.current_account, state.revision)
    }

    /// Remove one account, its attached signer, and its current selection.
    ///
    /// # Errors
    ///
    /// Refuses a missing account or revision exhaustion without changing
    /// current state.
    pub fn remove_account(&self, public_key: PublicKey) -> Result<(), SessionError> {
        let mut state = self.lock_state();
        if !state.accounts.contains(&public_key) {
            return Err(SessionError::MissingAccount(public_key));
        }
        let revision = next_revision(state.revision)?;
        state.accounts.remove(&public_key);
        state.signers.remove(&public_key);
        if state.current_account == Some(public_key) {
            state.current_account = None;
        }
        commit_revision(&mut state, &self.inner.revision, revision);
        Ok(())
    }

    /// Return the latest committed session revision.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.lock_state().revision
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
        if !state.accounts.contains(&public_key) && state.accounts.len() == ACCOUNT_CAPACITY {
            return Err(SessionError::AccountCapacityExceeded {
                limit: ACCOUNT_CAPACITY,
            });
        }
        let revision = next_revision(state.revision)?;
        state.accounts.insert(public_key);
        state.signers.insert(
            public_key,
            Attachment {
                generation: revision,
                signer,
            },
        );
        commit_revision(&mut state, &self.inner.revision, revision);
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
        let revision = next_revision(state.revision)?;
        state.signers.insert(
            public_key,
            Attachment {
                generation: revision,
                signer,
            },
        );
        commit_revision(&mut state, &self.inner.revision, revision);
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
        let revision = next_revision(state.revision)?;
        state.signers.remove(&public_key);
        commit_revision(&mut state, &self.inner.revision, revision);
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

    /// Subscribe to coalescible account, selection, and signer change signals.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.revision.subscribe()
    }

    /// Take the attachment lock, recovering a poisoned mutex instead of panicking.
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

const ACCOUNT_CAPACITY: usize = 64;

fn commit_revision(state: &mut State, signal: &watch::Sender<u64>, revision: u64) {
    state.revision = revision;
    signal.send_replace(revision);
}

fn next_revision(current: u64) -> Result<u64, SessionError> {
    current
        .checked_add(1)
        .ok_or(SessionError::GenerationExhausted)
}
