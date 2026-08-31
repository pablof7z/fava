//! The lifecycle owner: one authenticated session, watched and answered.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use fava_relay::{AuthenticationState, BoundedText, RelayAccess, RelaySessionKey};
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_transport::{RelayConnection, RelaySessionIdentity, Transport};
use fava_write::PublicKey;
use tokio::sync::watch;

use crate::challenge::Challenge;
use crate::demand::{AuthenticationDemand, AuthenticationDemandId, PendingAuthentication};
use crate::policy::{AuthenticationDecision, AuthenticationPolicy};
use crate::state::SessionAuthentication;

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
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) signers: fava_session::Session,
    pub(crate) policy: Arc<dyn AuthenticationPolicy>,
    pub(crate) runtime: Runtime,
    pub(crate) state: Mutex<State>,
    pub(crate) pending_changed: watch::Sender<u64>,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) sessions: BTreeMap<RelaySessionKey, SessionAuthentication>,
    pub(crate) deferred: BTreeMap<AuthenticationDemandId, AuthenticationDemand>,
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

    pub(crate) fn entry(&mut self, key: &RelaySessionKey) -> &mut SessionAuthentication {
        self.sessions
            .entry(key.clone())
            .or_insert_with(|| SessionAuthentication::new(key.clone()))
    }

    /// Drop every deferred demand belonging to a generation that is gone.
    pub(crate) fn drop_deferred_before(
        &mut self,
        key: &RelaySessionKey,
        current: RelayConnection,
    ) -> bool {
        let stale: Vec<_> = self
            .deferred
            .iter()
            .filter(|(_, demand)| {
                demand.session.key == *key && demand.session.connection != current
            })
            .map(|(id, _)| *id)
            .collect();
        for id in &stale {
            self.deferred.remove(id);
        }
        !stale.is_empty()
    }
}

impl Authenticator {
    /// Build the owner for one engine.
    #[must_use]
    pub fn new(
        transport: Arc<dyn Transport>,
        signers: fava_session::Session,
        policy: Arc<dyn AuthenticationPolicy>,
        runtime: Runtime,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                transport,
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
            .map(|(id, demand)| PendingAuthentication {
                id: *id,
                session: demand.session.clone(),
            })
            .collect()
    }

    /// Signal fired whenever the set of deferred demands changes.
    ///
    /// A demand appears when a policy defers, and disappears when it is
    /// answered or its connection is replaced.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.pending_changed.subscribe()
    }

    /// How far authentication has got on one session.
    #[must_use]
    pub fn state(&self, key: &RelaySessionKey) -> Option<AuthenticationState> {
        self.lock().sessions.get(key)?.state().cloned()
    }

    /// Whether one session's current generation is authenticated.
    #[must_use]
    pub fn authenticated(&self, key: &RelaySessionKey) -> bool {
        self.lock()
            .sessions
            .get(key)
            .is_some_and(SessionAuthentication::authenticated)
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
    pub(crate) fn record(
        &self,
        identity: &RelaySessionIdentity,
        state: AuthenticationState,
    ) -> bool {
        let mut guard = self.lock();
        let entry = guard.entry(&identity.key);
        if entry.generation() != Some(identity.connection) {
            return false;
        }
        entry.resolved(identity.connection, state);
        true
    }

    /// Retain a demand a policy deferred, and signal the change.
    pub(crate) fn defer(&self, demand: &AuthenticationDemand) -> AuthenticationDemandId {
        let mut guard = self.lock();
        let id = guard.mint_id();
        guard.deferred.insert(id, demand.clone());
        guard.entry(&demand.session.key).resolved(
            demand.session.connection,
            AuthenticationState::AwaitingAnswer,
        );
        drop(guard);
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

    /// Account this session authenticates as, when it has one.
    pub(crate) fn account(key: &RelaySessionKey) -> Option<PublicKey> {
        match key.access {
            RelayAccess::Authenticated(account) => Some(account),
            RelayAccess::Public => None,
        }
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

        {
            let mut guard = self.lock();
            let entry = guard.entry(&identity.key);
            entry.challenged(identity.connection, demand.challenge.clone());
            if !entry.may_attempt() {
                entry.resolved(
                    identity.connection,
                    AuthenticationState::Failed {
                        reason: BoundedText::new(format!(
                            "relay re-challenged past the {} attempt bound",
                            SessionAuthentication::MAX_ATTEMPTS
                        )),
                    },
                );
                return None;
            }
        }

        match self.inner.policy.decide(&demand) {
            AuthenticationDecision::Decline => {
                self.record(&identity, AuthenticationState::Declined);
                None
            }
            AuthenticationDecision::Defer => Some(self.defer(&demand)),
            AuthenticationDecision::Authenticate => {
                answer::authenticate(self, &demand, session).await;
                None
            }
        }
    }

    /// Cancellation token for one session's watch, minted by the runtime.
    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.inner.runtime.cancellation_token()
    }
}
