//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher and NIP-42 AUTH-capable variant.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_relay::RelaySessionKey;
use fava_session::Session;
use fava_transport::{
    HandoffCorrelation, HandoffOutcome, OpenRelaySession, RelayInbound, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};
use fava_write::{EventBuilder, Kind, Tag};

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

/// NIP-42 authentication-capable publisher.
///
/// Performs the full NIP-42 AUTH handshake when the relay demands it: signs a
/// kind-22242 challenge-response event using the session signer for the
/// publishing account, sends the `AUTH` frame, then resends the original
/// `EVENT` and waits for its `OK`.
///
/// If no signer is attached for the event's public key, or if signing fails,
/// the attempt reports `AuthenticationRequired` so the delivery layer can
/// record the shortfall without retrying indefinitely.
#[derive(Clone)]
pub struct Nip42Publisher {
    session: Session,
}

impl Nip42Publisher {
    /// Construct a publisher that signs auth challenges with the given session.
    #[must_use]
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

impl Publisher for Nip42Publisher {
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
            let session = Arc::clone(lease.session());
            let mut inbound = session.messages();

            // Encode the EVENT frame once; we may need to resend it after AUTH.
            let event_frame = match encode_client(&ClientMessage::event(attempt.event.clone())) {
                Ok(frame) => frame.into_bytes(),
                Err(error) => {
                    let _ = lease.release().await;
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };

            // Send the initial EVENT.
            match session
                .send(
                    event_frame.clone(),
                    HandoffCorrelation::new(u64::from(attempt.number)),
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

            let relay_url = attempt.session.relay.to_string();
            let pubkey = attempt.event.pubkey;
            let event_id = attempt.event.id;
            let session_ref = &self.session;

            let result = tokio::time::timeout(attempt.timeout, async {
                let mut authed = false;
                for _ in 0..MAX_INBOUND_FRAMES {
                    let item = inbound
                        .next_inbound()
                        .await
                        .map_err(|e| format!("session closed: {e}"))?;
                    let RelayInbound::Frame { frame, .. } = item else {
                        return Err(format!("relay session ended: {item:?}"));
                    };
                    let text = String::from_utf8_lossy(&frame).into_owned();
                    let message = decode_relay(&text).map_err(|e| e.to_string())?;
                    match message {
                        RelayMessage::Ok {
                            event_id: eid,
                            status,
                            message,
                        } if eid == event_id => {
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
                        RelayMessage::Auth { challenge } if !authed => {
                            let challenge = challenge.into_owned();
                            // Build and sign a kind-22242 auth event.
                            let auth_event = match build_auth_event(pubkey, &relay_url, &challenge)
                            {
                                Ok(ev) => ev,
                                Err(_) => {
                                    return Ok(PublishOutcome::AuthenticationRequired);
                                }
                            };
                            let Some((generation, _)) = session_ref.signer(pubkey) else {
                                return Ok(PublishOutcome::AuthenticationRequired);
                            };
                            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                            let signed = match session_ref
                                .invoke_signer(pubkey, generation, auth_event, cancel_rx)
                            {
                                Some(fut) => match fut.await {
                                    Ok(ev) => ev,
                                    Err(_) => {
                                        drop(cancel_tx);
                                        return Ok(PublishOutcome::AuthenticationRequired);
                                    }
                                },
                                None => {
                                    return Ok(PublishOutcome::AuthenticationRequired);
                                }
                            };
                            drop(cancel_tx);
                            let auth_frame = match encode_client(&ClientMessage::auth(signed)) {
                                Ok(f) => f.into_bytes(),
                                Err(e) => return Err(format!("AUTH encode: {e}")),
                            };
                            // Send AUTH response.
                            let auth_correlation =
                                HandoffCorrelation::new(u64::from(attempt.number) | (1 << 32));
                            match session.send(auth_frame, auth_correlation).await {
                                HandoffOutcome::HandedOff { .. } => {}
                                HandoffOutcome::NotHandedOff { reason, .. } => {
                                    return Err(format!("AUTH send failed: {reason:?}"));
                                }
                                HandoffOutcome::Ambiguous { .. } => {}
                            }
                            // Resend the original EVENT after authenticating.
                            let resend_correlation =
                                HandoffCorrelation::new(u64::from(attempt.number) | (2 << 32));
                            match session.send(event_frame.clone(), resend_correlation).await {
                                HandoffOutcome::HandedOff { .. } => {}
                                HandoffOutcome::NotHandedOff { reason, .. } => {
                                    return Ok(PublishOutcome::NotHandedOff {
                                        reason: format!("{reason:?}"),
                                    });
                                }
                                HandoffOutcome::Ambiguous { reason, .. } => {
                                    return Ok(PublishOutcome::OutcomeUnknown {
                                        reason: format!("{reason:?}"),
                                    });
                                }
                            }
                            authed = true;
                        }
                        // Auth OK or any other OK — keep listening for our event's OK.
                        RelayMessage::Auth { .. }
                        | RelayMessage::Ok { .. }
                        | RelayMessage::Event { .. }
                        | RelayMessage::EndOfStoredEvents(_)
                        | RelayMessage::Closed { .. }
                        | RelayMessage::Count { .. }
                        | RelayMessage::NegMsg { .. }
                        | RelayMessage::NegErr { .. } => {}
                        RelayMessage::Notice(msg) => {
                            return Err(format!("relay NOTICE: {msg}"));
                        }
                    }
                }
                Err(format!(
                    "matching OK absent after {MAX_INBOUND_FRAMES} relay frames"
                ))
            })
            .await;

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

/// Build the NIP-42 kind-22242 auth challenge-response event (unsigned).
fn build_auth_event(
    pubkey: nostr::key::PublicKey,
    relay_url: &str,
    challenge: &str,
) -> Result<fava_write::UnsignedEvent, fava_write::EventBuildError> {
    let relay_tag = Tag::parse(["relay", relay_url])
        .map_err(|e| fava_write::EventBuildError::Encoding(e.to_string()))?;
    let challenge_tag = Tag::parse(["challenge", challenge])
        .map_err(|e| fava_write::EventBuildError::Encoding(e.to_string()))?;
    EventBuilder::new(pubkey, Kind::from_u16(22242))
        .tag(relay_tag)
        .tag(challenge_tag)
        .build()
}

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
                    HandoffCorrelation::new(u64::from(attempt.number)),
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
