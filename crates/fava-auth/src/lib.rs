//! Generation-scoped NIP-42 relay authentication.
//!
//! Relay access identity is explicit. It is never derived from event
//! authorship, query authors, the current account, or routing. The application
//! supplies an [`AuthenticationPolicy`]; [`Authentication`] correlates one
//! exact challenge, the policy decision, the selected [`Signer`], and the
//! relay's answer into one session-scoped [`AuthenticationOutcome`].

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_state::{RelaySessionKey, Timestamp};
use fava_transport::{HandoffOutcome, RelaySession};
use fava_wire::{ClientMessage, RelayMessage, decode_relay, encode_client};
use fava_write::{EventBuilder, EventId, Kind, PublicKey, Tag, UnsignedEvent};
use thiserror::Error;
use tokio::sync::watch;

/// NIP-42 client authentication event kind.
pub const AUTHENTICATION_KIND: u16 = 22242;

/// Largest relay challenge Fava will answer.
pub const MAX_CHALLENGE_BYTES: usize = 1_024;

/// Largest relay response text retained as authentication evidence.
pub const MAX_MESSAGE_BYTES: usize = 4_096;

/// Longest one complete challenge/response round trip may remain unresolved.
pub const AUTHENTICATION_DEADLINE: Duration = Duration::from_secs(10);

/// Largest number of relay frames read while waiting for the exact answer.
const MAX_INBOUND_FRAMES: usize = 64;

/// One exact relay challenge bound to one transport session generation.
///
/// A challenge never outlives the generation that carried it. Answering a
/// challenge from a retired generation is refused by identity, not by timing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayChallenge {
    session: RelaySessionKey,
    generation: u64,
    challenge: String,
}

impl RelayChallenge {
    /// Accept one bounded relay challenge for an exact session generation.
    ///
    /// # Errors
    ///
    /// Returns [`AuthenticationError::ChallengeTooLarge`] when the relay
    /// exceeds [`MAX_CHALLENGE_BYTES`] and [`AuthenticationError::EmptyChallenge`]
    /// when the relay supplies no challenge at all.
    pub fn new(
        session: RelaySessionKey,
        generation: u64,
        challenge: impl Into<String>,
    ) -> Result<Self, AuthenticationError> {
        let challenge = challenge.into();
        if challenge.is_empty() {
            return Err(AuthenticationError::EmptyChallenge);
        }
        if challenge.len() > MAX_CHALLENGE_BYTES {
            return Err(AuthenticationError::ChallengeTooLarge {
                bytes: challenge.len(),
                maximum: MAX_CHALLENGE_BYTES,
            });
        }
        Ok(Self {
            session,
            generation,
            challenge,
        })
    }

    /// Exact relay and access identity being authenticated.
    #[must_use]
    pub const fn session(&self) -> &RelaySessionKey {
        &self.session
    }

    /// Transport session generation that carried this challenge.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Exact challenge text supplied by the relay.
    #[must_use]
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

/// One application decision for one exact relay challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorizationDecision {
    /// Authenticate this relay access as the named identity.
    Authorize(PublicKey),
    /// Refuse to authenticate this relay access, with an exact reason.
    Decline(String),
}

/// Replaceable application policy deciding relay authentication.
///
/// The policy sees only relay access identity, session generation, and the
/// relay's challenge. It never sees query filters, event authorship, or the
/// signer registry.
pub trait AuthenticationPolicy: Send + Sync {
    /// Decide whether to authenticate this exact challenge.
    fn authorize<'a>(
        &'a self,
        challenge: &'a RelayChallenge,
    ) -> Pin<Box<dyn Future<Output = AuthorizationDecision> + Send + 'a>>;
}

/// Exact session-scoped result of one authentication round trip.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationOutcome {
    /// The relay accepted the signed challenge answer.
    Accepted {
        /// Identity that was authenticated for this relay access.
        identity: PublicKey,
        /// Exact bounded relay message.
        message: String,
    },
    /// The relay rejected the signed challenge answer.
    Refused {
        /// Exact bounded relay message.
        message: String,
    },
    /// The application policy declined to authenticate this relay access.
    Declined {
        /// Exact application reason.
        reason: String,
    },
    /// The round trip could not complete, with an exact scoped reason.
    Failed {
        /// Exact scoped failure reason.
        reason: String,
    },
}

impl AuthenticationOutcome {
    /// Whether relay access is currently authenticated for this generation.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted { .. })
    }
}

/// Refusal of one authentication input before any relay work occurs.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AuthenticationError {
    /// The relay supplied no challenge text.
    #[error("relay AUTH carried an empty challenge")]
    EmptyChallenge,
    /// The relay challenge exceeds the declared bound.
    #[error("relay challenge uses {bytes} bytes but Fava allows {maximum}")]
    ChallengeTooLarge {
        /// Exact challenge size.
        bytes: usize,
        /// Declared maximum challenge size.
        maximum: usize,
    },
    /// No signer is registered for the identity the policy authorized.
    #[error("no signer is registered for the authorized identity")]
    NoSigner,
    /// The authentication event could not be constructed.
    #[error("authentication event construction failed: {0}")]
    Event(String),
}

