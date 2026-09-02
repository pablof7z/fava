//! The live connection identity of one fake session.

use std::sync::atomic::{AtomicU64, Ordering};

use fava_transport::{RelayConnection, RelaySessionIdentity};
use nostr::types::RelayUrl;

/// Live generation of one fake session, readable without the session lock.
pub(crate) struct LiveIdentity {
    pub(crate) relay: RelayUrl,
    pub(crate) generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            relay: self.relay.clone(),
            connection: RelayConnection::new(self.generation.load(Ordering::SeqCst))
                .expect("transport generations are non-zero"),
        }
    }
}
