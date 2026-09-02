//! An in-memory `Transport` a test drives adversarially.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use fava_relay::Authority;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, LeaseRelease, OpenRelaySession, RelayConnection,
    RelaySession, RelaySessionFuture, RelaySessionIdentity, RelaySessionLease, ReleaseFuture,
    ReleaseOutcome, Transport, TransportError, TransportFailure, TransportShutdownFuture,
};
use nostr::types::RelayUrl;
use tokio::sync::{Notify, broadcast};

use crate::session::FakeSession;

/// A relay transport with no socket, driven entirely by test controls.
///
/// It expresses the five adversarial classes the frozen contract requires:
/// pending establishment ([`FakeTransport::hold_establishment`]), mid-operation
/// failure ([`FakeRelay::fail_now`]), mid-operation cancellation (drop a
/// [`RelaySession::send`] future while [`FakeRelay::block_queue`] holds it),
/// stale completion ([`FakeRelay::reconnect`]), and a slow peer applying
/// backpressure ([`FakeRelay::stall_writer`]).
#[derive(Clone, Default)]
pub struct FakeTransport {
    state: Arc<FakeState>,
}

/// Unheard authentication requests held before the oldest is dropped.
const REQUEST_BACKLOG: usize = 64;

struct FakeState {
    /// Every live connection to a relay, grouped by relay: more than one can
    /// exist when a request needs an authority none of the others can reach.
    entries: Mutex<BTreeMap<RelayUrl, Vec<Entry>>>,
    dials: Mutex<BTreeMap<RelayUrl, Arc<AtomicUsize>>>,
    held: Mutex<BTreeSet<RelayUrl>>,
    gate: Notify,
    shutting_down: AtomicBool,
    generations: Arc<AtomicU64>,
    /// Sessions whose relay has asked them to authenticate.
    requests: broadcast::Sender<Arc<dyn RelaySession>>,
    subscriptions: Arc<AtomicU64>,
}

impl Default for FakeState {
    fn default() -> Self {
        Self {
            entries: Mutex::default(),
            dials: Mutex::default(),
            held: Mutex::default(),
            gate: Notify::default(),
            shutting_down: AtomicBool::default(),
            generations: Arc::default(),
            requests: broadcast::Sender::new(REQUEST_BACKLOG),
            subscriptions: Arc::default(),
        }
    }
}

struct Entry {
    session: Arc<FakeSession>,
    holders: usize,
    /// Every authority an acquire has ever been satisfied under on this
    /// connection. Introspection (`session`, `relay`, `holders`) looks a
    /// connection up by this: it names what a test acquired, and must keep
    /// naming it even after the connection's live authentication moves on to
    /// a refusal that can no longer reach anything.
    served: BTreeSet<Authority>,
}

impl Entry {
    /// Whether this connection can still reach `authority`, right now.
    fn can_serve(&self, authority: &Authority) -> bool {
        self.session
            .router
            .connection()
            .borrow()
            .can_serve(authority)
    }
}

impl FakeTransport {
    /// A transport with no registered sessions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The live session reachable for `relay` under `authority`, for reading
    /// what its connection says.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the registry lock.
    #[must_use]
    pub fn session(
        &self,
        relay: &RelayUrl,
        authority: &Authority,
    ) -> Option<Arc<dyn RelaySession>> {
        let entries = self.state.entries.lock().expect("registry is not poisoned");
        entries
            .get(relay)?
            .iter()
            .find(|entry| entry.served.contains(authority))
            .map(|entry| Arc::clone(&entry.session) as Arc<dyn RelaySession>)
    }

    /// Controls for the connection currently reachable for `relay` under
    /// `authority`.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the registry lock.
    #[must_use]
    pub fn relay(&self, relay: &RelayUrl, authority: &Authority) -> Option<FakeRelay> {
        let entries = self.state.entries.lock().expect("registry is not poisoned");
        entries
            .get(relay)?
            .iter()
            .find(|entry| entry.served.contains(authority))
            .map(|entry| FakeRelay {
                session: Arc::clone(&entry.session),
            })
    }

    /// Total sockets opened for `relay`, counting every reconnect and every
    /// distinct connection.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the dial counters.
    #[must_use]
    pub fn dials(&self, relay: &RelayUrl) -> usize {
        self.state
            .dials
            .lock()
            .expect("dial counters are not poisoned")
            .get(relay)
            .map_or(0, |count| count.load(Ordering::SeqCst))
    }

