//! Signing and sending the response, and the answer a person gives later.

use std::sync::Arc;

use fava_relay::{AuthenticationState, BoundedText};
use fava_transport::{HandoffCorrelation, HandoffOutcome, RelaySession};
use fava_wire::{ClientMessage, encode_client};
use thiserror::Error;
use tokio::sync::watch;

use super::Authenticator;
use crate::demand::{AuthenticationDemand, AuthenticationDemandId};
use crate::event::auth_event;
use crate::policy::AuthenticationDecision;

/// What became of a person's answer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnswerOutcome {
    /// The answer applied and the handshake proceeded.
    Applied,
    /// The connection this demand belonged to was replaced. Nothing was
    /// signed and no session was authenticated.
    NoLongerApplicable,
}

/// Why an answer could not be applied.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AnswerError {
    /// No demand with this identity is awaiting an answer.
    #[error("no demand awaits this answer")]
    Unknown,
    /// A person's answer must be a decision, not another deferral.
    #[error("a deferred demand cannot be answered by deferring again")]
    DeferredAgain,
}

impl Authenticator {
    /// Apply a person's answer to one deferred demand.
    ///
    /// # Errors
    ///
    /// Returns [`AnswerError`] when no such demand awaits, or when the answer
    /// is itself a deferral.
    pub async fn answer(
        &self,
        id: AuthenticationDemandId,
        decision: AuthenticationDecision,
    ) -> Result<AnswerOutcome, AnswerError> {
        if matches!(decision, AuthenticationDecision::Defer) {
            return Err(AnswerError::DeferredAgain);
        }

        let demand = {
            let mut guard = self.lock();
            let demand = guard.deferred.remove(&id).ok_or(AnswerError::Unknown)?;
            let current = guard.entry(&demand.session.key).generation();
            if current != Some(demand.session.generation) {
                drop(guard);
                self.signal();
                return Ok(AnswerOutcome::NoLongerApplicable);
            }
            demand
        };
        self.signal();

        match decision {
            AuthenticationDecision::Decline => {
                self.record(&demand.session, AuthenticationState::Declined);
            }
            AuthenticationDecision::Authenticate => {
                let Some(session) = self.live_session(&demand).await else {
                    self.record(
                        &demand.session,
                        AuthenticationState::Failed {
                            reason: BoundedText::new(
                                "the relay session was gone before the answer arrived",
                            ),
                        },
                    );
                    return Ok(AnswerOutcome::NoLongerApplicable);
                };
                authenticate(self, &demand, &session).await;
            }
            AuthenticationDecision::Defer => unreachable!("refused above"),
        }
        Ok(AnswerOutcome::Applied)
    }

    /// Reacquire the session a deferred demand belongs to, if it still exists
    /// at the same generation.
    async fn live_session(&self, demand: &AuthenticationDemand) -> Option<Arc<dyn RelaySession>> {
        let request = super::session_watch::open_request(&demand.session.key);
        let lease = self.inner().transport.acquire_session(request).await.ok()?;
        if lease.acquired_identity().generation != demand.session.generation {
            let _ = lease.release().await;
            return None;
        }
        let session = Arc::clone(lease.session());
        let _ = lease.release().await;
        Some(session)
    }
}

/// Sign the challenge response and hand it to the relay.
///
/// Nothing here decides policy: the decision already happened. A missing
/// signer is a failure of this attempt, not a refusal by the relay.
pub(super) async fn authenticate(
    authenticator: &Authenticator,
    demand: &AuthenticationDemand,
    session: &Arc<dyn RelaySession>,
) {
    let identity = &demand.session;
    let Some(account) = Authenticator::account(&identity.key) else {
        authenticator.record(
            identity,
            AuthenticationState::Failed {
                reason: BoundedText::new("a public session has no account to authenticate as"),
            },
        );
        return;
    };

    let unsigned = match auth_event(account, &identity.key.relay, &demand.challenge) {
        Ok(event) => event,
        Err(error) => {
            authenticator.record(
                identity,
                AuthenticationState::Failed {
                    reason: BoundedText::new(error.to_string()),
                },
            );
            return;
        }
    };

    let signers = &authenticator.inner().signers;
    let Some((generation, _)) = signers.signer(account) else {
        authenticator.record(
            identity,
            AuthenticationState::Failed {
                reason: BoundedText::new("no signer is attached for this account"),
            },
        );
        return;
    };

    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let Some(signing) = signers.invoke_signer(account, generation, unsigned, cancel_rx) else {
        authenticator.record(
            identity,
            AuthenticationState::Failed {
                reason: BoundedText::new("the signer was detached before the response was signed"),
            },
        );
        return;
    };

    let signed = match signing.await {
        Ok(signed) => signed,
        Err(error) => {
            authenticator.record(
                identity,
                AuthenticationState::Failed {
                    reason: BoundedText::new(error.to_string()),
                },
            );
            return;
        }
    };

    let event_id = signed.id;
    let frame = match encode_client(&ClientMessage::auth(signed)) {
        Ok(frame) => frame.into_bytes(),
        Err(error) => {
            authenticator.record(
                identity,
                AuthenticationState::Failed {
                    reason: BoundedText::new(error.to_string()),
                },
            );
            return;
        }
    };

    let correlation = HandoffCorrelation::new(identity.generation.get());
    match session.send(frame, correlation).await {
        HandoffOutcome::HandedOff { .. } => {
            authenticator
                .lock()
                .awaiting_ok
                .insert(identity.key.clone(), event_id);
            authenticator.record(identity, AuthenticationState::Attempted);
        }
        HandoffOutcome::NotHandedOff { reason, .. } => {
            authenticator.record(
                identity,
                AuthenticationState::Failed {
                    reason: BoundedText::new(format!("{reason:?}")),
                },
            );
        }
        HandoffOutcome::Ambiguous { reason, .. } => {
            authenticator.record(
                identity,
                AuthenticationState::Failed {
                    reason: BoundedText::new(format!("{reason:?}")),
                },
            );
        }
    }
}
