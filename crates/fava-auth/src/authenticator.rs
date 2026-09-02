//! The lifecycle owner: one authenticated session, watched and answered.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use fava_relay::BoundedText;
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_transport::RelaySessionIdentity;
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

#[derive(Default)]
pub(crate) struct State {
    /// Answers spent on each connection. A replacement is a different
    /// connection and starts at none, so nothing resets this.
    pub(crate) attempts: BTreeMap<RelaySessionIdentity, u32>,
    /// Demands a person still owes an answer, and the connection each one
    /// belongs to. Held weakly: a connection nobody is using any more takes
    /// its question with it, and an answer for it applies to nothing.
    pub(crate) deferred: BTreeMap<
        AuthenticationDemandId,
        (
            AuthenticationDemand,
            std::sync::Weak<dyn fava_transport::RelaySession>,
        ),
    >,
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
            .filter(|(_, (demand, session))| {
                session
                    .upgrade()
                    .is_some_and(|session| session.identity() == demand.session)
            })
            .map(|(id, (demand, _))| PendingAuthentication {
                id: *id,
                session: demand.session.clone(),
            })
            .collect()
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
    /// Write a verdict onto the connection it belongs to.
    ///
    /// The connection is where this is read from, so it is where it is kept.
    /// A verdict for a connection that has been replaced is dropped: it
    /// describes one that no longer exists.
    pub(crate) fn record(
        &self,
        identity: &RelaySessionIdentity,
        session: &Arc<dyn fava_transport::RelaySession>,
        state: fava_relay::Authentication,
    ) -> bool {
        if session.identity() != *identity {
            return false;
        }
        if matches!(state, fava_relay::Authentication::Authenticating { .. }) {
            let mut guard = self.lock();
            let spent = guard.attempts.entry(identity.clone()).or_default();
            *spent = spent.saturating_add(1);
        }
        fava_transport::RelaySessionExt::record_authentication(session, state);
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
        if self.lock().attempts.get(&identity).copied().unwrap_or(0) >= MAX_ATTEMPTS {
            self.record(
                &identity,
                session,
                fava_relay::Authentication::Failed {
                    reason: BoundedText::new(format!(
                        "relay re-challenged past the {MAX_ATTEMPTS} attempt bound"
                    )),
                },
            );
            return None;
        }

        match self.inner.policy.decide(&demand) {
            AuthenticationDecision::Decline => {
                self.record(&identity, session, fava_relay::Authentication::Declined);
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
