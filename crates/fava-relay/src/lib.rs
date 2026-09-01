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

/// What one piece of work needs of a relay connection.
///
/// Work states this rather than naming a connection, because more than one
/// connection can satisfy it and which one does is not the work's business.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub enum Authority {
    /// The relay must not know who is asking.
    Unauthenticated,
    /// The relay must have accepted this exact account.
    As(PublicKey),
}

/// Whether a relay connection is reachable.
///
/// Independent of [`Authentication`]: a connection can be authenticated and
/// then drop, and it can be connected without the relay ever asking who it is.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Connectivity {
    /// No socket, and none being opened. Carries why the last one ended, so a
    /// holder that owns no subscription still learns the reason.
    Disconnected {
        /// Exact scoped reason, or empty before a socket ever existed.
        detail: BoundedText,
        /// Attempts spent, once no further connection will appear. `None`
        /// while a reconnect may still follow. A budget can be exhausted
        /// having spent none, so the count cannot double as the verdict.
        spent: Option<usize>,
    },
    /// A socket is being opened.
    Connecting,
    /// A socket is live.
    Connected,
}

/// How far NIP-42 authentication has got on one relay connection.
///
/// This belongs to a connection, not to a relay: a replacement connection
/// starts at [`Authentication::None`], because nothing proved to the relay
/// survives the connection that proved it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Authentication {
    /// The relay has not asked, and nothing has been offered.
    None,
    /// The relay asked. Nobody has decided what to do about it.
    Requested {
        /// The relay's challenge, verbatim.
        challenge: String,
    },
    /// An answer is on the wire; the relay has not ruled on it.
    Authenticating {
        /// The account the answer was signed as.
        as_of: PublicKey,
    },
    /// The relay accepted the answer.
    Authenticated {
        /// The account the relay accepted.
        as_of: PublicKey,
    },
    /// The application refused to answer. Distinct from not having decided.
    Declined,
    /// The answer was refused, or could not be given.
    Failed {
        /// The relay's own words, or the exact reason no answer was possible.
        reason: BoundedText,
    },
}

impl Authentication {
    /// Whether a connection in this state can still serve work needing
    /// `authority`.
    ///
    /// The question is reachability, not equality. A connection nobody has
    /// authenticated can still become anyone's, so it serves everything. One
    /// already accepted as an account can never become another's, and can
    /// never become anonymous again — the relay has already been told.
    #[must_use]
    pub fn can_serve(&self, authority: &Authority) -> bool {
        match authority {
            // A connection carries anonymous work only while the relay still
            // has no idea who is holding it. Once asked, an answer may go out
            // before the work does; once refused on our side, it never will.
            Authority::Unauthenticated => matches!(self, Self::None | Self::Declined),
            Authority::As(want) => match self {
                // Nothing offered yet, or asked and not yet answered: this
                // connection can still become theirs.
                Self::None | Self::Requested { .. } => true,
                // Committed to one account, whether or not the relay has ruled.
                Self::Authenticating { as_of } | Self::Authenticated { as_of } => as_of == want,
                // Refused by us, or refused by the relay. Either way it will
                // not authenticate as anyone on this connection.
                Self::Declined | Self::Failed { .. } => false,
            },
        }
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

#[cfg(test)]
mod connection_tests {
    use super::{Authentication, Authority, BoundedText, Connectivity};
    use nostr::key::Keys;

    /// Every pair of state and requirement, decided one way and stated once.
    #[test]
    fn a_connection_serves_exactly_the_work_it_can_still_reach() {
        let alice = Keys::generate().public_key();
        let bob = Keys::generate().public_key();
        let anon = Authority::Unauthenticated;
        let as_alice = Authority::As(alice);
        let as_bob = Authority::As(bob);

        let cases: [(Authentication, &Authority, bool, &str); 13] = [
            (Authentication::None, &anon, true, "nothing offered yet"),
            (Authentication::None, &as_alice, true, "nothing offered yet"),
            (
                Authentication::Requested {
                    challenge: "n".to_owned(),
                },
                &as_alice,
                true,
                "asked, not yet answered",
            ),
            (
                Authentication::Requested {
                    challenge: "n".to_owned(),
                },
                &anon,
                false,
                "the answer may go out first",
            ),
            (
                Authentication::Authenticating { as_of: alice },
                &as_alice,
                true,
                "same account",
            ),
            (
                Authentication::Authenticating { as_of: alice },
                &as_bob,
                false,
                "another account",
            ),
            (
                Authentication::Authenticating { as_of: alice },
                &anon,
                false,
                "already named",
            ),
            (
                Authentication::Authenticated { as_of: alice },
                &as_alice,
                true,
                "same account",
            ),
            (
                Authentication::Authenticated { as_of: alice },
                &as_bob,
                false,
                "another account",
            ),
            (
                Authentication::Authenticated { as_of: alice },
                &anon,
                false,
                "already named",
            ),
            (
                Authentication::Declined,
                &anon,
                true,
                "will never say who it is",
            ),
            (
                Authentication::Declined,
                &as_alice,
                false,
                "refused for this relay",
            ),
            (
                Authentication::Failed {
                    reason: BoundedText::new("no"),
                },
                &anon,
                false,
                "the relay knows who tried",
            ),
        ];

        for (state, authority, expected, why) in cases {
            assert_eq!(
                state.can_serve(authority),
                expected,
                "{state:?} serving {authority:?}: {why}"
            );
        }
    }

    #[test]
    fn connectivity_is_three_states_and_says_nothing_about_authentication() {
        for state in [
            Connectivity::Disconnected {
                detail: BoundedText::new("socket closed"),
                spent: None,
            },
            Connectivity::Connecting,
            Connectivity::Connected,
        ] {
            let _ = format!("{state:?}");
        }
    }
}
