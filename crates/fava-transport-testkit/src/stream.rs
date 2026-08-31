//! The live connection identity of one fake session.

use std::sync::atomic::{AtomicU64, Ordering};

use fava_relay::RelaySessionKey;
use fava_transport::{RelayConnection, RelaySessionIdentity};

/// Live generation of one fake session, readable without the session lock.
pub(crate) struct LiveIdentity {
    pub(crate) key: RelaySessionKey,
    pub(crate) generation: AtomicU64,
}

impl LiveIdentity {
    pub(crate) fn read(&self) -> RelaySessionIdentity {
        RelaySessionIdentity {
            key: self.key.clone(),
            connection: RelayConnection::new(self.generation.load(Ordering::SeqCst))
                .expect("transport generations are non-zero"),
        }
    }
}
