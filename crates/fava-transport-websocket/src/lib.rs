//! WebSocket implementation of the Fava transport contract.
//!
//! One socket per [`RelaySessionKey`], shared by every lease holder. The
//! registry, the refcount, the four deadlines, the two bounded queues, and
//! reconnect pacing all live here, because that is what `ARCH:1588-1594` and
//! `GOALS:936` assign to the transport implementer.

mod backoff;
mod driver;
mod fanout;
mod session;

use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_state::RelaySessionKey;
use fava_transport::{
    LeaseRelease, OpenRelaySession, RelaySession, RelaySessionFuture, RelaySessionIdentity,
    RelaySessionLease, ReleaseFuture, ReleaseOutcome, Transport, TransportError,
    TransportShutdownFuture,
};
use tokio::sync::mpsc;

use crate::driver::establish;
use crate::session::{SessionShared, WebSocketRelaySession};

/// WebSocket relay transport with one shared session per relay-access identity.
#[derive(Default)]
pub struct WebSocketTransport {
    registry: Arc<Registry>,
}

#[derive(Default)]
struct Registry {
    entries: Mutex<BTreeMap<RelaySessionKey, Entry>>,
    shutting_down: AtomicBool,
    entropy: AtomicU64,
}

struct Entry {
    session: Arc<WebSocketRelaySession>,
    holders: usize,
}

impl WebSocketTransport {
    /// Construct a transport with an empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reuse the live session for `key`, if one is registered.
    ///
    /// # Panics
    ///
    /// If a thread panicked while holding the session registry.
    fn reuse(&self, key: &RelaySessionKey) -> Option<RelaySessionLease> {
        let mut entries = self.registry.entries.lock().expect("registry is not poisoned");
        let entry = entries.get_mut(key)?;
        entry.holders += 1;
        let session = Arc::clone(&entry.session);
        let identity = session.identity();
        Some(RelaySessionLease::new(
            session,
            Arc::clone(&self.registry) as Arc<dyn LeaseRelease>,
            identity,
        ))
    }
}

impl Transport for WebSocketTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        Box::pin(async move {
            if self.registry.shutting_down.load(Ordering::SeqCst) {
                return Err(TransportError::ShuttingDown);
            }
            if let Some(lease) = self.reuse(&request.key) {
                return Ok(lease);
            }

            let entropy = self.registry.entropy.fetch_add(0x9E37_79B9, Ordering::SeqCst);
            let shared = Arc::new(SessionShared::new(&request, entropy));
            let socket = establish(&shared)
                .await
                .map_err(TransportError::ConnectionRefused)?;
            let (outbound, inbound) = mpsc::channel(request.bounds.outbound_frames.get());
            tokio::spawn(driver::drive(Arc::clone(&shared), inbound, socket));

            let session = Arc::new(WebSocketRelaySession { shared, outbound });
            let identity = session.identity();
            // Another acquire may have dialled the same key while this one was
            // establishing. The first registration wins; this socket is closed
            // rather than leaked, so the key keeps exactly one live session.
            let mut entries = self
                .registry
                .entries
                .lock()
                .expect("registry is not poisoned");
            if let Some(existing) = entries.get_mut(&request.key) {
                existing.holders += 1;
                let winner = Arc::clone(&existing.session);
                drop(entries);
                let loser = session;
                tokio::spawn(async move { loser.close().await });
                let identity = winner.identity();
                return Ok(RelaySessionLease::new(
                    winner,
                    Arc::clone(&self.registry) as Arc<dyn LeaseRelease>,
                    identity,
                ));
            }
            entries.insert(
                request.key.clone(),
                Entry {
                    session: Arc::clone(&session),
                    holders: 1,
                },
            );
            drop(entries);
            Ok(RelaySessionLease::new(
                session,
                Arc::clone(&self.registry) as Arc<dyn LeaseRelease>,
                identity,
            ))
        })
    }

    fn holders(&self, key: &RelaySessionKey) -> Option<NonZeroUsize> {
        let entries = self
            .registry
            .entries
            .lock()
            .expect("registry is not poisoned");
        entries
            .get(key)
            .and_then(|entry| NonZeroUsize::new(entry.holders))
    }

    fn shutdown(&self, deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async move {
            self.registry.shutting_down.store(true, Ordering::SeqCst);
            let closing: Vec<_> = {
                let mut entries = self
                    .registry
                    .entries
                    .lock()
                    .expect("registry is not poisoned");
                std::mem::take(&mut *entries)
                    .into_values()
                    .map(|entry| entry.session)
                    .collect()
            };
            let remaining = closing.len();
            let joined = tokio::time::timeout(deadline, async {
                for session in closing {
                    let _ = session.close().await;
                }
            })
            .await;
            if joined.is_err() {
                return Err(TransportError::ShutdownIncomplete { remaining });
            }
            Ok(())
        })
    }
}

impl Registry {
    fn decrement(
        &self,
        key: &RelaySessionKey,
    ) -> Option<(Arc<WebSocketRelaySession>, ReleaseOutcome)> {
        let mut entries = self.entries.lock().expect("registry is not poisoned");
        let entry = entries.get_mut(key)?;
        entry.holders = entry.holders.saturating_sub(1);
        if let Some(holders) = NonZeroUsize::new(entry.holders) {
            return Some((
                Arc::clone(&entry.session),
                ReleaseOutcome::Retained { holders },
            ));
        }
        let entry = entries.remove(key)?;
        Some((entry.session, ReleaseOutcome::Closed))
    }
}

impl LeaseRelease for Registry {
    fn release_now(&self, identity: &RelaySessionIdentity) {
        if let Some((session, ReleaseOutcome::Closed)) = self.decrement(&identity.key) {
            tokio::spawn(async move { session.close().await });
        }
    }

    fn release_deterministically<'a>(
        &'a self,
        identity: &'a RelaySessionIdentity,
    ) -> ReleaseFuture<'a> {
        Box::pin(async move {
            let Some((session, outcome)) = self.decrement(&identity.key) else {
                return Err(TransportError::Closed(identity.clone()));
            };
            if outcome == ReleaseOutcome::Closed {
                session.close().await?;
            }
            Ok(outcome)
        })
    }
}
