//! One live connection generation and its per-consumer inbound streams.

use fava_relay::RelaySessionKey;

use crate::{HandoffFuture, RelayInboundFuture, ReleaseFuture};

/// Exact authority of one live connection generation.
///
/// Authority: ARCH:1567-1571 (`fn identity(&self) -> RelaySessionIdentity`),
/// ARCH:1610 "Every inbound frame and handoff completion carries exact session
/// generation and relay-access identity."
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionIdentity {
    /// Relay URL and relay-access authority.
    pub key: RelaySessionKey,
    /// Transport-owned connection generation. Advances on every reconnect.
    pub generation: RelaySessionGeneration,
}

/// Transport-owned identity of one physical connection generation.
///
/// A transport implementation mints these values. Callers can inspect them,
/// but [`crate::OpenRelaySession`] has no generation input and therefore
/// cannot select the identity a live session will wear.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionGeneration(u64);

impl RelaySessionGeneration {
    /// Construct a non-zero generation inside a transport implementation.
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    /// Raw generation value for diagnostics and provider storage.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Return the next generation, or `None` instead of reusing the maximum.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Caller-supplied correlation for one exact frame handoff.
///
/// Authority: ARCH:1572-1576 (`send(&self, frame, correlation: HandoffCorrelation)`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffCorrelation(u64);

impl HandoffCorrelation {
    /// Mint a caller-owned correlation token.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Raw caller-owned value for diagnostics.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One exact live connection to a relay, shared by every current lease holder.
///
/// Authority: ARCH:1569-1581.
pub trait RelaySession: Send + Sync {
    /// Current identity. The generation changes under the holder on reconnect.
    fn identity(&self) -> RelaySessionIdentity;

    /// Attempt to hand off one complete frame, correlated.
    ///
    /// MUST NOT park indefinitely: the outbound queue is bounded and
    /// `deadlines.write` applies. A full queue is `NotHandedOff`, never a wait.
    fn send(&self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'_>;

    /// Obtain an independently-pollable inbound stream for **this consumer**.
    ///
    /// Two calls return two streams; every inbound item is delivered to every
    /// live stream. One consumer cannot remove an item from another's stream.
    ///
    /// Authority: ARCH:1578 verbatim signature.
    fn messages(&self) -> Box<dyn RelayMessageStream>;

    /// Close this session's current generation deterministically, regardless of
    /// remaining leases. Callers hold leases; this is the transport's own
    /// escape hatch and is idempotent.
    fn close(&self) -> ReleaseFuture<'_>;
}

/// One consumer's bounded view of a session's inbound items.
///
/// Authority: ARCH:1578 (`Box<dyn RelayMessageStream>`).
pub trait RelayMessageStream: Send {
    /// Await the next inbound item for this consumer.
    ///
    /// # Errors
    ///
    /// [`crate::TransportError`] when the consumer's own view cannot continue.
    /// Session lifecycle transitions — disconnect, reconnect, exhaustion, and
    /// bounded loss — arrive as `Ok` [`crate::RelayInbound`] items so they
    /// carry the identity `ARCH:1610` requires.
    fn next_inbound(&mut self) -> RelayInboundFuture<'_>;

    /// Detach this consumer. Idempotent; does not affect other consumers.
    fn close(&mut self);
}

#[cfg(test)]
mod tests {
    use super::RelaySessionGeneration;

    #[test]
    fn maximum_generation_has_no_successor() {
        let maximum = RelaySessionGeneration::new(u64::MAX).expect("non-zero");
        assert_eq!(maximum.checked_next(), None);
        assert_eq!(maximum.get(), u64::MAX);
    }
}
