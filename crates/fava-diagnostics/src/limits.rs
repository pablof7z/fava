//! Current facts about bounds that refused, backpressured, or fell short.

use fava_query::ObservationId;
use fava_state::RelaySessionKey;

use crate::providers::ProviderOperation;

/// One bound that refused, backpressured, or fell short.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitDiagnostic {
    /// Which bound.
    pub bound: BoundKind,
    /// What the bound was.
    pub limit: usize,
    /// What was required.
    pub required: usize,
    /// Scope the shortfall is attributable to.
    pub scope: LimitScope,
}

/// The externally-influenced resources Fava bounds (GOALS:1431-1448).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundKind {
    /// Concurrent relay sessions.
    RelaySessions,
    /// Wire subscriptions on one relay.
    WireSubscriptions,
    /// Outbound frame bytes.
    OutboundFrameBytes,
    /// Inbound frame bytes.
    InboundFrameBytes,
    /// Outbound queue depth.
    OutboundQueue,
    /// Inbound queue depth.
    InboundQueue,
    /// Router fan-out.
    RouteFanOut,
    /// Event-cache capacity.
    EventCacheCapacity,
    /// Write-store active work.
    WriteStoreActiveWork,
    /// Observation delivery queue.
    ObservationDelivery,
    /// Diagnostics retention.
    DiagnosticsRetention,
    /// Provider operation concurrency.
    ProviderOperations,
}

/// What a limit shortfall is attributable to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LimitScope {
    /// Engine-wide.
    Engine,
    /// One relay session.
    Relay {
        /// The session.
        session: RelaySessionKey,
    },
    /// One observation.
    Observation {
        /// The observation.
        observation: ObservationId,
    },
    /// One provider operation.
    Provider {
        /// The operation.
        operation: ProviderOperation,
    },
}
