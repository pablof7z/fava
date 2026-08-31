//! The relay-issued challenge, accepted under an explicit bound.

use thiserror::Error;

/// Exact relay-issued authentication challenge.
///
/// A challenge arrives as unbounded relay-supplied text and must be echoed
/// back byte-exact in the kind-22242 response, so it is refused rather than
/// truncated: a shortened challenge would never match.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Challenge {
    text: String,
}

impl Challenge {
    /// Maximum accepted challenge size.
    ///
    /// Relay challenges are short opaque nonces; strfry uses 22 bytes. This
    /// bound accepts every real one while keeping a hostile relay from
    /// spending our memory.
    pub const MAX_BYTES: usize = 512;

    /// Accept one relay challenge.
    ///
    /// # Errors
    ///
    /// Returns [`ChallengeError`] for empty text or text above
    /// [`Challenge::MAX_BYTES`].
    pub fn new(text: &str) -> Result<Self, ChallengeError> {
        if text.is_empty() {
            return Err(ChallengeError::Empty);
        }
        if text.len() > Self::MAX_BYTES {
            return Err(ChallengeError::TooLarge {
                bytes: text.len(),
                maximum: Self::MAX_BYTES,
            });
        }
        Ok(Self {
            text: text.to_owned(),
        })
    }

    /// Exact challenge text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

/// Why a relay challenge was refused.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ChallengeError {
    /// The relay sent no challenge text.
    #[error("relay challenge was empty")]
    Empty,
    /// The challenge exceeded the accepted bound.
    #[error("relay challenge is {bytes} bytes, maximum {maximum}")]
    TooLarge {
        /// Exact size the relay sent.
        bytes: usize,
        /// Exact accepted maximum.
        maximum: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::{Challenge, ChallengeError};

    #[test]
    fn empty_challenge_is_refused() {
        assert_eq!(Challenge::new(""), Err(ChallengeError::Empty));
    }

    #[test]
    fn challenge_at_the_bound_is_accepted_whole() {
        let text = "c".repeat(Challenge::MAX_BYTES);
        let challenge = Challenge::new(&text).expect("a challenge at the bound is accepted");
        assert_eq!(challenge.as_str(), text, "the challenge is never shortened");
    }

    #[test]
    fn challenge_above_the_bound_is_refused_not_truncated() {
        let text = "c".repeat(Challenge::MAX_BYTES + 1);
        assert_eq!(
            Challenge::new(&text),
            Err(ChallengeError::TooLarge {
                bytes: Challenge::MAX_BYTES + 1,
                maximum: Challenge::MAX_BYTES,
            })
        );
    }
}