    /// Suspend establishment for `relay` so an acquire stays pending.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the establishment gate.
    pub fn hold_establishment(&self, relay: &RelayUrl) {
        self.state
            .held
            .lock()
            .expect("establishment gate is not poisoned")
            .insert(relay.clone());
    }

    /// Let a held establishment for `relay` complete.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the establishment gate.
    pub fn release_establishment(&self, relay: &RelayUrl) {
        self.state
            .held
            .lock()
            .expect("establishment gate is not poisoned")
            .remove(relay);
        self.state.gate.notify_waiters();
    }

    /// Leave exactly one physical generation available for exhaustion tests.
    pub fn leave_one_generation(&self) {
        self.state.generations.store(u64::MAX - 1, Ordering::SeqCst);
    }

    /// Exhaust physical generation identity before the next acquisition.
    pub fn exhaust_generations(&self) {
        self.state.generations.store(u64::MAX, Ordering::SeqCst);
    }

    async fn await_establishment(&self, relay: &RelayUrl) {
        loop {
            let notified = self.state.gate.notified();
            if !self
                .state
                .held
                .lock()
                .expect("establishment gate is not poisoned")
                .contains(relay)
            {
                return;
            }
            notified.await;
        }
    }
}

impl Transport for FakeTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        Box::pin(async move {
            if self.state.shutting_down.load(Ordering::SeqCst) {
                return Err(TransportError::ShuttingDown);
            }
            if let Some(lease) = self.state.reuse(&request.relay, &request.authority) {
                return Ok(lease);
            }
            let generation = self
                .state
                .mint_generation()
                .ok_or(TransportError::GenerationExhausted)?;
            if tokio::time::timeout(
                request.deadlines.establish,
                self.await_establishment(&request.relay),
            )
            .await
            .is_err()
            {
                return Err(TransportError::ConnectionRefused(
                    TransportFailure::EstablishTimeout {
                        after: request.deadlines.establish,
                    },
                ));
            }
            Ok(self.state.dial(&request, generation))
        })
    }

    fn authentication_requests(&self) -> broadcast::Receiver<Arc<dyn RelaySession>> {
        self.state.requests.subscribe()
    }

    fn holders(&self, relay: &RelayUrl, authority: &Authority) -> Option<NonZeroUsize> {
        let entries = self.state.entries.lock().expect("registry is not poisoned");
        entries
            .get(relay)?
            .iter()
            .find(|entry| entry.served.contains(authority))
            .and_then(|entry| NonZeroUsize::new(entry.holders))
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async move {
            self.state.shutting_down.store(true, Ordering::SeqCst);
            let closing: Vec<_> = {
                let mut entries = self.state.entries.lock().expect("registry is not poisoned");
                std::mem::take(&mut *entries)
                    .into_values()
                    .flatten()
                    .map(|entry| entry.session)
                    .collect()
            };
            for session in closing {
                session.close().await?;
            }
            Ok(())
        })
    }
}

