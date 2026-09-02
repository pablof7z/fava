//! WebSocket implementation of the Fava transport contract.
//!
//! One socket per relay and reachable authority, shared by every lease
//! holder. The registry, the refcount, the four deadlines, the two bounded
//! queues, and reconnect pacing all live here, because that is what
//! `ARCH:1588-1594` and `GOALS:936` assign to the transport implementer.

mod backoff;
mod driver;
mod identity;
mod session;

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use fava_relay::Authority;
use fava_transport::{
    LeaseRelease, OpenRelaySession, RelayConnection, RelaySession, RelaySessionFuture,
    RelaySessionIdentity, RelaySessionLease, ReleaseFuture, ReleaseOutcome, Transport,
    TransportError, TransportShutdownFuture,
};
use nostr::types::RelayUrl;
use tokio::sync::{broadcast, mpsc};

use crate::driver::establish;
use crate::session::{SessionShared, WebSocketRelaySession};

/// WebSocket relay transport with one shared session per relay and reachable
/// authority.
#[derive(Default)]
pub struct WebSocketTransport {
    registry: Arc<Registry>,
}

/// How many unheard authentication requests the stream holds before the
/// oldest is dropped. One per relay in flight at once is generous; a lagging
/// subscriber has stopped answering, which is its own problem.
const REQUEST_BACKLOG: usize = 64;

struct Registry {
    /// Every live connection to a relay, grouped by relay: more than one can
    /// exist when a request needs an authority none of the others can reach.
    entries: Mutex<BTreeMap<RelayUrl, Vec<Entry>>>,
    shutting_down: AtomicBool,
    entropy: AtomicU64,
    generations: Arc<AtomicU64>,
    /// Sessions whose relay has asked them to authenticate.
    requests: broadcast::Sender<Arc<dyn RelaySession>>,
    /// Transport-wide source of wire subscription identifiers. Never reset by a
    /// reconnect or a re-acquired session, so a reopened request can never wear
    /// a closed request's identity (GOALS:426, QUERY-010).
    subscriptions: Arc<AtomicU64>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: Mutex::default(),
            shutting_down: AtomicBool::default(),
            entropy: AtomicU64::default(),
            generations: Arc::default(),
            requests: broadcast::Sender::new(REQUEST_BACKLOG),
            subscriptions: Arc::default(),
        }
    }
}

/// One live relay session and how many leases are still outstanding on it.
struct Entry {
    session: Arc<WebSocketRelaySession>,
    holders: usize,
    /// Every authority an acquire has ever been satisfied under on this
    /// connection. Introspection looks a connection up by this: it names
    /// what was acquired, and keeps naming it even after the connection's
    /// live authentication moves on to a refusal that can no longer reach
    /// anything.
    served: BTreeSet<Authority>,
}

impl Entry {
    /// Whether this connection can still reach `authority`, right now.
    fn can_serve(&self, authority: &Authority) -> bool {
        self.session
            .router()
            .connection()
            .borrow()
            .can_serve(authority)
    }
}

