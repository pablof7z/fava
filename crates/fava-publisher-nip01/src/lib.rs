//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher.

use std::future::Future;
use std::pin::Pin;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_transport::{HandoffOutcome, Transport};
use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};

const MAX_RESPONSE_BYTES: usize = 4_096;
const MAX_INBOUND_FRAMES: usize = 64;

/// NIP-01 publisher using the selected relay transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nip01Publisher;

impl Publisher for Nip01Publisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            let session = match transport.open_session(attempt.session.clone()).await {
                Ok(session) if session.key() == &attempt.session => session,
                Ok(session) => {
                    let _ = session.close().await;
                    return PublishOutcome::NotHandedOff {
                        reason: "transport returned the wrong relay session identity".to_owned(),
                    };
                }
                Err(error) => {
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            let frame = match encode_client(&ClientMessage::event(attempt.event.clone())) {
                Ok(frame) => frame,
                Err(error) => {
                    let _ = session.close().await;
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            match session.send(frame).await {
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
            let result = tokio::time::timeout(attempt.timeout, async {
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
                        RelayMessage::Auth { .. } => {
                            return Ok(PublishOutcome::AuthenticationRequired);
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
            })
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
