//! The ownership graph of open observations.

use std::num::NonZeroUsize;

use fava_query::{BoundedText, ObservationId, QueryBranchId, RelaySourceState};
use fava_relay::RelaySessionKey;
use fava_wire::SubscriptionId;

use crate::providers::ProviderOperation;

/// The complete ownership record for one open observation.
///
/// Authority: ARCH:2320-2328 "open observation and route ownership".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryDiagnostic {
    /// Observation identity.
    pub observation: ObservationId,
    /// Route revision currently bound to this observation, when automatic.
    pub route_revision: Option<u64>,
    /// Relay destinations the bound route revision names.
    pub route_relays: Vec<RelaySessionKey>,
    /// Logical demand this observation currently holds, per relay per branch.
    pub demand: Vec<LogicalDemandDiagnostic>,
    /// Desired-plan revision currently installed for this observation.
    pub plan_revision: Option<u64>,
    /// Wire subscriptions this observation currently relies on.
    pub wire: Vec<ObservationWireBinding>,
    /// Source shortfalls scoped to this observation.
    pub shortfalls: Vec<BoundedText>,
    /// Provider operation this observation is currently waiting on.
    pub pending_operation: Option<ProviderOperation>,
    /// Query revisions superseded before delivery.
    pub coalesced_updates: u64,
}

/// One relay's worth of one observation's logical demand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalDemandDiagnostic {
    /// Relay session this demand is assigned to.
    pub session: RelaySessionKey,
    /// Branch that needs it.
    pub branch: QueryBranchId,
    /// Current state of this relay's contribution to this observation.
    pub state: RelaySourceState,
}

/// Binding from one observation to one shared wire subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationWireBinding {
    /// Relay session.
    pub session: RelaySessionKey,
    /// Wire subscription id.
    pub subscription: SubscriptionId,
    /// Total observations sharing this wire subscription, including this one.
    pub shared_holders: NonZeroUsize,
}