impl WebSocketTransport {
    /// Construct a transport with an empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reuse a live session for `relay` that can still reach `authority`, if
    /// one is registered.
    ///
    /// # Panics
    ///
    /// If a thread panicked while holding the session registry.
    fn reuse(&self, relay: &RelayUrl, authority: &Authority) -> Option<RelaySessionLease> {
        let mut entries = self
            .registry
            .entries
            .lock()
            .expect("registry is not poisoned");
        let entry = entries
            .get_mut(relay)?
            .iter_mut()
            .find(|entry| entry.can_serve(authority))?;
        entry.holders += 1;
        entry.served.insert(*authority);
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
            if let Some(lease) = self.reuse(&request.relay, &request.authority) {
                return Ok(lease);
            }

            let generation = mint_generation(&self.registry.generations)
                .ok_or(TransportError::GenerationExhausted)?;

            let entropy = self
                .registry
                .entropy
                .fetch_add(0x9E37_79B9, Ordering::SeqCst);
            let shared = Arc::new(SessionShared::new(
                &request,
                entropy,
                generation,
                Arc::clone(&self.registry.generations),
                Arc::clone(&self.registry.subscriptions),
            ));
            let socket = establish(&shared)
                .await
                .map_err(TransportError::ConnectionRefused)?;
            let (outbound, inbound) = mpsc::channel(request.bounds.outbound_frames.get());
            tokio::spawn(driver::drive(Arc::clone(&shared), inbound, socket));

            let session = Arc::new(WebSocketRelaySession { shared, outbound });
            let identity = session.identity();
            // Another acquire may have dialled a connection reaching the same
            // authority while this one was establishing. The first
            // registration wins; this socket is closed rather than leaked.
            let mut entries = self
                .registry
                .entries
                .lock()
                .expect("registry is not poisoned");
            if let Some(existing) = entries.get_mut(&request.relay).and_then(|connections| {
                connections
                    .iter_mut()
                    .find(|entry| entry.can_serve(&request.authority))
            }) {
                existing.holders += 1;
                existing.served.insert(request.authority);
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
            entries
                .entry(request.relay.clone())
                .or_default()
                .push(Entry {
                    session: Arc::clone(&session),
                    holders: 1,
                    served: BTreeSet::from([request.authority]),
                });
            drop(entries);
            // The relay may ask this connection to authenticate at any point,
            // including before anything is sent on it. Only a weak reference
            // is handed to the task: it must not be the thing keeping this
            // session alive, or it would need itself to end.
            let watched: Weak<dyn RelaySession> =
                Arc::downgrade(&(Arc::clone(&session) as Arc<dyn RelaySession>));
            let requests = self.registry.requests.clone();
            tokio::spawn(async move {
                fava_transport::publish_authentication_requests(watched, requests).await;
            });
            Ok(RelaySessionLease::new(
                session,
                Arc::clone(&self.registry) as Arc<dyn LeaseRelease>,
                identity,
            ))
        })
    }

    fn awaiting_authentication(&self) -> Vec<Arc<dyn RelaySession>> {
        let entries = self
            .registry
            .entries
            .lock()
            .expect("registry is not poisoned");
        entries
            .values()
            .flatten()
            .filter(|entry| {
                matches!(
                    entry
                        .session
                        .router()
                        .connection()
                        .borrow()
                        .authentication
                        .progress,
                    fava_relay::Progress::Requested { .. }
                )
            })
            .map(|entry| Arc::clone(&entry.session) as Arc<dyn RelaySession>)
            .collect()
    }

    fn authentication_requests(&self) -> broadcast::Receiver<Arc<dyn RelaySession>> {
        self.registry.requests.subscribe()
    }

    fn holders(&self, relay: &RelayUrl, authority: &Authority) -> Option<NonZeroUsize> {
        let entries = self
            .registry
            .entries
            .lock()
            .expect("registry is not poisoned");
        entries
            .get(relay)?
            .iter()
            .find(|entry| entry.served.contains(authority))
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
                    .flatten()
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

pub(crate) fn mint_generation(counter: &AtomicU64) -> Option<RelayConnection> {
    let previous = counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .ok()?;
    RelayConnection::new(previous + 1)
}

impl Registry {
    fn decrement(
        &self,
        identity: &RelaySessionIdentity,
    ) -> Option<(Arc<WebSocketRelaySession>, ReleaseOutcome)> {
        let mut entries = self.entries.lock().expect("registry is not poisoned");
        let connections = entries.get_mut(&identity.relay)?;
        let position = connections
            .iter()
            .position(|entry| entry.session.identity().connection == identity.connection)?;
        connections[position].holders = connections[position].holders.saturating_sub(1);
        if let Some(holders) = NonZeroUsize::new(connections[position].holders) {
            return Some((
                Arc::clone(&connections[position].session),
                ReleaseOutcome::Retained { holders },
            ));
        }
        let entry = connections.remove(position);
        if connections.is_empty() {
            entries.remove(&identity.relay);
        }
        Some((entry.session, ReleaseOutcome::Closed))
    }
}

impl LeaseRelease for Registry {
    fn release_now(&self, identity: &RelaySessionIdentity) {
        if let Some((session, ReleaseOutcome::Closed)) = self.decrement(identity) {
            tokio::spawn(async move { session.close().await });
        }
    }

    fn release_deterministically<'a>(
        &'a self,
        identity: &'a RelaySessionIdentity,
    ) -> ReleaseFuture<'a> {
        Box::pin(async move {
            let Some((session, outcome)) = self.decrement(identity) else {
                return Err(TransportError::Closed(identity.clone()));
            };
            if outcome == ReleaseOutcome::Closed {
                session.close().await?;
            }
            Ok(outcome)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::mint_generation;
    use fava_transport::RelayConnection;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn generation_exhaustion_never_wraps_or_reuses() {
        let counter = AtomicU64::new(u64::MAX - 1);
        assert_eq!(
            mint_generation(&counter).map(RelayConnection::get),
            Some(u64::MAX)
        );
        assert_eq!(mint_generation(&counter), None);
        assert_eq!(mint_generation(&counter), None);
    }
}
