//! Items delivered to one inbound consumer of one relay session.

use nostr::types::Timestamp;

use crate::{RelaySessionIdentity, TransportFailure};

/// One item delivered to one inbound consumer.
///
/// The stream carries session-lifecycle transitions alongside frames.
/// `GOALS:483-489` (QUERY-015) and `ARCH:2092` require a generation change to
/// reach the exact affected observations, and the message stream is the only
/// ordered channel between a session and its holders — a side-channel would
/// reintroduce the ordering ambiguity the generation exists to remove.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayInbound {
    /// One complete relay frame under an exact generation.
    Frame {
        /// Session and generation that produced these bytes.
        identity: RelaySessionIdentity,
        /// Raw frame bytes. Decoding belongs to `fava-wire`.
        frame: Vec<u8>,
        /// Local admission time.
        received_at: Timestamp,
    },
    /// The session disconnected; a reconnect may follow.
    Disconnected {
        /// Generation that ended.
        identity: RelaySessionIdentity,
        /// Exact scoped reason.
        reason: TransportFailure,
    },
    /// A new generation is live. Every holder MUST replay its active demand.
    Reconnected {
        /// Generation that ended.
        previous: RelaySessionIdentity,
        /// Generation now current.
        identity: RelaySessionIdentity,
    },
    /// Reconnect budget is exhausted; no further generation will appear.
    ReconnectExhausted {
        /// Last generation attempted.
        identity: RelaySessionIdentity,
        /// Number of attempts actually made.
        attempts: usize,
        /// Exact reason of the final attempt.
        reason: TransportFailure,
    },
    /// This consumer's bounded inbound queue overflowed. Loss is typed, never
    /// silent (GOALS:434, QUERY-011).
    Lost {
        /// Generation during which items were dropped.
        identity: RelaySessionIdentity,
        /// Exact number of items dropped since the last `Lost`.
        dropped: u64,
    },
}

impl RelayInbound {
    /// Session and generation this item belongs to. For a reconnect this is
    /// the generation that is now current.
    #[must_use]
    pub fn identity(&self) -> &RelaySessionIdentity {
        match self {
            Self::Frame { identity, .. }
            | Self::Disconnected { identity, .. }
            | Self::Reconnected { identity, .. }
            | Self::ReconnectExhausted { identity, .. }
            | Self::Lost { identity, .. } => identity,
        }
    }
}
