//! The facts this owner publishes: scoped relay evidence for every observation
//! that uses a relay, and the bounded diagnostics record for the session.

use std::time::Duration;

use fava_query::{
    BoundedText, OperationGeneration, RelayDeadline, RelayShortfall, RelaySourceState,
};
use fava_state::RelaySessionKey;
use fava_transport::{TransportBounds, TransportDeadlines};
use fava_subscriptions::{RelayDemand, SubscriptionPlan};
use fava_wire::SubscriptionId;

use crate::diagnostics;
use crate::engine::Engine;

impl Engine {
    pub(crate) fn publish_states(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        generation: OperationGeneration,
        state: &RelaySourceState,
    ) {
        for item in demand {
            self.registry
                .record_state(item.owner, relay, generation, state.clone());
        }
    }

    pub(crate) fn publish_state_for_relay(&self, relay: &RelaySessionKey, state: &RelaySourceState) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        for owner in self.registry.open_observations() {
            if self.registry.demand_id(owner, relay).is_some() {
                self.registry
                    .record_state(owner, relay, generation, state.clone());
            }
        }
    }

    pub(crate) fn publish_for_subscription(
        &self,
        relay: &RelaySessionKey,
        id: &SubscriptionId,
        state: &RelaySourceState,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        for owner in slot.owners(id) {
            self.registry
                .record_state(owner, relay, generation, state.clone());
        }
    }

    /// Publish who shares each request, and what the plan could not carry.
    pub(crate) fn publish_plan(
        &self,
        relay: &RelaySessionKey,
        cohort: &[RelayDemand],
        planned: &SubscriptionPlan,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let revision = planned.revision.0;
        for item in cohort {
            let id = item.id();
            let shared_with = slot
                .serving(id)
                .map(|wire| slot.owners(wire))
                .unwrap_or_default();
            let shortfall = planned
                .shortfalls
                .iter()
                .find(|entry| entry.demand == id)
                .map(|entry| RelayShortfall {
                    branches: vec![item.branch],
                    detail: BoundedText::new(format!("{:?}", entry.reason)),
                });
            self.registry
                .record_sharing(item.owner, relay, revision, shared_with, shortfall);
            self.registry.record_plan(
                item.owner,
                fava_query::DesiredPlanEvidence {
                    revision,
                    relays: vec![relay.clone()],
                    installed: slot.live.len(),
                },
            );
        }
    }

    pub(crate) fn publish_relay_diagnostic(&self, relay: &RelaySessionKey) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        self.providers.diagnostics.relay(diagnostics::relay_fact(
            relay,
            slot.generation,
            slot.state.clone(),
            usize::from(slot.lease.is_some()),
            slot.live
                .iter()
                .map(|(id, entry)| {
                    diagnostics::wire_fact(
                        id.clone(),
                        slot.owners(id),
                        entry.stored_events_complete,
                    )
                })
                .collect(),
            slot.reconnects,
        ));
    }}


pub(crate) fn failure_state(detail: &BoundedText) -> RelaySourceState {
    if detail.as_str().contains("EstablishTimeout") {
        return RelaySourceState::TimedOut {
            deadline: RelayDeadline::Establish,
            after_ms: 0,
        };
    }
    if detail.as_str().contains("IdleTimeout") {
        return RelaySourceState::TimedOut {
            deadline: RelayDeadline::Idle,
            after_ms: 0,
        };
    }
    RelaySourceState::Disconnected {
        detail: detail.clone(),
    }
}

/// Fava-owned defaults for the four transport deadlines.
pub(crate) const fn default_deadlines() -> TransportDeadlines {
    TransportDeadlines {
        establish: Duration::from_secs(10),
        write: Duration::from_secs(5),
        idle: Duration::from_secs(120),
        close: Duration::from_secs(5),
    }
}

/// Fava-owned defaults for the transport's bounded queues.
pub(crate) fn default_bounds() -> TransportBounds {
    TransportBounds {
        inbound_frames: nonzero(256),
        outbound_frames: nonzero(256),
        max_frame_bytes: nonzero(512 * 1024),
    }
}

fn nonzero(value: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).expect("constant is non-zero")
}

