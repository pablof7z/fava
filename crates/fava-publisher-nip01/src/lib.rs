//! Standard NIP-01 `EVENT`/`OK` publisher.
//!
//! Authenticating a relay connection is not a publisher's business: NIP-42
//! authenticates the connection, and one component owns that lifecycle. A
//! publisher that meets a relay demanding authentication does not sign or
//! send anything itself. When the write's own authority names an account the
//! connection can still become, it waits for that connection to say so —
//! reading it, never asking anyone — and sends again. Anonymous work has no
//! account to wait for, so it says so and stops, exactly as before.

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_relay::{Authority, Connectivity, Progress};
use fava_transport::{
    Connection, HandoffOutcome, OpenRelaySession, RelaySession, RelaySessionExt, SessionEnded,
    Settlement, Transport, TransportBounds, TransportDeadlines,
};
use nostr::key::PublicKey;
use nostr::types::RelayUrl;

/// Inbound queue depth this publisher asks the transport for.
const INBOUND_FRAMES: usize = 64;

/// The machine-readable prefix NIP-42 gives an `OK` refusal that authentication
/// would have satisfied.
const AUTH_REQUIRED: &str = "auth-required:";

/// Deadlines and bounds this publisher hands the transport for one attempt.
/// The attempt's own timeout is the only Fava-owned duration it knows.
fn open_request(
    relay: &RelayUrl,
    authority: &fava_relay::Authority,
    timeout: std::time::Duration,
) -> OpenRelaySession {
    let frames = |count: usize| NonZeroUsize::new(count).expect("constant is non-zero");
    OpenRelaySession {
        relay: relay.clone(),
        authority: *authority,
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
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            let lease = match transport
                .acquire_session(open_request(
                    &attempt.session,
                    &attempt.authority,
                    attempt.timeout,
                ))
                .await
            {
                Ok(lease) => lease,
                Err(error) => {
                    return PublishOutcome::NotHandedOff {
                        reason: error.to_string(),
                    };
                }
            };
            // Held for the whole attempt, including any wait: releasing it
            // early can close the connection when nothing else holds it, and
            // a closed connection is never authenticated.
            let session = Arc::clone(lease.session());
            let outcome = send_until_settled(&session, &attempt).await;
            let _ = lease.release().await;
            outcome
        })
    }
}

/// Send `attempt.event` on `session`, waiting out and retrying every
/// `auth-required:` refusal the write's own authority can still resolve,
/// until a genuine relay verdict or connection failure settles it.
async fn send_until_settled(
    session: &Arc<dyn RelaySession>,
    attempt: &PublishAttempt,
) -> PublishOutcome {
    loop {
        // The acknowledgement for this event arrives on this event's own
        // handle. Nothing else on the connection can settle it, so each round
        // is bounded by its own deadline and by session liveness and nothing
        // else.
        let mut acknowledged = match session.publish(attempt.event.clone()).await {
            Ok(handle) => handle,
            Err(HandoffOutcome::NotHandedOff { reason, .. }) => {
                return PublishOutcome::NotHandedOff {
                    reason: format!("{reason:?}"),
                };
            }
            Err(refused) => {
                return PublishOutcome::OutcomeUnknown {
                    reason: format!("{refused:?}"),
                };
            }
        };

        let settled = tokio::time::timeout(attempt.timeout, acknowledged.settled()).await;
        match settled {
            Ok(Settlement::Accepted { message }) => {
                return PublishOutcome::Acknowledged {
                    message: message.as_str().to_owned(),
                };
            }
            Ok(Settlement::Rejected { message }) => {
                let message = message.as_str().to_owned();
                // NIP-01 makes an `OK` refusal's prefix machine-readable and
                // NIP-42 defines this one. Reading it is decoding the wire,
                // which is this publisher's whole job.
                if !message.starts_with(AUTH_REQUIRED) {
                    return PublishOutcome::Rejected { message };
                }
                let Authority::As(account) = attempt.authority else {
                    // Anonymous work has no account to wait for. Only
                    // acceptance ever moves `established`, and acceptance is
                    // exactly what this write must never carry, so there is
                    // nothing here for waiting to resolve.
                    return PublishOutcome::AuthenticationRequired { message };
                };
                match wait_for_authentication(session, account).await {
                    Ok(()) => {}
                    Err(reason) => {
                        return PublishOutcome::AuthenticationRequired {
                            message: format!(
                                "{message}; authentication was required and did not happen: {reason}"
                            ),
                        };
                    }
                }
            }
            Ok(Settlement::Ended(ended)) => {
                return PublishOutcome::OutcomeUnknown {
                    reason: describe_ended(ended),
                };
            }
            Err(_) => {
                return PublishOutcome::OutcomeUnknown {
                    reason: "publication deadline elapsed after handoff".to_owned(),
                };
            }
        }
    }
}

/// Wait for `session`'s connection to become authenticated as `account` — so
/// the write can be sent again — or to reach a state from which that can no
/// longer happen.
///
/// No timeout, no attempt ceiling: reaching this point means the relay has
/// already been asked, and answering it is the application's decision to
/// make in its own time. Every caller of a publisher already imposes its own
/// bound; none is imposed here.
async fn wait_for_authentication(
    session: &Arc<dyn RelaySession>,
    account: PublicKey,
) -> Result<(), String> {
    let mut connection = session.connection();
    let settled = connection
        .wait_for(|current| arrived(current, account) || blocked(current))
        .await;
    match settled {
        Err(_) => Err("the connection ended while authentication was outstanding".to_owned()),
        Ok(current) if arrived(&current, account) => Ok(()),
        Ok(current) => Err(describe_block(&current)),
    }
}

