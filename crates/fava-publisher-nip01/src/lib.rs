//! Standard one-attempt NIP-01 `EVENT`/`OK` publisher.
//!
//! Authenticating a relay connection is not a publisher's business: NIP-42
//! authenticates the connection, and one component owns that lifecycle. A
//! publisher that meets a relay demanding authentication says so and stops.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_relay::RelaySessionKey;
use fava_transport::{
    HandoffOutcome, OpenRelaySession, SessionEnded, Settlement, Transport, TransportBounds,
    TransportDeadlines,
};

/// Inbound queue depth this publisher asks the transport for.
const INBOUND_FRAMES: usize = 64;

/// The machine-readable prefix NIP-42 gives an `OK` refusal that authentication
/// would have satisfied.
const AUTH_REQUIRED: &str = "auth-required:";

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
                Ok(Settlement::Rejected { message }) => {
                    let message = message.as_str().to_owned();
                    // NIP-01 makes an `OK` refusal's prefix machine-readable and
                    // NIP-42 defines this one. Reading it is decoding the wire,
                    // which is this publisher's whole job; deciding what to do
                    // about the demand belongs to whoever owns authentication.
                    if message.starts_with(AUTH_REQUIRED) {
                        PublishOutcome::AuthenticationRequired { message }
                    } else {
                        PublishOutcome::Rejected { message }
                    }
                }
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
