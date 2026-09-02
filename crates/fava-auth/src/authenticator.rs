//! The lifecycle owner: one authenticated session, watched and answered.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use fava_relay::BoundedText;
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_transport::{RelaySession, RelaySessionIdentity, Transport};
use nostr::key::PublicKey;

use crate::challenge::Challenge;
use crate::demand::AuthenticationDemand;
use crate::policy::{AuthenticationDecision, AuthenticationPolicy};
/// How many answers one connection may be driven to send.
///
/// A relay that keeps asking is either broken or hostile; either way this
/// stops it costing an unbounded number of signatures.
pub const MAX_ATTEMPTS: u32 = 8;

mod answer;
mod session_watch;

pub use answer::AnswerError;
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
    /// Read-only handle onto the transport `answer_requests` was given.
    ///
    /// This owner opens no connection and holds none; it only asks the
    /// transport, once set, which connections are currently waiting to be
    /// answered. Set at most once, by [`Authenticator::answer_requests`].
    pub(crate) transport: OnceLock<Arc<dyn Transport>>,
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
    /// Demands lost to a broadcast overflow: a relay asked, and this owner's
    /// subscription fell behind before it read that ask. Nothing names the
    /// specific connection lost, so this counts what happened rather than
    /// pretending it did not.
    pub(crate) lagged: u64,
}

impl State {
    /// Drop every attempt count whose connection has been replaced or is
    /// gone.
    ///
    /// It is keyed by the identity it was recorded under, which a reconnect
    /// leaves behind: the session a `Weak` points at is still alive, but
    /// wears a new identity, and nothing signs, answers, or re-attempts for
    /// the old one again.
    pub(crate) fn prune_stale(&mut self) {
        self.attempts
            .retain(|identity, (_, session)| identity_still_live(identity, session));
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
                transport: OnceLock::new(),
            }),
        }
    }

    /// Connections currently waiting to be answered.
    ///
    /// A connection asked and not yet answered is
    /// [`fava_relay::Progress::Requested`], and that is the only record of
    /// the ask -- read straight off the transport rather than a copy kept
    /// here. Empty before [`Self::answer_requests`] has been called.
    #[must_use]
    pub fn pending(&self) -> Vec<RelaySessionIdentity> {
        self.inner
            .transport
            .get()
            .map_or_else(Vec::new, |transport| {
                transport
                    .awaiting_authentication()
                    .iter()
                    .map(|session| session.identity())
                    .collect()
            })
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
    }

    /// Signal fired whenever anything this owner knows about authentication
    /// changes.
    ///
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
            // Retire what is counted against connections that are gone. This
            // is the only place anything is added, so it is the only place
            // anything needs removing.
            guard.prune_stale();
            let (spent, _) = guard
                .attempts
                .entry(identity.clone())
                .or_insert_with(|| (0, Arc::downgrade(session)));
            *spent = spent.saturating_add(1);
        }
        fava_transport::RelaySessionExt::record_progress(session, progress);
        true
    }

    /// Record that the relay accepted this connection as `account`.
    ///
    /// This is the only thing that sets what the relay knows, and nothing
    /// clears it while the connection lives.
    pub(crate) fn record_accepted(
        identity: &RelaySessionIdentity,
        session: &Arc<dyn fava_transport::RelaySession>,
        account: PublicKey,
    ) -> bool {
        if session.identity() != *identity {
            return false;
        }
        fava_transport::RelaySessionExt::record_accepted(session, account);
        true
    }

    /// Decide one challenge and act on the decision.
    pub(crate) async fn resolve(
        &self,
        identity: RelaySessionIdentity,
        challenge: Challenge,
        session: &Arc<dyn fava_transport::RelaySession>,
    ) {
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
            return;
        }

        match self.inner.policy.decide(&demand) {
            AuthenticationDecision::Decline => {
                self.record(&identity, session, fava_relay::Progress::Declined);
            }
            AuthenticationDecision::Defer => {
                // The connection already carries this demand: the transport
                // set its progress to `Requested` before this owner ever saw
                // it, and `pending` reads that directly. Nothing here needs
                // to remember it a second time -- only wake whoever is
                // waiting on `subscribe` to go look.
            }
            AuthenticationDecision::Authenticate { as_of } => {
                answer::authenticate(self, &demand, as_of, session).await;
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
    use std::sync::Arc;
    use std::time::Duration;

    use fava_transport::{OpenRelaySession, Transport, TransportBounds, TransportDeadlines};
    use fava_transport_testkit::FakeTransport;
    use nostr::types::RelayUrl;

    use super::{MAX_ATTEMPTS, State};

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

    // ARCH-6b.9: an attempt count is keyed by the identity it was recorded
    // under. A reconnect leaves that identity behind without ever visiting it
    // again, so nothing but an active prune stops the map from growing once
    // per connection, forever.
    #[tokio::test]
    async fn prune_stale_drops_entries_left_behind_by_a_reconnect() {
        let (transport, lease) = open_session().await;
        let session = Arc::clone(lease.session());
        let identity = session.identity();

        let mut state = State::default();
        state
            .attempts
            .insert(identity.clone(), (MAX_ATTEMPTS, Arc::downgrade(&session)));

        // The connection this identity names is still the live one: pruning
        // must not touch it.
        state.prune_stale();
        assert_eq!(
            state.attempts.len(),
            1,
            "a live connection's count survives"
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
            .insert(identity, (1, Arc::downgrade(&session)));
        // Every strong handle goes away, the lease included: nothing holds
        // this connection any more.
        drop(session);
        drop(lease);

        state.prune_stale();
        assert!(
            state.attempts.is_empty(),
            "an attempt count outlives no session"
        );
    }
}