/// Whether this connection has been accepted as `account` and can carry the
/// write again.
fn arrived(connection: &Connection, account: PublicKey) -> bool {
    matches!(connection.connectivity, Connectivity::Connected)
        && connection.authentication.established == Some(account)
}

/// Whether this connection has reached a state authentication can no longer
/// leave: refused outright, declined by the application, unanswerable, or
/// gone for good.
fn blocked(connection: &Connection) -> bool {
    matches!(
        connection.authentication.progress,
        Progress::Declined | Progress::Refused { .. } | Progress::Unanswerable { .. }
    ) || matches!(
        connection.connectivity,
        Connectivity::Disconnected { spent: Some(_), .. }
    )
}

/// Name why `connection` is blocked, for a failed write's evidence.
fn describe_block(connection: &Connection) -> String {
    match &connection.authentication.progress {
        Progress::Declined => "the application declined to authenticate".to_owned(),
        Progress::Refused { reason } => {
            format!("the relay refused authentication: {}", reason.as_str())
        }
        Progress::Unanswerable { reason } => {
            format!("authentication could not be answered: {}", reason.as_str())
        }
        Progress::Idle | Progress::Requested { .. } | Progress::Answering { .. } => {
            match &connection.connectivity {
                Connectivity::Disconnected {
                    detail,
                    spent: Some(attempts),
                } => format!(
                    "the reconnect budget of {attempts} was exhausted before authentication completed: {}",
                    detail.as_str()
                ),
                _ => "authentication did not complete".to_owned(),
            }
        }
    }
}

/// Describe a session ending before the relay's verdict arrived, in terms an
/// application can report without decoding transport internals itself.
fn describe_ended(ended: SessionEnded) -> String {
    match ended {
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
    }
}

#[cfg(test)]
mod tests {
    use fava_relay::{Authentication, BoundedText};
    use fava_transport::{RelayConnection, RelaySessionIdentity};
    use nostr::key::Keys;

    use super::{Connection, Connectivity, Progress, arrived, blocked, describe_block};

    fn account() -> nostr::key::PublicKey {
        Keys::generate().public_key()
    }

    fn connection(connectivity: Connectivity, authentication: Authentication) -> Connection {
        Connection {
            identity: RelaySessionIdentity {
                relay: nostr::types::RelayUrl::parse("ws://127.0.0.1:1/").expect("relay url"),
                connection: RelayConnection::new(1).expect("non-zero"),
            },
            connectivity,
            authentication,
        }
    }

    /// A connection accepted as `account` has arrived, and is not blocked —
    /// otherwise the wait could never succeed once the relay had already said
    /// yes.
    #[test]
    fn a_connection_accepted_as_the_account_has_arrived() {
        let account = account();
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: Some(account),
                progress: Progress::Idle,
            },
        );
        assert!(arrived(&state, account));
        assert!(!blocked(&state));
    }

    /// A connection accepted as someone else has not arrived for this
    /// account, and is not itself blocked: the relay learned nothing about
    /// this account, so a later challenge could still answer it.
    #[test]
    fn a_connection_accepted_as_someone_else_has_not_arrived() {
        let wanted = account();
        let other = account();
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: Some(other),
                progress: Progress::Idle,
            },
        );
        assert!(!arrived(&state, wanted));
    }

    /// A person still deciding — the connection's own state while nobody has
    /// answered its challenge yet — blocks nothing. This is the case a write
    /// waiting on a person must not be mistaken for a refusal: 6b.8.
    #[test]
    fn a_connection_awaiting_a_person_is_not_blocked() {
        let account = account();
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: None,
                progress: Progress::Requested {
                    challenge: "prove it".to_owned(),
                },
            },
        );
        assert!(!arrived(&state, account));
        assert!(!blocked(&state));
    }

    /// The application declining to authenticate ends the wait: nobody is
    /// going to answer this challenge.
    #[test]
    fn a_declined_challenge_is_blocked() {
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: None,
                progress: Progress::Declined,
            },
        );
        assert!(blocked(&state));
        assert!(describe_block(&state).contains("declined"));
    }

    /// A relay refusing the answer, in its own words, ends the wait and
    /// carries that text forward.
    #[test]
    fn a_refused_challenge_is_blocked_and_names_the_relay() {
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: None,
                progress: Progress::Refused {
                    reason: BoundedText::new("get lost"),
                },
            },
        );
        assert!(blocked(&state));
        assert!(describe_block(&state).contains("get lost"));
    }

    /// An unanswerable challenge — no signer, no account, one too long to
    /// hold — ends the wait exactly as a refusal does.
    #[test]
    fn an_unanswerable_challenge_is_blocked() {
        let state = connection(
            Connectivity::Connected,
            Authentication {
                established: None,
                progress: Progress::Unanswerable {
                    reason: BoundedText::new("no signer attached"),
                },
            },
        );
        assert!(blocked(&state));
        assert!(describe_block(&state).contains("no signer attached"));
    }

    /// A dropped connection that may still reconnect is not blocked: a
    /// reconnect may still bring the account with it.
    #[test]
    fn a_connection_that_may_still_reconnect_is_not_blocked() {
        let account = account();
        let state = connection(
            Connectivity::Disconnected {
                detail: BoundedText::new(""),
                spent: None,
            },
            Authentication::unoffered(),
        );
        assert!(!arrived(&state, account));
        assert!(!blocked(&state));
    }

    /// An exhausted reconnect budget ends the wait: no further connection
    /// will appear to carry the account.
    #[test]
    fn an_exhausted_reconnect_budget_is_blocked() {
        let state = connection(
            Connectivity::Disconnected {
                detail: BoundedText::new("gave up"),
                spent: Some(3),
            },
            Authentication::unoffered(),
        );
        assert!(blocked(&state));
        assert!(describe_block(&state).contains('3'));
    }
}
