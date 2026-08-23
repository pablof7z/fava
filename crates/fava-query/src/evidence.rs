//! Vocabulary of per-relay, per-branch query evidence.

use fava_state::Timestamp;

/// Exact state of one relay's contribution to one query.
///
/// Every variant is distinct at the type level. No two of them may be produced
/// by the same underlying fact (GOALS:422).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelaySourceState {
    /// Routing named this relay; no session has been acquired yet.
    Planned,
    /// A session is being established.
    Connecting,
    /// A session is live and the request is installed; no EOSE yet.
    Open {
        /// When the request was installed.
        requested_at: Timestamp,
    },
    /// The relay sent EOSE for the exact current request identity.
    StoredEventsComplete {
        /// When the EOSE arrived.
        at: Timestamp,
    },
    /// The relay sent CLOSED for the request.
    Refused {
        /// Verbatim, bounded relay text (GOALS:1111, RELAY-008).
        message: BoundedText,
        /// When it arrived.
        at: Timestamp,
    },
    /// The relay demands NIP-42 authentication for this request.
    AuthenticationRequired {
        /// Current authentication state for this session.
        state: AuthenticationState,
        /// When the requirement was learned.
        at: Timestamp,
    },
    /// A Fava-owned deadline expired.
    TimedOut {
        /// Which deadline.
        deadline: RelayDeadline,
        /// The duration that expired, in milliseconds.
        after_ms: u64,
    },
    /// The session dropped and reconnect is in progress.
    Disconnected {
        /// Bounded reason.
        detail: BoundedText,
    },
    /// Reconnect budget is exhausted; this relay will not return by itself.
    Unreachable {
        /// Attempts actually made.
        attempts: usize,
        /// Bounded reason of the final attempt.
        detail: BoundedText,
    },
    /// Fava withdrew this relay's demand (route withdrawal or query close).
    Withdrawn {
        /// Why.
        reason: RelayWithdrawal,
    },
}

/// Which Fava-owned deadline expired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayDeadline {
    /// Session establishment.
    Establish,
    /// Frame write.
    Write,
    /// Inbound silence.
    Idle,
    /// Close handshake.
    Close,
}

/// NIP-42 state for one relay session, as seen by a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationState {
    /// A challenge arrived; no policy decision yet.
    ChallengeReceived,
    /// The application's policy declined to authenticate.
    Declined,
    /// AUTH was sent; no relay verdict yet.
    Attempted,
    /// The relay accepted AUTH but still refuses the request.
    AcceptedButStillRefused,
    /// The relay rejected AUTH.
    Rejected {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
}

/// Why Fava stopped asking one relay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayWithdrawal {
    /// No router still contributes this destination (GOALS:479).
    RouteWithdrawn,
    /// The observation closed.
    ObservationClosed,
    /// The engine is shutting down.
    Shutdown,
}

/// Owner-supplied text retained under a Fava-owned byte bound.
///
/// Identical semantics to `fava_transport::BoundedReason`; duplicated here so
/// `fava-query` keeps zero contract dependencies. `MAX_BYTES` is 512 in both.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedText {
    text: String,
    truncated_bytes: usize,
}

impl BoundedText {
    /// Maximum retained bytes.
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

    /// Bytes dropped by the bound.
    #[must_use]
    pub const fn truncated_bytes(&self) -> usize {
        self.truncated_bytes
    }
}