impl FakeState {
    fn mint_generation(&self) -> Option<RelayConnection> {
        let previous = self
            .generations
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_add(1)
            })
            .ok()?;
        RelayConnection::new(previous + 1)
    }

    fn reuse(
        self: &Arc<Self>,
        relay: &RelayUrl,
        authority: &Authority,
    ) -> Option<RelaySessionLease> {
        let mut entries = self.entries.lock().expect("registry is not poisoned");
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
            Arc::clone(self) as Arc<dyn LeaseRelease>,
            identity,
        ))
    }

    fn dial(
        self: &Arc<Self>,
        request: &OpenRelaySession,
        generation: RelayConnection,
    ) -> RelaySessionLease {
        let counter = {
            let mut dials = self.dials.lock().expect("dial counters are not poisoned");
            Arc::clone(
                dials
                    .entry(request.relay.clone())
                    .or_insert_with(|| Arc::new(AtomicUsize::new(0))),
            )
        };
        let session = Arc::new(FakeSession::new(
            request,
            counter,
            generation,
            Arc::clone(&self.generations),
            Arc::clone(&self.subscriptions),
        ));
        let identity = session.identity();
        self.entries
            .lock()
            .expect("registry is not poisoned")
            .entry(request.relay.clone())
            .or_default()
            .push(Entry {
                session: Arc::clone(&session),
                holders: 1,
                served: BTreeSet::from([request.authority]),
            });
        // Only a weak reference is handed to the task: it must not be the
        // thing keeping this session alive, or it would need itself to end.
        let watched: Weak<dyn RelaySession> =
            Arc::downgrade(&(Arc::clone(&session) as Arc<dyn RelaySession>));
        let requests = self.requests.clone();
        tokio::spawn(async move {
            fava_transport::publish_authentication_requests(watched, requests).await;
        });
        RelaySessionLease::new(session, Arc::clone(self) as Arc<dyn LeaseRelease>, identity)
    }

    fn decrement(
        &self,
        identity: &RelaySessionIdentity,
    ) -> Option<(Arc<FakeSession>, ReleaseOutcome)> {
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

impl LeaseRelease for FakeState {
    fn release_now(&self, identity: &RelaySessionIdentity) {
        if let Some((session, ReleaseOutcome::Closed)) = self.decrement(identity) {
            session.mark_closed();
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

/// Adversarial controls for one registered fake session.
#[derive(Clone)]
pub struct FakeRelay {
    session: Arc<FakeSession>,
}

impl FakeRelay {
    /// Deliver one inbound frame to every live consumer of this session.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn push_frame(&self, frame: &[u8]) {
        self.session.push_frame(frame);
    }

    /// Stop draining the outbound queue: a slow peer applying backpressure.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn stall_writer(&self) {
        self.session.state().writer_stalled = true;
    }

    /// Refuse even to enqueue, so a handoff waits for capacity until its write
    /// deadline. Combined with dropping the future this expresses
    /// mid-operation cancellation.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn block_queue(&self) {
        self.session.state().queue_blocked = true;
    }

    /// Fail the live generation mid-operation with an exact reason.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn fail_now(&self, detail: &str) {
        self.session.fail(detail);
    }

    /// Make every reconnect attempt fail, so the budget is exhausted.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn refuse_reconnects(&self, detail: &str) {
        self.session.state().refuse_reconnect = Some(detail.to_owned());
    }

    /// Retire the current generation and mint the next one under the same
    /// session object, exactly as RELAY-006 requires.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the session lock.
    pub fn reconnect(&self) {
        self.session.reconnect();
    }

    /// Frames still sitting in the stalled outbound queue.
    #[must_use]
    pub fn queued_frames(&self) -> Vec<Vec<u8>> {
        self.session.state().queued.clone()
    }

    /// Frames the fake socket accepted and drained.
    #[must_use]
    pub fn delivered_frames(&self) -> Vec<Vec<u8>> {
        self.session.state().delivered.clone()
    }

    /// Completions produced after their `send` future already returned, such
    /// as a frame stranded in the socket by a mid-operation failure.
    #[must_use]
    pub fn unflushed_completions(&self) -> Vec<HandoffOutcome> {
        self.session.state().completions.clone()
    }

    /// Handoffs whose future the caller dropped before it resolved.
    #[must_use]
    pub fn cancelled_handoffs(&self) -> Vec<HandoffCorrelation> {
        self.session.state().cancelled.clone()
    }
}

/// A lease with no registry behind it, for a fake that owns no refcount.
///
/// Releasing it reports [`ReleaseOutcome::Closed`] and closes the session.
#[must_use]
pub fn detached_lease(session: Arc<dyn RelaySession>) -> RelaySessionLease {
    let identity = session.identity();
    let release = Arc::new(DetachedRelease {
        session: Arc::clone(&session),
    });
    RelaySessionLease::new(session, release, identity)
}

/// The single holder of a detached lease, so releasing it closes the session.
struct DetachedRelease {
    session: Arc<dyn RelaySession>,
}

impl LeaseRelease for DetachedRelease {
    fn release_now(&self, _identity: &RelaySessionIdentity) {}

    fn release_deterministically<'a>(
        &'a self,
        _identity: &'a RelaySessionIdentity,
    ) -> ReleaseFuture<'a> {
        Box::pin(async move {
            self.session.close().await?;
            Ok(ReleaseOutcome::Closed)
        })
    }
}