/// Build the exact NIP-42 kind 22242 authentication event body.
///
/// # Errors
///
/// Returns [`AuthenticationError::Event`] when the bounded event body cannot
/// be constructed for the exact relay and challenge.
pub fn authentication_event(
    identity: PublicKey,
    challenge: &RelayChallenge,
    created_at: Timestamp,
) -> Result<UnsignedEvent, AuthenticationError> {
    let relay = Tag::parse(["relay", challenge.session().relay.as_str()])
        .map_err(|error| AuthenticationError::Event(error.to_string()))?;
    let nonce = Tag::parse(["challenge", challenge.challenge()])
        .map_err(|error| AuthenticationError::Event(error.to_string()))?;
    EventBuilder::new(identity, Kind::from_u16(AUTHENTICATION_KIND))
        .created_at(created_at)
        .tag(relay)
        .tag(nonce)
        .build()
        .map_err(|error| AuthenticationError::Event(error.to_string()))
}

/// One signed challenge answer ready for handoff on its exact generation.
///
/// The caller owns the send and the correlation of the relay's `OK`, so a
/// reader that also carries query traffic never has to drain frames it does
/// not own.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAuthentication {
    /// Identity the application authorized for this relay access.
    pub identity: PublicKey,
    /// Exact NIP-42 `AUTH` frame to hand off.
    pub frame: String,
    /// Event id whose relay `OK` settles this authentication.
    pub answer: EventId,
}

/// Correlates policy, signer, and relay answer for one relay access.
///
/// `Authentication` owns no connection. The caller supplies the exact session
/// whose generation produced the challenge, so a completion belonging to a
/// retired generation cannot affect current work.
pub struct Authentication {
    policy: Arc<dyn AuthenticationPolicy>,
    signers: BTreeMap<PublicKey, Arc<dyn Signer>>,
    deadline: Duration,
}

impl Authentication {
    /// Select one application policy and the signers it may authorize.
    #[must_use]
    pub fn new(
        policy: Arc<dyn AuthenticationPolicy>,
        signers: impl IntoIterator<Item = Arc<dyn Signer>>,
    ) -> Self {
        Self {
            policy,
            signers: signers
                .into_iter()
                .map(|signer| (signer.public_key(), signer))
                .collect(),
            deadline: AUTHENTICATION_DEADLINE,
        }
    }

    /// Replace the bounded round-trip deadline.
    #[must_use]
    pub const fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    /// Answer one exact challenge on the session generation that carried it.
    ///
    /// The whole round trip is bounded by the configured deadline. The outcome
    /// is scoped to this relay access and generation; it never becomes a fact
    /// about another account, relay, query, or write.
    pub async fn answer(
        &self,
        challenge: &RelayChallenge,
        session: &dyn RelaySession,
    ) -> AuthenticationOutcome {
        if session.key() != challenge.session() || session.generation() != challenge.generation() {
            return AuthenticationOutcome::Failed {
                reason: "relay challenge belongs to a retired session generation".to_owned(),
            };
        }
        match tokio::time::timeout(self.deadline, self.round_trip(challenge, session)).await {
            Ok(outcome) => outcome,
            Err(_) => AuthenticationOutcome::Failed {
                reason: format!(
                    "relay authentication did not resolve within {:?}",
                    self.deadline
                ),
            },
        }
    }

    /// Decide, sign, and encode one challenge answer without any relay work.
    ///
    /// # Errors
    ///
    /// Returns the exact declining or failing [`AuthenticationOutcome`] when no
    /// frame may be handed off.
    pub async fn prepare(
        &self,
        challenge: &RelayChallenge,
        session: &dyn RelaySession,
    ) -> Result<PreparedAuthentication, AuthenticationOutcome> {
        if session.key() != challenge.session() || session.generation() != challenge.generation() {
            return Err(AuthenticationOutcome::Failed {
                reason: "relay challenge belongs to a retired session generation".to_owned(),
            });
        }
        let identity = match self.policy.authorize(challenge).await {
            AuthorizationDecision::Authorize(identity) => identity,
            AuthorizationDecision::Decline(reason) => {
                return Err(AuthenticationOutcome::Declined {
                    reason: bounded(reason),
                });
            }
        };
        let Some(signer) = self.signers.get(&identity) else {
            return Err(AuthenticationOutcome::Failed {
                reason: AuthenticationError::NoSigner.to_string(),
            });
        };
        if !matches!(signer.availability(), SignerAvailability::Available) {
            return Err(AuthenticationOutcome::Failed {
                reason: "signer for the authorized identity is unavailable".to_owned(),
            });
        }
        let body =
            authentication_event(identity, challenge, Timestamp::now()).map_err(|error| {
                AuthenticationOutcome::Failed {
                    reason: error.to_string(),
                }
            })?;
        let (_keep_alive, cancel) = watch::channel(false);
        let event = match signer.sign_event(body, cancel).await {
            Ok(event) => event,
            Err(SignerError::Cancelled) => {
                return Err(AuthenticationOutcome::Failed {
                    reason: "authentication signing was cancelled".to_owned(),
                });
            }
            Err(error) => {
                return Err(AuthenticationOutcome::Failed {
                    reason: error.to_string(),
                });
            }
        };
        let answer = event.id;
        let frame = encode_client(&ClientMessage::auth(event)).map_err(|error| {
            AuthenticationOutcome::Failed {
                reason: error.to_string(),
            }
        })?;
        Ok(PreparedAuthentication {
            identity,
            frame,
            answer,
        })
    }

