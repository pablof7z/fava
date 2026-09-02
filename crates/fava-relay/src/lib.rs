//! Neutral immutable logical relay identity, what work requires of a
//! connection, and the bounded facts a connection carries.

use nostr::key::PublicKey;

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
/// Two questions, kept apart because they have different answers. What the
/// relay has accepted here is one; how the challenge in front of us is going
/// is the other. Held together they force a reader to decide, for every
/// outcome, whether the relay learned anything from it — and the answer is
/// only ever visible in `established`.
///
/// This belongs to a connection, not to a relay: a replacement starts having
/// proved nothing, because nothing proved to the relay survives the connection
/// that proved it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Authentication {
    /// The account the relay has accepted on this connection.
    ///
    /// Set only by acceptance, and never cleared while the connection lives.
    /// A relay that has greeted someone by name does not forget, so neither
    /// does this.
    pub established: Option<PublicKey>,
    /// How the challenge in front of us is going.
    pub progress: Progress,
}

/// How the challenge in front of one connection is going.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Progress {
    /// The relay has not asked, or its last question is finished with.
    #[default]
    Idle,
    /// The relay asked. Nobody has decided what to do about it.
    Requested {
        /// The relay's challenge, verbatim.
        challenge: String,
    },
    /// An answer is on the wire; the relay has not ruled on it.
    Answering {
        /// The account the answer was signed as.
        as_of: PublicKey,
    },
    /// The application refused to answer this one. Distinct from not having
    /// decided, and distinct from being unable.
    Declined,
    /// The relay refused the answer, in its own words.
    Refused {
        /// Verbatim, bounded relay text.
        reason: BoundedText,
    },
    /// No answer could be produced: no account, no signer, a challenge too
    /// long to hold. The relay was told nothing, so it learned nothing.
    Unanswerable {
        /// Exact bounded reason.
        reason: BoundedText,
    },
}

impl Authentication {
    /// A connection that has offered nothing and been asked nothing.
    #[must_use]
    pub const fn unoffered() -> Self {
        Self {
            established: None,
            progress: Progress::Idle,
        }
    }

    /// Whether a connection in this state can still serve work needing
    /// `authority`.
    ///
    /// Anonymous work asks one question: does the relay know who is holding
    /// this connection. Only acceptance makes it, so failing to answer and
    /// being refused both leave a connection anonymous — the relay learned
    /// nothing either way.
    ///
    /// Work naming an account asks whether this connection can still become
    /// theirs. One already accepted as someone else cannot; one with an answer
    /// in flight for someone else cannot until that settles. Everything else
    /// can, including a connection whose last challenge was declined or
    /// refused, because the next challenge is a new question.
    #[must_use]
    pub fn can_serve(&self, authority: &Authority) -> bool {
        match (authority, self.established) {
            (Authority::Unauthenticated, established) => established.is_none(),
            (Authority::As(want), Some(established)) => established == *want,
            (Authority::As(want), None) => !matches!(
                self.progress,
                Progress::Answering { as_of } if as_of != *want
            ),
        }
    }
}

#[cfg(test)]
mod connection_tests {
    fn key(seed: u8) -> nostr::key::PublicKey {
        let mut bytes = [0_u8; 32];
        bytes[31] = seed;
        nostr::key::Keys::new(nostr::key::SecretKey::from_slice(&bytes).expect("secret key"))
            .public_key()
    }

    use super::{Authentication, Authority, BoundedText, Connectivity, Progress};

    /// What the relay knows decides anonymous work; nothing else does.
    #[test]
    fn only_acceptance_stops_a_connection_carrying_anonymous_work() {
        let alice = key(1);
        let anon = Authority::Unauthenticated;

        // The relay learned nothing from any of these, so it does not know
        // who is holding the connection, so anonymous work may ride it.
        for progress in [
            Progress::Idle,
            Progress::Declined,
            Progress::Refused {
                reason: BoundedText::new("restricted: not on the list"),
            },
            Progress::Unanswerable {
                reason: BoundedText::new("no signer is attached for this account"),
            },
        ] {
            let connection = Authentication {
                established: None,
                progress: progress.clone(),
            };
            assert!(
                connection.can_serve(&anon),
                "the relay was told nothing, so {progress:?} is still anonymous"
            );
        }

        // Once it has been told, it has been told, whatever happens next.
        for progress in [
            Progress::Idle,
            Progress::Requested {
                challenge: "n".to_owned(),
            },
            Progress::Declined,
        ] {
            let connection = Authentication {
                established: Some(alice),
                progress: progress.clone(),
            };
            assert!(
                !connection.can_serve(&anon),
                "the relay knows this connection as alice; {progress:?} does not undo that"
            );
        }
    }

    /// Work naming an account asks whether this connection can still become
    /// theirs.
    #[test]
    fn a_connection_serves_the_account_it_can_still_reach() {
        let alice = key(1);
        let bob = key(2);

        let nothing_yet = Authentication::default();
        assert!(nothing_yet.can_serve(&Authority::As(alice)));
        assert!(nothing_yet.can_serve(&Authority::As(bob)));

        let answering_alice = Authentication {
            established: None,
            progress: Progress::Answering { as_of: alice },
        };
        assert!(answering_alice.can_serve(&Authority::As(alice)));
        assert!(
            !answering_alice.can_serve(&Authority::As(bob)),
            "an answer already on the wire names who this connection is becoming"
        );

        let is_alice = Authentication {
            established: Some(alice),
            progress: Progress::Idle,
        };
        assert!(is_alice.can_serve(&Authority::As(alice)));
        assert!(!is_alice.can_serve(&Authority::As(bob)));

        // The next challenge is a new question, so a refused one does not
        // close the connection to the account that was refused.
        let refused = Authentication {
            established: None,
            progress: Progress::Refused {
                reason: BoundedText::new("error: try again"),
            },
        };
        assert!(refused.can_serve(&Authority::As(alice)));
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
