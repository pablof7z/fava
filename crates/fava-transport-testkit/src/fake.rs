//! An in-memory `Transport` a test drives adversarially.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_state::RelaySessionKey;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, LeaseRelease, OpenRelaySession, RelaySession,
    RelaySessionFuture, RelaySessionIdentity, RelaySessionLease, ReleaseFuture, ReleaseOutcome,
    Transport, TransportError, TransportFailure, TransportShutdownFuture,
};
use tokio::sync::Notify;

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

#[derive(Default)]
struct FakeState {
    entries: Mutex<BTreeMap<RelaySessionKey, Entry>>,
    dials: Mutex<BTreeMap<RelaySessionKey, Arc<AtomicUsize>>>,
    held: Mutex<BTreeSet<RelaySessionKey>>,
    gate: Notify,
    shutting_down: AtomicBool,
}

struct Entry {
    session: Arc<FakeSession>,
    holders: usize,
}

impl FakeTransport {
    /// A transport with no registered sessions.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls for the session currently registered under `key`.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the registry lock.
    #[must_use]
    pub fn relay(&self, key: &RelaySessionKey) -> Option<FakeRelay> {
        let entries = self.state.entries.lock().expect("registry is not poisoned");
        entries.get(key).map(|entry| FakeRelay {
            session: Arc::clone(&entry.session),
        })
    }

    /// Total sockets opened for `key`, counting every reconnect.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the dial counters.
    #[must_use]
    pub fn dials(&self, key: &RelaySessionKey) -> usize {
        self.state
            .dials
            .lock()
            .expect("dial counters are not poisoned")
            .get(key)
            .map_or(0, |count| count.load(Ordering::SeqCst))
    }

    /// Suspend establishment for `key` so an acquire stays pending.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the establishment gate.
    pub fn hold_establishment(&self, key: &RelaySessionKey) {
        self.state
            .held
            .lock()
            .expect("establishment gate is not poisoned")
            .insert(key.clone());
    }

    /// Let a held establishment for `key` complete.
    ///
    /// # Panics
    ///
    /// If a previous test thread panicked while holding the establishment gate.
    pub fn release_establishment(&self, key: &RelaySessionKey) {
        self.state
            .held
            .lock()
            .expect("establishment gate is not poisoned")
            .remove(key);
        self.state.gate.notify_waiters();
    }

    async fn await_establishment(&self, key: &RelaySessionKey) {
        loop {
            let notified = self.state.gate.notified();
            if !self
                .state
                .held
                .lock()
                .expect("establishment gate is not poisoned")
                .contains(key)
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
            if let Some(lease) = self.state.reuse(&request.key) {
                return Ok(lease);
            }
            if tokio::time::timeout(
                request.deadlines.establish,
                self.await_establishment(&request.key),
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
            Ok(self.state.dial(&request))
        })
    }

    fn holders(&self, key: &RelaySessionKey) -> Option<NonZeroUsize> {
        let entries = self.state.entries.lock().expect("registry is not poisoned");
        entries
            .get(key)
            .and_then(|entry| NonZeroUsize::new(entry.holders))
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async move {
            self.state.shutting_down.store(true, Ordering::SeqCst);
            let closing: Vec<_> = {
                let mut entries = self.state.entries.lock().expect("registry is not poisoned");
                std::mem::take(&mut *entries)
                    .into_values()
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
    fn reuse(self: &Arc<Self>, key: &RelaySessionKey) -> Option<RelaySessionLease> {
        let mut entries = self.entries.lock().expect("registry is not poisoned");
        let entry = entries.get_mut(key)?;
        entry.holders += 1;
        let session = Arc::clone(&entry.session);
        let identity = session.identity();
        Some(RelaySessionLease::new(
            session,
            Arc::clone(self) as Arc<dyn LeaseRelease>,
            identity,
        ))
    }

    fn dial(self: &Arc<Self>, request: &OpenRelaySession) -> RelaySessionLease {
        let counter = {
            let mut dials = self.dials.lock().expect("dial counters are not poisoned");
            Arc::clone(
                dials
                    .entry(request.key.clone())
                    .or_insert_with(|| Arc::new(AtomicUsize::new(0))),
            )
        };
        let session = Arc::new(FakeSession::new(request, counter));
        let identity = session.identity();
        self.entries
            .lock()
            .expect("registry is not poisoned")
            .insert(
                request.key.clone(),
                Entry {
                    session: Arc::clone(&session),
                    holders: 1,
                },
            );
        RelaySessionLease::new(
            session,
            Arc::clone(self) as Arc<dyn LeaseRelease>,
            identity,
        )
    }

    fn decrement(&self, key: &RelaySessionKey) -> Option<(Arc<FakeSession>, ReleaseOutcome)> {
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

impl LeaseRelease for FakeState {
    fn release_now(&self, identity: &RelaySessionIdentity) {
        if let Some((session, ReleaseOutcome::Closed)) = self.decrement(&identity.key) {
            session.mark_closed();
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
    pub fn push_frame(&self, frame: Vec<u8>) {
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
