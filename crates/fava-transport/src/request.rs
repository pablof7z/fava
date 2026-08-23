//! Fava-owned deadlines and bounds for one relay session.

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_state::RelaySessionKey;

/// Fava-owned deadlines for one relay session. Never defaulted by transport.
///
/// Authority: GOALS:424 (QUERY-010) "Timeout, disconnect, retry exhaustion,
/// silence, local cancellation, and relay refusal MUST remain distinct";
/// ARCH:1624 "keepalive and dead-session detection".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportDeadlines {
    /// DNS + TCP + TLS + WebSocket handshake must complete within this.
    pub establish: Duration,
    /// One frame must reach the outbound queue *and* the socket within this.
    pub write: Duration,
    /// Maximum silence before the session is declared dead. A keepalive probe
    /// is the implementer's business; the deadline is not.
    pub idle: Duration,
    /// Close handshake must complete within this; afterwards the session is
    /// dropped and reported closed regardless of the peer.
    pub close: Duration,
}

/// Bounded byte queues for one session, in whole frames.
///
/// Authority: ARCH:1590 "bounded inbound and outbound byte queues";
/// GOALS:1448 (OPS-004) "Exceeding a bound MUST produce refusal, backpressure,
/// or exact shortfall."
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportBounds {
    /// Frames buffered per inbound consumer stream before typed loss.
    pub inbound_frames: NonZeroUsize,
    /// Frames buffered for the socket writer before refusal.
    pub outbound_frames: NonZeroUsize,
    /// Maximum encoded size of a single frame, both directions.
    pub max_frame_bytes: NonZeroUsize,
}

/// Complete acquire request for one relay-access identity.
///
/// Authority: ARCH:1560-1565 (`fn open_session(&self, request: OpenRelaySession)`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRelaySession {
    /// Relay URL and access authority to acquire.
    pub key: RelaySessionKey,
    /// Fava-owned deadlines applied to this session.
    pub deadlines: TransportDeadlines,
    /// Fava-owned queue and frame bounds applied to this session.
    pub bounds: TransportBounds,
    /// Reconnect budget. `None` means reconnect until every lease is released.
    pub reconnect_attempts: Option<NonZeroUsize>,
}
