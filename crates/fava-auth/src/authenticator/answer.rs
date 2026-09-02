//! Signing and sending the response, and the answer a person gives later.

use std::sync::Arc;

use fava_relay::BoundedText;
use fava_transport::RelaySession;
use fava_write::PublicKey;
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

        let (demand, session) = {
            let mut guard = self.lock();
            guard.deferred.remove(&id).ok_or(AnswerError::Unknown)?
        };
        self.signal();

        // An answer belongs to the connection it was shown for. Ask that
        // connection, rather than comparing against a copy of its identity
        // kept here: a connection that is gone, or replaced, answers nothing.
        let live = session
            .upgrade()
            .filter(|session| session.identity() == demand.session);
        let Some(session) = live else {
            return Ok(AnswerOutcome::NoLongerApplicable);
        };

        match decision {
            AuthenticationDecision::Decline => {
                self.record(&demand.session, &session, fava_relay::Progress::Declined);
            }
            AuthenticationDecision::Authenticate { as_of } => {
                authenticate(self, &demand, as_of, &session).await;
            }
            AuthenticationDecision::Defer => unreachable!("refused above"),
        }
        Ok(AnswerOutcome::Applied)
    }
}

/// Sign the challenge response and hand it to the relay.
///
/// Nothing here decides policy: the decision already happened. A missing
/// signer is a failure of this attempt, not a refusal by the relay.
pub(super) async fn authenticate(
    authenticator: &Authenticator,
    demand: &AuthenticationDemand,
    account: PublicKey,
    session: &Arc<dyn RelaySession>,
) {
    let identity = &demand.session;
    let unsigned = match auth_event(account, &identity.relay, &demand.challenge) {
        Ok(event) => event,
        Err(error) => {
            authenticator.record(
                identity,
                session,
                fava_relay::Progress::Unanswerable {
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
            session,
            fava_relay::Progress::Unanswerable {
                reason: BoundedText::new("no signer is attached for this account"),
            },
        );
        return;
    };

    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let Some(signing) = signers.invoke_signer(account, generation, unsigned, cancel_rx) else {
        authenticator.record(
            identity,
            session,
            fava_relay::Progress::Unanswerable {
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
                session,
                fava_relay::Progress::Unanswerable {
                    reason: BoundedText::new(error.to_string()),
                },
            );
            return;
        }
    };

    let mut acknowledged = match fava_transport::RelaySessionExt::answer(session, signed).await {
        Ok(handle) => handle,
        Err(refused) => {
            authenticator.record(
                identity,
                session,
                fava_relay::Progress::Unanswerable {
                    reason: BoundedText::new(format!("{refused:?}")),
                },
            );
            return;
        }
    };
    authenticator.record(
        identity,
        session,
        fava_relay::Progress::Answering { as_of: account },
    );

    // Await the verdict off to the side. Answering a challenge must not block
    // the watch that reads them: a relay may challenge again before it answers,
    // and a relay that never answers must not wedge the owner.
    let owner = authenticator.clone();
    let identity = identity.clone();
    let session = Arc::clone(session);
    let token = authenticator.cancellation();
    let _ = authenticator.inner().runtime.spawn_cancellable(
        crate::authenticator::VERDICT_TASK,
        token,
        async move {
            // A relay refusing the proof has refused it, whatever prefix it
            // chose to say so with; its own words carry the difference.
            let state = match acknowledged.settled().await {
                fava_transport::Settlement::Accepted { .. } => {
                    owner.record_accepted(&identity, &session, account);
                    return;
                }
                fava_transport::Settlement::Rejected { message } => {
                    fava_relay::Progress::Refused { reason: message }
                }
                // The connection that carried the proof is gone, so the proof
                // is void. The next connection challenges again.
                fava_transport::Settlement::Ended(_) => return,
            };
            owner.record(&identity, &session, state);
        },
    );
}
