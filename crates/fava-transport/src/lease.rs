//! Refcounted holds on one relay session.

use std::sync::Arc;

use crate::{RelaySession, RelaySessionIdentity, ReleaseFuture, ReleaseOutcome, TransportError};

/// Hook the transport registry installs so a lease can decrement its refcount
/// without the contract crate knowing the registry's shape.
pub trait LeaseRelease: Send + Sync {
    /// Decrement the holder count for `identity`. MUST be non-blocking and
    /// MUST NOT await. Closing, if this was the last holder, is scheduled by
    /// the transport, not performed here.
    fn release_now(&self, identity: &RelaySessionIdentity);

    /// Decrement and drive deterministic close when this was the last holder.
    fn release_deterministically<'a>(
        &'a self,
        identity: &'a RelaySessionIdentity,
    ) -> ReleaseFuture<'a>;
}

/// A refcounted hold on one relay session.
///
/// Authority: ARCH:1593 "current and retiring session lifecycle";
/// GOALS:936 shared connection ownership; ARCH:2072 "ownership/refcounts for
/// shared work" (held by `fava-observe`, expressed through this lease).
pub struct RelaySessionLease {
    session: Arc<dyn RelaySession>,
    registry: Arc<dyn LeaseRelease>,
    identity: RelaySessionIdentity,
    released: bool,
}

impl RelaySessionLease {
    /// Construct a lease. Called only by a `Transport` implementation.
    #[must_use]
    pub fn new(
        session: Arc<dyn RelaySession>,
        registry: Arc<dyn LeaseRelease>,
        identity: RelaySessionIdentity,
    ) -> Self {
        Self {
            session,
            registry,
            identity,
            released: false,
        }
    }

    /// The leased session.
    #[must_use]
    pub fn session(&self) -> &Arc<dyn RelaySession> {
        &self.session
    }

    /// Identity at the moment of acquisition. Use `session().identity()` for
    /// the current generation.
    #[must_use]
    pub fn acquired_identity(&self) -> &RelaySessionIdentity {
        &self.identity
    }

    /// Release deterministically, awaiting close when this is the last holder.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when the close handshake fails or times out.
    pub async fn release(mut self) -> Result<ReleaseOutcome, TransportError> {
        self.released = true;
        let registry = Arc::clone(&self.registry);
        let identity = self.identity.clone();
        registry.release_deterministically(&identity).await
    }
}

impl Drop for RelaySessionLease {
    fn drop(&mut self) {
        if !self.released {
            self.registry.release_now(&self.identity);
        }
    }
}
