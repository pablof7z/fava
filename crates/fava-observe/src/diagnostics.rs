//! The facts this owner publishes into the shared bounded diagnostics.

use fava_diagnostics::{
    BoundKind, LimitDiagnostic, LimitScope, LogicalDemandDiagnostic, QueryDiagnostic,
    RelayDiagnostic, RelaySessionState, WireSubscriptionDiagnostic,
};
use fava_query::{BoundedText, ObservationId, OperationGeneration, RelayQueryEvidence};
use fava_relay::RelaySessionKey;
use fava_wire::SubscriptionId;

/// Current facts for one relay session this owner holds.
pub(crate) fn relay_fact(
    session: &RelaySessionKey,
    generation: OperationGeneration,
    state: RelaySessionState,
    holders: usize,
    subscriptions: Vec<WireSubscriptionDiagnostic>,
    reconnect_attempts: usize,
) -> RelayDiagnostic {
    RelayDiagnostic {
        session: session.clone(),
        generation,
        state,
        holders,
        subscriptions,
        reconnect_attempts,
    }
}

/// One installed wire subscription and the observations it serves.
pub(crate) fn wire_fact(
    id: SubscriptionId,
    serves: Vec<ObservationId>,
    stored_events_complete: bool,
) -> WireSubscriptionDiagnostic {
    WireSubscriptionDiagnostic {
        id,
        serves,
        stored_events_complete,
        closed: None,
    }
}

/// A relay whose plan this owner could not install.
pub(crate) fn refused_plan(session: &RelaySessionKey, detail: BoundedText) -> RelayDiagnostic {
    RelayDiagnostic {
        session: session.clone(),
        generation: OperationGeneration(0),
        state: RelaySessionState::Unreachable { detail },
        holders: 0,
        subscriptions: Vec::new(),
        reconnect_attempts: 0,
    }
}

/// Bounded inbound loss reported by one relay session's consumer.
pub(crate) fn inbound_loss(session: &RelaySessionKey, dropped: u64) -> LimitDiagnostic {
    LimitDiagnostic {
        bound: BoundKind::InboundQueue,
        limit: 0,
        required: usize::try_from(dropped).unwrap_or(usize::MAX),
        scope: LimitScope::Relay {
            session: session.clone(),
        },
    }
}

/// The complete ownership record for one open observation.
pub(crate) fn query_fact(
    observation: ObservationId,
    route_revision: Option<u64>,
    plan_revision: Option<u64>,
    relays: &[RelayQueryEvidence],
    coalesced_updates: u64,
) -> QueryDiagnostic {
    QueryDiagnostic {
        observation,
        route_revision,
        route_relays: relays.iter().map(|entry| entry.session.clone()).collect(),
        demand: relays
            .iter()
            .map(|entry| LogicalDemandDiagnostic {
                session: entry.session.clone(),
                branch: entry.branches.first().copied().unwrap_or_default(),
                state: entry.state.clone(),
            })
            .collect(),
        plan_revision,
        wire: Vec::new(),
        shortfalls: relays
            .iter()
            .filter_map(|entry| entry.shortfall.as_ref().map(|value| value.detail.clone()))
            .collect(),
        pending_operation: None,
        coalesced_updates,
    }
}
