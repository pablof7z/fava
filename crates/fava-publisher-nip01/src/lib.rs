//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_relay::RelaySessionKey;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};

const MAX_INBOUND_FRAMES: usize = 64;

/// Deadlines and bounds this publisher hands the transport for one attempt.
/// The attempt's own timeout is the only Fava-owned duration it knows.
fn open_request(key: &RelaySessionKey, timeout: std::time::Duration) -> OpenRelaySession {
    let frames = |count: usize| NonZeroUsize::new(count).expect("constant is non-zero");
    OpenRelaySession {
        key: key.clone(),
        deadlines: TransportDeadlines {
            establish: timeout,
            write: timeout,
            idle: timeout,
            close: timeout,
        },
        bounds: TransportBounds {
            inbound_frames: frames(MAX_INBOUND_FRAMES),
            outbound_frames: frames(4),
            max_frame_bytes: frames(1_048_576),
        },
        reconnect_attempts: None,
    }
}

/// NIP-01 publisher using the selected relay transport.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nip01Publisher;

impl Publisher for Nip01Publisher {
    // Phase 07.6 moves this whole flow behind the runtime's provider call; the
    // body stays one sequence until then.
    #[allow(clippy::too_many_lines)]
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            let lease = match transport
                .acquire_session(open_request(&attempt.session, attempt.timeout))
                .await
            {
                Ok(lease) => lease,
                Err(error) => {
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            let session = std::sync::Arc::clone(lease.session());
            let mut inbound = session.messages();
            let frame = match encode_client(&ClientMessage::event(attempt.event.clone())) {
                Ok(frame) => frame,
                Err(error) => {
                    let _ = lease.release().await;
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            match session
                .send(
                    frame.into_bytes(),
                    HandoffCorrelation(u64::from(attempt.number)),
                )
                .await
            {
                HandoffOutcome::NotHandedOff { reason, .. } => {
                    let _ = lease.release().await;
                    return PublishOutcome::NotHandedOff {
                        reason: format!("{reason:?}"),
                    };
                }
                HandoffOutcome::Ambiguous { reason, .. } => {
                    let _ = lease.release().await;
                    return PublishOutcome::OutcomeUnknown {
                        reason: format!("{reason:?}"),
                    };
                }
                HandoffOutcome::HandedOff { .. } => {}
            }
            let result = tokio::time::timeout(attempt.timeout, async {
                for _ in 0..MAX_INBOUND_FRAMES {
                    let item = inbound
                        .next_inbound()
                        .await
                        .map_err(|error| error.to_string())?;
                    let RelayInbound::Frame { frame, .. } = item else {
                        return Err(format!("relay session ended: {item:?}"));
                    };
                    let frame = String::from_utf8_lossy(&frame).into_owned();
                    let message = decode_relay(&frame).map_err(|error| error.to_string())?;
                    match message {
                        RelayMessage::Ok {
                            event_id,
                            status,
                            message,
                        } if event_id == attempt.event.id => {
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
            // Release this attempt's hold. The session closes only when the
            // last holder lets go; other observations and publications share it.
            inbound.close();
            let _ = lease.release().await;
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
