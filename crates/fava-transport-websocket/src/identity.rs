//! The live connection identity of one socket, readable without the driver's
//! lock.

use std::sync::atomic::{AtomicU64, Ordering};

use fava_relay::RelaySessionKey;
use fava_transport::{RelayConnection, RelaySessionIdentity};

/// Live generation of one session, readable without the driver's lock.
pub(crate) struct LiveIdentity {
    key: RelaySessionKey,
    generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn new(key: RelaySessionKey, generation: RelayConnection) -> Self {
        Self {
            key,
            generation: AtomicU64::new(generation.get()),
        }
    }

    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            key: self.key.clone(),
            connection: RelayConnection::new(self.generation.load(Ordering::SeqCst))
                .expect("transport generations are non-zero"),
        }
    }

    pub(crate) fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    /// Retire the current generation and return both identities.
    pub(crate) fn advance(
        &self,
        generation: RelayConnection,
    ) -> (RelaySessionIdentity, RelaySessionIdentity) {
        let previous = self.read();
        self.generation.store(generation.get(), Ordering::SeqCst);
        (previous, self.read())
    }
}
