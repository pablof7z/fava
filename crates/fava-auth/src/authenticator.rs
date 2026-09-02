//! The lifecycle owner: one authenticated session, watched and answered.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, Weak};

use fava_relay::BoundedText;
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_transport::{RelaySession, RelaySessionIdentity};
use nostr::key::PublicKey;
use tokio::sync::watch;

use crate::challenge::Challenge;
use crate::demand::{AuthenticationDemand, AuthenticationDemandId, PendingAuthentication};
use crate::policy::{AuthenticationDecision, AuthenticationPolicy};
/// How many answers one connection may be driven to send.
///
/// A relay that keeps asking is either broken or hostile; either way this
/// stops it costing an unbounded number of signatures.
pub const MAX_ATTEMPTS: u32 = 8;

mod answer;
mod session_watch;

pub use answer::{AnswerError, AnswerOutcome};
pub use session_watch::WatchError;

/// Task name for one session's authentication watch.
pub(crate) const WATCH_TASK: TaskName = TaskName("auth.watch");
pub(crate) const VERDICT_TASK: TaskName = TaskName("auth.verdict");
pub(crate) const ANSWER_TASK: TaskName = TaskName("auth.answer");

/// Owns every NIP-42 challenge lifecycle in one engine.
///
/// One lifecycle exists per authenticated session key. The owner leases the
/// session it watches, so a relay's unsolicited challenge is seen even when no
/// query or publication is in flight, and neither of those learns the protocol.
#[derive(Clone)]
pub struct Authenticator {
    inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) signers: fava_session::Session,
    pub(crate) policy: Arc<dyn AuthenticationPolicy>,
    pub(crate) runtime: Runtime,
    pub(crate) state: Mutex<State>,
    pub(crate) pending_changed: watch::Sender<u64>,
}

/// A demand or attempt count, held weakly so a connection nobody is using any
/// more takes its question with it: an answer or a retry for it applies to
/// nothing.
type Held<T> = (T, Weak<dyn RelaySession>);

/// Whether `session` is still the live connection wearing `identity`.
///
/// A reconnect leaves the `Weak` pointing at a session that is still alive
/// but now wears a different identity; a drop leaves it pointing at nothing.
/// Either way, `identity` names a connection nobody answers or retries for
/// any more.
fn identity_still_live(identity: &RelaySessionIdentity, session: &Weak<dyn RelaySession>) -> bool {
    session
        .upgrade()
        .is_some_and(|session| session.identity() == *identity)
}

#[derive(Default)]
pub(crate) struct State {
    /// Answers spent on each connection, and the connection each count
    /// belongs to. A replacement is a different connection and starts at
    /// none, so nothing resets a live entry -- `prune_stale` instead removes
    /// the entries a replacement or a drop leaves behind.
    pub(crate) attempts: BTreeMap<RelaySessionIdentity, Held<u32>>,
    /// Demands a person still owes an answer, and the connection each one
    /// belongs to.
    pub(crate) deferred: BTreeMap<AuthenticationDemandId, Held<AuthenticationDemand>>,
    /// Demands lost to a broadcast overflow: a relay asked, and this owner's
    /// subscription fell behind before it read that ask. Nothing names the
    /// specific connection lost, so this counts what happened rather than
    /// pretending it did not.
    pub(crate) lagged: u64,
    next_id: u64,
    revision: u64,
}

impl State {
    fn mint_id(&mut self) -> AuthenticationDemandId {
        self.next_id = self.next_id.saturating_add(1);
        AuthenticationDemandId::from_nonzero(
            NonZeroU64::new(self.next_id).expect("the counter starts at one"),
        )
    }

    /// Drop every deferred demand already outstanding for this exact session
    /// and connection.
    pub(crate) fn drop_deferred_for(&mut self, session: &RelaySessionIdentity) {
        self.deferred
            .retain(|_, (demand, _)| &demand.session != session);
    }

