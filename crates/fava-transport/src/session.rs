//! One live connection generation and its per-consumer inbound streams.

use fava_query::OperationGeneration;
use fava_state::RelaySessionKey;

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
    pub generation: OperationGeneration,
}

/// Caller-supplied correlation for one exact frame handoff.
///
/// Authority: ARCH:1572-1576 (`send(&self, frame, correlation: HandoffCorrelation)`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffCorrelation(pub u64);

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
    fn send<'a>(&'a self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'a>;

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
    fn close<'a>(&'a self) -> ReleaseFuture<'a>;
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
