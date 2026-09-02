//! The live connection identity of one socket, readable without the driver's
//! lock.

use std::sync::atomic::{AtomicU64, Ordering};

use fava_transport::{RelayConnection, RelaySessionIdentity};
use nostr::types::RelayUrl;

/// Live generation of one session, readable without the driver's lock.
pub(crate) struct LiveIdentity {
    relay: RelayUrl,
    generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn new(relay: RelayUrl, generation: RelayConnection) -> Self {
        Self {
            relay,
            generation: AtomicU64::new(generation.get()),
        }
    }

    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            relay: self.relay.clone(),
            connection: RelayConnection::new(self.generation.load(Ordering::SeqCst))
                .expect("transport generations are non-zero"),
        }
    }

    pub(crate) fn relay(&self) -> &RelayUrl {
        &self.relay
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
