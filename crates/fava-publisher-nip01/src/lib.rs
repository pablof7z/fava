//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher.

use std::future::Future;
use std::pin::Pin;

use std::sync::Arc;

use fava_auth::{Authentication, AuthenticationOutcome, RelayChallenge};
use fava_nip11::RelayInformationFetcher;
use fava_publisher::{
    EventLimitFacts, PublishAttempt, PublishOutcome, Publisher, RelayWriteLimits,
};
use fava_transport::{HandoffOutcome, RelaySession, Transport};
use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};

const MAX_RESPONSE_BYTES: usize = 4_096;
const MAX_INBOUND_FRAMES: usize = 64;

/// NIP-01 publisher using the selected relay transport.
///
/// Answering a NIP-42 challenge is attempt mechanism, not policy. The selected
/// [`Authentication`] carries the application's policy and signer choice, so
/// this publisher never decides which identity may hold relay access.
#[derive(Clone, Default)]
pub struct Nip01Publisher {
    authentication: Option<Arc<Authentication>>,
    relay_information: Option<Arc<dyn RelayInformationFetcher>>,
}

impl Nip01Publisher {
    /// Publish without answering any relay authentication challenge.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            authentication: None,
            relay_information: None,
        }
    }

    /// Answer mid-attempt challenges with the selected application policy.
    #[must_use]
    pub fn authenticated(authentication: Arc<Authentication>) -> Self {
        Self {
            authentication: Some(authentication),
            relay_information: None,
        }
    }

    /// Refuse knowingly-invalid work using the limits a relay declares.
    ///
    /// Without a relay-information service every limit stays unknown and this
    /// publisher sends what the application asked for. No claim is invented.
    #[must_use]
    pub fn with_relay_information<T>(mut self, fetcher: Arc<T>) -> Self
    where
        T: RelayInformationFetcher + 'static,
    {
        self.relay_information = Some(fetcher);
        self
    }

    /// Resolve the write limits one relay currently declares.
    async fn write_limits(&self, attempt: &PublishAttempt) -> RelayWriteLimits {
        let Some(fetcher) = self.relay_information.as_ref() else {
            return RelayWriteLimits::unknown();
        };
        (fetcher.get(attempt.session.relay.clone()).await).map_or_else(
            |_| RelayWriteLimits::unknown(),
            |document| document.limitation.writes(),
        )
    }

    /// Answer one challenge and report whether the attempt may continue.
    async fn authenticate(
        &self,
        challenge: String,
        session: &dyn RelaySession,
    ) -> Result<(), String> {
        let Some(authentication) = self.authentication.as_ref() else {
            return Err("relay requires authentication and none is selected".to_owned());
        };
        let challenge = RelayChallenge::new(session.key().clone(), session.generation(), challenge)
            .map_err(|error| error.to_string())?;
        match authentication.answer(&challenge, session).await {
            AuthenticationOutcome::Accepted { .. } => Ok(()),
            AuthenticationOutcome::Refused { message } => {
                Err(format!("relay refused authentication: {message}"))
            }
            AuthenticationOutcome::Declined { reason } => {
                Err(format!("application declined authentication: {reason}"))
            }
            AuthenticationOutcome::Failed { reason } => {
                Err(format!("authentication failed: {reason}"))
            }
        }
    }

    /// Read bounded relay frames until this exact attempt has an outcome.
    async fn await_outcome(
        &self,
        attempt: &PublishAttempt,
        session: &dyn RelaySession,
        event_frame: &str,
    ) -> Result<PublishOutcome, String> {
        for _ in 0..MAX_INBOUND_FRAMES {
            let frame = session
                .next_message()
                .await
                .map_err(|error| error.to_string())?;
            let message = decode_relay(&frame).map_err(|error| error.to_string())?;
            match message {
                RelayMessage::Ok {
                    event_id,
                    status,
                    message,
                } if event_id == attempt.event.id => {
                    if message.len() > MAX_RESPONSE_BYTES {
                        return Err(format!(
                            "relay OK text exceeds {MAX_RESPONSE_BYTES}-byte bound"
                        ));
                    }
                    return Ok(if status {
                        PublishOutcome::Acknowledged {
                            message: message.into_owned(),
                        }
                    } else {
                        PublishOutcome::Rejected {
                            message: message.into_owned(),
                        }
                    });
                }
                RelayMessage::Auth { challenge } => {
                    if let Err(reason) = self.authenticate(challenge.into_owned(), session).await {
                        return Ok(PublishOutcome::AuthenticationDenied { reason });
                    }
                    match session.send(event_frame.to_owned()).await {
                        HandoffOutcome::HandedOff => {}
                        HandoffOutcome::NotHandedOff { reason } => {
                            return Ok(PublishOutcome::NotHandedOff { reason });
                        }
                        HandoffOutcome::Ambiguous { reason } => {
                            return Ok(PublishOutcome::OutcomeUnknown { reason });
                        }
                    }
                }
                RelayMessage::Notice(message) => {
                    return Err(format!("relay NOTICE: {message}"));
                }
                RelayMessage::Event { .. }
                | RelayMessage::Ok { .. }
                | RelayMessage::EndOfStoredEvents(_)
                | RelayMessage::Closed { .. }
                | RelayMessage::Count { .. }
                | RelayMessage::NegMsg { .. }
                | RelayMessage::NegErr { .. } => {}
            }
        }
        Err(format!(
            "matching OK absent after {MAX_INBOUND_FRAMES} relay frames"
        ))
    }
}

impl Publisher for Nip01Publisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            let event_frame = match encode_client(&ClientMessage::event(attempt.event.clone())) {
                Ok(frame) => frame,
                Err(error) => {
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            // Refuse work the relay has told us it cannot accept before any
            // connection exists, so knowingly invalid bytes are never sent.
            let limits = self.write_limits(&attempt).await;
            if let Some(reason) =
                limits.refusal(&EventLimitFacts::measure(&attempt.event, event_frame.len()))
            {
                return PublishOutcome::RefusedByLimit { reason };
            }
            let session = match transport.open_session(attempt.session.clone()).await {
                Ok(session) if session.key() == &attempt.session => session,
                Ok(session) => {
                    let _ = session.close().await;
                    return PublishOutcome::NotHandedOff {
                        reason: "transport returned the wrong relay session identity".to_owned(),
                    };
                }
                // No connection exists, so no attempt was spent.
                Err(error) => {
                    return PublishOutcome::Unreachable {
                        reason: error.to_string(),
                    };
                }
            };
            match session.send(event_frame.clone()).await {
                HandoffOutcome::NotHandedOff { reason } => {
                    let _ = session.close().await;
                    return PublishOutcome::NotHandedOff { reason };
                }
                HandoffOutcome::Ambiguous { reason } => {
                    let _ = session.close().await;
                    return PublishOutcome::OutcomeUnknown { reason };
                }
                HandoffOutcome::HandedOff => {}
            }
            let result = tokio::time::timeout(
                attempt.timeout,
                self.await_outcome(&attempt, session.as_ref(), &event_frame),
            )
            .await;
            let _ = session.close().await;
            match result {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(reason)) => PublishOutcome::OutcomeUnknown { reason },
                Err(_) => PublishOutcome::OutcomeUnknown {
                    reason: "publication deadline elapsed after handoff".to_owned(),
                },
            }
        })
    }
}