    /// Interpret one relay `OK` that settles a prepared authentication.
    #[must_use]
    pub fn settle(identity: PublicKey, status: bool, message: String) -> AuthenticationOutcome {
        let message = bounded(message);
        if status {
            AuthenticationOutcome::Accepted { identity, message }
        } else {
            AuthenticationOutcome::Refused { message }
        }
    }

    async fn round_trip(
        &self,
        challenge: &RelayChallenge,
        session: &dyn RelaySession,
    ) -> AuthenticationOutcome {
        let prepared = match self.prepare(challenge, session).await {
            Ok(prepared) => prepared,
            Err(outcome) => return outcome,
        };
        match session.send(prepared.frame).await {
            HandoffOutcome::HandedOff => {}
            HandoffOutcome::NotHandedOff { reason } | HandoffOutcome::Ambiguous { reason } => {
                return AuthenticationOutcome::Failed {
                    reason: bounded(reason),
                };
            }
        }
        self.await_answer(prepared.identity, prepared.answer, session)
            .await
    }

    async fn await_answer(
        &self,
        identity: PublicKey,
        answer: EventId,
        session: &dyn RelaySession,
    ) -> AuthenticationOutcome {
        for _ in 0..MAX_INBOUND_FRAMES {
            let frame = match session.next_message().await {
                Ok(frame) => frame,
                Err(error) => {
                    return AuthenticationOutcome::Failed {
                        reason: error.to_string(),
                    };
                }
            };
            let Ok(message) = decode_relay(&frame) else {
                continue;
            };
            if let RelayMessage::Ok {
                event_id,
                status,
                message,
            } = message
                && event_id == answer
            {
                return Self::settle(identity, status, message.into_owned());
            }
        }
        AuthenticationOutcome::Failed {
            reason: format!("no matching relay OK within {MAX_INBOUND_FRAMES} frames"),
        }
    }
}

fn bounded(mut text: String) -> String {
    if text.len() > MAX_MESSAGE_BYTES {
        text.truncate(
            (0..=MAX_MESSAGE_BYTES)
                .rev()
                .find(|index| text.is_char_boundary(*index))
                .unwrap_or(0),
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use fava_state::{RelayAccess, RelayUrl};

    use super::*;

    fn session(relay: &str, access: &str) -> RelaySessionKey {
        RelaySessionKey::new(
            RelayUrl::parse(relay).expect("test relay parses"),
            RelayAccess::named(access),
        )
    }

    #[test]
    fn empty_and_oversized_challenges_are_refused_before_any_relay_work() {
        assert_eq!(
            RelayChallenge::new(session("ws://127.0.0.1:1", "a"), 1, ""),
            Err(AuthenticationError::EmptyChallenge)
        );
        let oversized = "x".repeat(MAX_CHALLENGE_BYTES + 1);
        assert_eq!(
            RelayChallenge::new(session("ws://127.0.0.1:1", "a"), 1, oversized),
            Err(AuthenticationError::ChallengeTooLarge {
                bytes: MAX_CHALLENGE_BYTES + 1,
                maximum: MAX_CHALLENGE_BYTES,
            })
        );
    }

    #[test]
    fn the_authentication_event_names_the_exact_relay_and_challenge() {
        let keys = nostr::key::Keys::generate();
        let challenge = RelayChallenge::new(session("ws://127.0.0.1:1", "a"), 7, "nonce")
            .expect("bounded challenge");
        let event = authentication_event(keys.public_key(), &challenge, Timestamp::from(1_000))
            .expect("event builds");
        assert_eq!(event.kind, Kind::from_u16(AUTHENTICATION_KIND));
        assert_eq!(event.pubkey, keys.public_key());
        let tags: Vec<Vec<String>> = event.tags.iter().map(|tag| tag.clone().to_vec()).collect();
        assert!(tags.contains(&vec!["relay".to_owned(), "ws://127.0.0.1:1".to_owned()]));
        assert!(tags.contains(&vec!["challenge".to_owned(), "nonce".to_owned()]));
    }

    #[test]
    fn relay_text_is_bounded_at_a_character_boundary() {
        let text = "é".repeat(MAX_MESSAGE_BYTES);
        let bounded = bounded(text);
        assert!(bounded.len() <= MAX_MESSAGE_BYTES);
        assert!(bounded.chars().all(|character| character == 'é'));
    }
}
