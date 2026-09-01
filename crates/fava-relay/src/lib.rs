//! Neutral immutable logical relay/access identity and its bounded facts.

use nostr::key::PublicKey;
use nostr::types::RelayUrl;

/// Exact application-selected access authority for relay work.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum RelayAccess {
    /// Public unauthenticated relay authority.
    Public,
    /// Relay authority authenticated as one exact protocol public key.
    Authenticated(PublicKey),
}

/// Stable logical relay and access identity shared across owners.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionKey {
    /// Exact normalized relay URL.
    pub relay: RelayUrl,
    /// Exact access authority.
    pub access: RelayAccess,
}

/// Relay- or OS-supplied text retained under a Fava-owned byte bound.
///
/// Authority: GOALS:1439 (OPS-004, "frame and message sizes"), GOALS:1111
/// (RELAY-008, verbatim evidence). Truncation is recorded, never silent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedText {
    text: String,
    truncated_bytes: usize,
}

impl BoundedText {
    /// Maximum retained bytes. Long enough for every NIP-01 `CLOSED`/`NOTICE`
    /// prefix that carries a machine-readable reason word, short enough that
    /// 256 retained facts per diagnostics category is a real memory bound.
    pub const MAX_BYTES: usize = 512;

    /// Retain at most `MAX_BYTES`, recording how many were dropped.
    #[must_use]
    pub fn new(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        let mut end = text.len().min(Self::MAX_BYTES);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            text: text[..end].to_owned(),
            truncated_bytes: text.len() - end,
        }
    }

    /// Retained text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Bytes dropped by the bound. Non-zero means the fact is a shortfall.
    #[must_use]
    pub const fn truncated_bytes(&self) -> usize {
        self.truncated_bytes
    }
}

/// How far NIP-42 authentication has got on one relay session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationState {
    /// A challenge arrived; no policy decision yet.
    ChallengeReceived,
    /// The application's policy declined to authenticate.
    Declined,
    /// AUTH was sent; no relay verdict yet.
    Attempted,
    /// A person owns the answer; nothing is signed until they give it.
    AwaitingAnswer,
    /// The relay accepted AUTH and the session is authenticated.
    Accepted,
    /// The relay accepted AUTH but still refuses the request.
    AcceptedButStillRefused {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
    /// The relay rejected AUTH.
    Rejected {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
    /// Authentication could not be attempted or completed.
    Failed {
        /// Exact bounded reason: no attached signer, a refused challenge, or
        /// the per-generation attempt bound reached.
        reason: BoundedText,
    },
}

/// What one component determined about authenticating a relay session.
///
/// Exactly one owner holds NIP-42 challenge state. Everyone else reads its
/// conclusion through this contract rather than deriving one of their own from
/// the wire, so a relay's demand has one source and one answer.
pub trait AuthenticationOutcomes: Send + Sync {
    /// How far authentication has got on one relay session, or `None` when the
    /// owner has nothing to say about it.
    fn state(&self, key: &RelaySessionKey) -> Option<AuthenticationState>;
}

#[cfg(test)]
mod tests {
    use super::{AuthenticationState, BoundedText};

    #[test]
    fn every_state_is_constructible_and_bounds_its_text() {
        let long = "x".repeat(BoundedText::MAX_BYTES * 2);
        let states = [
            AuthenticationState::ChallengeReceived,
            AuthenticationState::Declined,
            AuthenticationState::Attempted,
            AuthenticationState::AwaitingAnswer,
            AuthenticationState::Accepted,
            AuthenticationState::AcceptedButStillRefused {
                message: BoundedText::new(&long),
            },
            AuthenticationState::Rejected {
                message: BoundedText::new(&long),
            },
            AuthenticationState::Failed {
                reason: BoundedText::new(&long),
            },
        ];
        for state in &states {
            let text = match state {
                AuthenticationState::AcceptedButStillRefused { message }
                | AuthenticationState::Rejected { message } => Some(message),
                AuthenticationState::Failed { reason } => Some(reason),
                _ => None,
            };
            if let Some(text) = text {
                assert_eq!(text.as_str().len(), BoundedText::MAX_BYTES);
                assert_eq!(text.truncated_bytes(), BoundedText::MAX_BYTES);
            }
        }
        assert_eq!(states.len(), 8, "every variant is covered");
    }
}
