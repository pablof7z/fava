//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher and NIP-42 AUTH-capable variant.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_relay::RelaySessionKey;
use fava_session::Session;
use fava_transport::{
    HandoffOutcome, OpenRelaySession, SessionEnded, Settlement, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_write::{EventBuilder, Kind, Tag};

/// Inbound queue depth this publisher asks the transport for.
const INBOUND_FRAMES: usize = 64;

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
            inbound_frames: frames(INBOUND_FRAMES),
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
            let relay_url = attempt.session.relay.to_string();
            let pubkey = attempt.event.pubkey;
            let session_ref = &self.session;

            // Challenges have their own reader, so this attempt watches for one
            // alongside its own acknowledgement instead of reading everything
            // the connection carries and guessing which parts are its own.
            let challenges = fava_transport::RelaySessionExt::challenges(&session);

            let result = tokio::time::timeout(attempt.timeout, async {
                let mut acknowledged =
                    match fava_transport::RelaySessionExt::publish(&session, attempt.event.clone())
                        .await
                    {
                        Ok(handle) => handle,
                        Err(refused) => return Err(format!("{refused:?}")),
                    };
                let mut authed = false;
                loop {
                    let asked = challenges.notified();
                    if let Some(challenge) = challenges.take() {
                        if authed {
                            continue;
                        }
                        let Ok(auth_event) = build_auth_event(pubkey, &relay_url, &challenge)
                        else {
                            return Ok(PublishOutcome::AuthenticationRequired);
                        };
                        let Some((generation, _)) = session_ref.signer(pubkey) else {
                            return Ok(PublishOutcome::AuthenticationRequired);
                        };
                        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
                        let signed = match session_ref
                            .invoke_signer(pubkey, generation, auth_event, cancel_rx)
                        {
                            Some(fut) => {
                                let Ok(event) = fut.await else {
                                    drop(cancel_tx);
                                    return Ok(PublishOutcome::AuthenticationRequired);
                                };
                                event
                            }
                            None => return Ok(PublishOutcome::AuthenticationRequired),
                        };
                        drop(cancel_tx);
                        if let Err(refused) =
                            fava_transport::RelaySessionExt::answer(&session, signed).await
                        {
                            return Err(format!("AUTH not handed off: {refused:?}"));
                        }
                        // The relay discarded the first copy when it demanded
                        // authentication, so the event is sent again on its own
                        // fresh handle.
                        acknowledged = match fava_transport::RelaySessionExt::publish(
                            &session,
                            attempt.event.clone(),
                        )
                        .await
                        {
                            Ok(handle) => handle,
                            Err(refused) => return Err(format!("resend: {refused:?}")),
                        };
                        authed = true;
                        continue;
                    }
                    tokio::select! {
                        settlement = acknowledged.settled() => {
                            return Ok(match settlement {
                                Settlement::Accepted { message } => PublishOutcome::Acknowledged {
                                    message: message.as_str().to_owned(),
                                },
                                Settlement::Rejected { message } => PublishOutcome::Rejected {
                                    message: message.as_str().to_owned(),
                                },
                                Settlement::Ended(_) => return Err(
                                    "connection ended before the relay answered".to_owned(),
                                ),
                            });
                        }
                        () = asked => {}
                    }
                }
            })
            .await;

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
    EventBuilder::new(Kind::from_u16(22242))
        .tag(relay_tag)
        .tag(challenge_tag)
        .by(pubkey)
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
            // The acknowledgement for this event arrives on this event's own
            // handle. Nothing else on the connection can settle it, so the
            // attempt is bounded by its own deadline and by session liveness
            // and by nothing else.
            let mut acknowledged =
                match fava_transport::RelaySessionExt::publish(&session, attempt.event.clone())
                    .await
                {
                    Ok(handle) => handle,
                    Err(HandoffOutcome::NotHandedOff { reason, .. }) => {
                        let _ = lease.release().await;
                        return PublishOutcome::NotHandedOff {
                            reason: format!("{reason:?}"),
                        };
                    }
                    Err(refused) => {
                        let _ = lease.release().await;
                        return PublishOutcome::OutcomeUnknown {
                            reason: format!("{refused:?}"),
                        };
                    }
                };

            let settled = tokio::time::timeout(attempt.timeout, acknowledged.settled()).await;
            // Release this attempt's hold. The session closes only when the
            // last holder lets go; other observations and publications share it.
            let _ = lease.release().await;
            match settled {
                Ok(Settlement::Accepted { message }) => PublishOutcome::Acknowledged {
                    message: message.as_str().to_owned(),
                },
                Ok(Settlement::Rejected { message }) => PublishOutcome::Rejected {
                    message: message.as_str().to_owned(),
                },
                Ok(Settlement::Ended(ended)) => PublishOutcome::OutcomeUnknown {
                    reason: match ended {
                        SessionEnded::Disconnected { detail } => {
                            format!(
                                "connection ended before the relay answered: {}",
                                detail.as_str()
                            )
                        }
                        SessionEnded::ReconnectExhausted { attempts, detail } => format!(
                            "reconnect budget of {attempts} exhausted before the relay answered: {}",
                            detail.as_str()
                        ),
                    },
                },
                Err(_) => PublishOutcome::OutcomeUnknown {
                    reason: "publication deadline elapsed after handoff".to_owned(),
                },
            }
        })
    }
}
