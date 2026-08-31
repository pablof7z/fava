//! Current facts about the relay sessions Fava holds.

use fava_query::{BoundedText, ObservationId, Round};
use fava_relay::RelaySessionKey;
use fava_wire::SubscriptionId;

/// Current state of one relay session Fava holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayDiagnostic {
    /// Relay and access authority.
    pub session: RelaySessionKey,
    /// Current observation-owned provider-round, when assigned.
    pub generation: Option<Round>,
    /// Whether this connection is establishing, live, reconnecting, out of
    /// reconnect budget, or closed.
    pub state: RelaySessionState,
    /// Lease holders on this session — the shared-work refcount.
    pub holders: usize,
    /// Wire subscriptions currently installed on this session.
    pub subscriptions: Vec<WireSubscriptionDiagnostic>,
    /// Reconnect attempts made on this key since the last success.
    pub reconnect_attempts: usize,
}

/// Whether one relay connection is establishing, live, reconnecting, out of
/// reconnect budget, or closed, independent of any query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelaySessionState {
    /// Establishing.
    Connecting,
    /// Live.
    Open,
    /// Dropped; reconnect in progress.
    Reconnecting {
        /// Bounded reason for the drop.
        detail: BoundedText,
    },
    /// Reconnect exhausted.
    Unreachable {
        /// Bounded reason of the final attempt.
        detail: BoundedText,
    },
    /// Closed deterministically.
    Closed,
}

/// One wire subscription installed on one session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireSubscriptionDiagnostic {
    /// Wire id.
    pub id: SubscriptionId,
    /// Observations whose demand it serves — grouped-EOSE fan-out, visible.
    pub serves: Vec<ObservationId>,
    /// Whether an EOSE has arrived for this exact wire id and generation.
    pub stored_events_complete: bool,
    /// Verbatim, bounded CLOSED text if the relay refused it.
    pub closed: Option<BoundedText>,
}