    /// Drop every attempt count and deferred demand whose connection has
    /// been replaced or is gone.
    ///
    /// Both are keyed by the identity they were recorded under, which a
    /// reconnect leaves behind: the session a `Weak` points at is still
    /// alive, but wears a new identity, and nothing signs, answers, or
    /// re-attempts for the old one again. [`Self::pending`][pending] already
    /// tells a live entry from a dead one this way; this is the same check,
    /// applied so the dead entry is actually removed rather than only hidden
    /// from that read.
    ///
    /// [pending]: Authenticator::pending
    pub(crate) fn prune_stale(&mut self) {
        self.attempts
            .retain(|identity, (_, session)| identity_still_live(identity, session));
        self.deferred
            .retain(|_, (demand, session)| identity_still_live(&demand.session, session));
    }
}

impl Authenticator {
    /// Build the owner for one engine.
    ///
    /// It takes no transport: it answers connections other components open,
    /// and opens none of its own.
    #[must_use]
    pub fn new(
        signers: fava_session::Session,
        policy: Arc<dyn AuthenticationPolicy>,
        runtime: Runtime,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                signers,
                policy,
                runtime,
                state: Mutex::new(State::default()),
                pending_changed: watch::channel(0).0,
            }),
        }
    }

    /// Demands currently awaiting a person's answer.
    #[must_use]
    pub fn pending(&self) -> Vec<PendingAuthentication> {
        let state = self.lock();
        state
            .deferred
            .iter()
            // A question asked on a connection that has been replaced, or is
            // gone, is not waiting on anybody.
            .filter(|(_, (demand, session))| identity_still_live(&demand.session, session))
            .map(|(id, (demand, _))| PendingAuthentication {
                id: *id,
                session: demand.session.clone(),
            })
            .collect()
    }

    /// Demands lost because this owner's watch fell behind the relays it
    /// answers, and never saw the ask at all.
    ///
    /// A relay whose only demand was lost this way sits at
    /// [`fava_relay::Progress::Requested`] until it reconnects or challenges
    /// again: nothing republishes a challenge that has not changed, and this
    /// owner has no way to single out which connection it lost. This count
    /// is the honest alternative to answering silently: it says a demand
    /// went unanswered, even though it cannot say which one.
    #[must_use]
    pub fn lagged(&self) -> u64 {
        self.lock().lagged
    }

    /// Record that the broadcast this owner watches overflowed, dropping
    /// `skipped` asks it never read.
    pub(crate) fn record_lagged(&self, skipped: u64) {
        {
            let mut guard = self.lock();
            guard.lagged = guard.lagged.saturating_add(skipped);
        }
        self.signal();
    }

    /// Signal fired whenever anything this owner knows about authentication
    /// changes.
    ///
    /// That is a session reaching a new state, and a deferred demand
    /// appearing, being answered, or losing the connection it belonged to.
    /// The signal carries no detail: read [`Self::state`] or [`Self::pending`]
    /// after it fires. It may fire without either having changed, so treat it
    /// as a reason to look rather than as the change itself.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.pending_changed.subscribe()
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn inner(&self) -> &Arc<Inner> {
        &self.inner
    }

    /// Record a new state for one generation and publish it.
    /// Write how the challenge in front of one connection is going.
    ///
    /// The connection is where this is read from, so it is where it is kept.
    /// A verdict for a connection that has been replaced is dropped: it
    /// describes one that no longer exists.
    pub(crate) fn record(
        &self,
        identity: &RelaySessionIdentity,
        session: &Arc<dyn fava_transport::RelaySession>,
        progress: fava_relay::Progress,
    ) -> bool {
        if session.identity() != *identity {
            return false;
        }
        if matches!(progress, fava_relay::Progress::Answering { .. }) {
            let mut guard = self.lock();
            let (spent, _) = guard
                .attempts
                .entry(identity.clone())
                .or_insert_with(|| (0, Arc::downgrade(session)));
            *spent = spent.saturating_add(1);
        }
        fava_transport::RelaySessionExt::record_progress(session, progress);
        self.signal();
        true
    }

    /// Record that the relay accepted this connection as `account`.
    ///
    /// This is the only thing that sets what the relay knows, and nothing
    /// clears it while the connection lives.
    pub(crate) fn record_accepted(
        &self,
        identity: &RelaySessionIdentity,
        session: &Arc<dyn fava_transport::RelaySession>,
        account: PublicKey,
    ) -> bool {
        if session.identity() != *identity {
            return false;
        }
        fava_transport::RelaySessionExt::record_accepted(session, account);
        self.signal();
        true
    }

    /// Retain a demand a policy deferred, and signal the change.
    ///
    /// One connection is one conversation. A relay repeating its challenge has
    /// not asked a second question, and nobody can answer one it has already
    /// superseded, so the outstanding ask is replaced rather than joined.
    /// Without this a relay that re-challenges under a deferring policy grows
    /// the set without bound: the attempt ceiling counts attempts, and a
    /// deferred demand never makes one.
    pub(crate) fn defer(
        &self,
        demand: &AuthenticationDemand,
        session: &std::sync::Arc<dyn fava_transport::RelaySession>,
    ) -> AuthenticationDemandId {
        let mut guard = self.lock();
        guard.drop_deferred_for(&demand.session);
        let id = guard.mint_id();
        guard
            .deferred
            .insert(id, (demand.clone(), std::sync::Arc::downgrade(session)));
        drop(guard);
        // Nothing has been signed and nothing sent. The connection stays where
        // the relay left it: asked, and not yet answered.
        self.signal();
        id
    }

    pub(crate) fn signal(&self) {
        let next = {
            let mut guard = self.lock();
            // Every state change is a chance to notice a connection that has
            // been replaced or dropped since the last one, and stop keeping
            // what it left behind.
            guard.prune_stale();
            guard.revision = guard.revision.saturating_add(1);
            guard.revision
        };
        let _ = self.inner.pending_changed.send(next);
    }

    /// Decide one challenge and act on the decision.
    ///
    /// Returns the identity of a demand that was deferred to a person.
    pub(crate) async fn resolve(
        &self,
        identity: RelaySessionIdentity,
        challenge: Challenge,
        session: &Arc<dyn fava_transport::RelaySession>,
    ) -> Option<AuthenticationDemandId> {
        let demand = AuthenticationDemand {
            session: identity.clone(),
            challenge,
        };

        // A relay may not drive unbounded signing by asking again and again.
        let attempts = self
            .lock()
            .attempts
            .get(&identity)
            .map_or(0, |(spent, _)| *spent);
        if attempts >= MAX_ATTEMPTS {
            self.record(
                &identity,
                session,
                fava_relay::Progress::Unanswerable {
                    reason: BoundedText::new(format!(
                        "relay re-challenged past the {MAX_ATTEMPTS} attempt bound"
                    )),
                },
            );
            return None;
        }

        match self.inner.policy.decide(&demand) {
            AuthenticationDecision::Decline => {
                self.record(&identity, session, fava_relay::Progress::Declined);
                None
            }
            AuthenticationDecision::Defer => Some(self.defer(&demand, session)),
            AuthenticationDecision::Authenticate { as_of } => {
                answer::authenticate(self, &demand, as_of, session).await;
                None
            }
        }
    }

    /// Cancellation token for this owner's work, minted by the runtime.
    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.inner.runtime.cancellation_token()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use std::time::Duration;

    use fava_transport::{OpenRelaySession, Transport, TransportBounds, TransportDeadlines};
    use fava_transport_testkit::FakeTransport;
    use nostr::types::RelayUrl;

    use super::{AuthenticationDemand, AuthenticationDemandId, Challenge, MAX_ATTEMPTS, State};

    fn nonzero(value: usize) -> std::num::NonZeroUsize {
        std::num::NonZeroUsize::new(value).expect("constant is non-zero")
    }

    /// A held lease on a fresh session, and the transport it was opened
    /// against. The caller decides whether to keep the lease alive (to drive
    /// a reconnect on the connection it holds) or drop it (to make the
    /// connection actually go away).
    async fn open_session() -> (Arc<FakeTransport>, fava_transport::RelaySessionLease) {
        let transport = Arc::new(FakeTransport::new());
        let relay = RelayUrl::parse("wss://relay.example.com").expect("valid relay url");
        let lease = transport
            .as_ref()
            .acquire_session(OpenRelaySession {
                relay,
                authority: fava_relay::Authority::Unauthenticated,
                deadlines: TransportDeadlines {
                    establish: Duration::from_secs(1),
                    write: Duration::from_secs(1),
                    idle: Duration::from_secs(1),
                    close: Duration::from_secs(1),
                },
                bounds: TransportBounds {
                    inbound_frames: nonzero(64),
                    outbound_frames: nonzero(4),
                    max_frame_bytes: nonzero(1_048_576),
                },
                reconnect_attempts: None,
            })
            .await
            .expect("the test opens the session under test");
        (transport, lease)
    }

    // ARCH-6b.9: an attempt count and a deferred demand are both keyed by the
    // identity they were recorded under. A reconnect leaves that identity
    // behind without ever visiting it again, so nothing but an active prune
    // stops either map from growing once per connection, forever.
    #[tokio::test]
    async fn prune_stale_drops_entries_left_behind_by_a_reconnect() {
        let (transport, lease) = open_session().await;
        let session = Arc::clone(lease.session());
        let identity = session.identity();

        let mut state = State::default();
        state
            .attempts
            .insert(identity.clone(), (MAX_ATTEMPTS, Arc::downgrade(&session)));
        state.deferred.insert(
            AuthenticationDemandId::from_nonzero(NonZeroU64::new(1).expect("non-zero")),
            (
                AuthenticationDemand {
                    session: identity.clone(),
                    challenge: Challenge::new("nonce").expect("bounded challenge"),
                },
                Arc::downgrade(&session),
            ),
        );

        // The connection this identity names is still the live one: pruning
        // must not touch either entry.
        state.prune_stale();
        assert_eq!(
            state.attempts.len(),
            1,
            "a live connection's count survives"
        );
        assert_eq!(
            state.deferred.len(),
            1,
            "a live connection's demand survives"
        );

        // Reconnect: the same session object now wears a different identity.
        // Nothing will ever record against the old one again.
        transport
            .relay(&identity.relay, &fava_relay::Authority::Unauthenticated)
            .expect("the session opened above")
            .reconnect();
        assert_ne!(
            session.identity(),
            identity,
            "the fake actually advanced the connection"
        );

        state.prune_stale();
        assert!(
            state.attempts.is_empty(),
            "an attempt count for a replaced connection is not kept forever"
        );
        assert!(
            state.deferred.is_empty(),
            "a demand for a replaced connection is not kept forever"
        );
    }

    // The other half of ARCH-6b.9: a connection that is dropped entirely,
    // rather than replaced, must not be kept either.
    #[tokio::test]
    async fn prune_stale_drops_entries_for_a_connection_nobody_holds_any_more() {
        let (_transport, lease) = open_session().await;
        let session = Arc::clone(lease.session());
        let identity = session.identity();

        let mut state = State::default();
        state
            .attempts
            .insert(identity.clone(), (1, Arc::downgrade(&session)));
        state.deferred.insert(
            AuthenticationDemandId::from_nonzero(NonZeroU64::new(1).expect("non-zero")),
            (
                AuthenticationDemand {
                    session: identity,
                    challenge: Challenge::new("nonce").expect("bounded challenge"),
                },
                Arc::downgrade(&session),
            ),
        );
        // Every strong handle goes away, the lease included: nothing holds
        // this connection any more.
        drop(session);
        drop(lease);

        state.prune_stale();
        assert!(
            state.attempts.is_empty(),
            "an attempt count outlives no session"
        );
        assert!(state.deferred.is_empty(), "a demand outlives no session");
    }
}
