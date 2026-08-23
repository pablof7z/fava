//! Correlated handoff completions and their scoped reasons.

use std::num::NonZeroUsize;
use std::time::Duration;

use crate::{BoundedReason, HandoffCorrelation, RelaySessionIdentity};

/// Correlated result of attempting to hand one exact frame to a relay session.
///
/// Authority: ARCH:1599-1604 (three variants), ARCH:1610 (must carry identity).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffOutcome {
    /// Bytes definitely did not leave Fava.
    NotHandedOff {
        /// Session and generation the attempt was made against.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
        /// Exact local refusal reason.
        reason: TransportFailure,
    },
    /// The transport accepted the complete frame for the session.
    HandedOff {
        /// Session and generation that accepted the bytes.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
    },
    /// The transport cannot prove whether the relay received the frame.
    Ambiguous {
        /// Session and generation the attempt was made against.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
        /// Exact ambiguity reason.
        reason: TransportAmbiguity,
    },
}

impl HandoffOutcome {
    /// Session and generation this completion belongs to.
    #[must_use]
    pub fn identity(&self) -> &RelaySessionIdentity {
        match self {
            Self::NotHandedOff { identity, .. }
            | Self::HandedOff { identity, .. }
            | Self::Ambiguous { identity, .. } => identity,
        }
    }

    /// Caller correlation this completion belongs to.
    #[must_use]
    pub fn correlation(&self) -> HandoffCorrelation {
        match self {
            Self::NotHandedOff { correlation, .. }
            | Self::HandedOff { correlation, .. }
            | Self::Ambiguous { correlation, .. } => *correlation,
        }
    }
}

/// Reasons bytes definitely did not leave Fava.
///
/// Authority: ARCH:1600-1602 names `TransportFailure` as a distinct type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportFailure {
    /// The session generation is closed.
    SessionClosed,
    /// The outbound queue is full at its declared bound.
    OutboundQueueFull {
        /// Declared bound in frames.
        capacity: usize,
    },
    /// The frame exceeds the declared per-frame byte bound.
    FrameTooLarge {
        /// Exact encoded size.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Establishment did not complete within `TransportDeadlines::establish`.
    EstablishTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// No inbound item within `TransportDeadlines::idle`.
    IdleTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// The relay or the network refused or dropped the connection.
    Disconnected {
        /// Bounded verbatim reason (GOALS:1111, RELAY-008).
        detail: BoundedReason,
    },
    /// Fava is shutting down; no new bytes are admitted.
    ShuttingDown,
}

/// Reasons the transport cannot prove whether bytes reached the relay.
///
/// Authority: ARCH:1600-1602 names `TransportAmbiguity` as a distinct type;
/// ARCH:1606-1608 requires the distinction to survive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportAmbiguity {
    /// The socket accepted the bytes and then errored before flush confirmation.
    FlushUnconfirmed {
        /// Bounded verbatim reason.
        detail: BoundedReason,
    },
    /// `TransportDeadlines::write` expired after the bytes entered the socket.
    WriteTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// The session disconnected while the frame was in flight.
    DisconnectedInFlight {
        /// Bounded verbatim reason.
        detail: BoundedReason,
    },
}

/// Result of releasing one lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseOutcome {
    /// Other holders remain; the session stays open.
    Retained {
        /// Holder count after this release.
        holders: NonZeroUsize,
    },
    /// This was the last holder; the session was closed deterministically.
    Closed,
}
