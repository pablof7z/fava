//! The facts this owner publishes: scoped relay evidence for every observation
//! that uses a relay, and the bounded diagnostics record for the session.

use std::time::Duration;

use fava_query::{
    BoundedText, OperationGeneration, RelayDeadline, RelayShortfall, RelaySourceState,
};
use fava_relay::RelaySessionKey;
use fava_subscriptions::{RelayDemand, SubscriptionPlan};
use fava_transport::{TransportBounds, TransportDeadlines};
use fava_wire::SubscriptionId;

use crate::diagnostics;
use crate::engine::Engine;

impl Engine {
    /// Tell every listed demand owner how far this relay has got.
    pub(crate) fn publish_states(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        generation: OperationGeneration,
        state: &RelaySourceState,
    ) {
        for item in demand {
            self.registry
                .record_state(item.owner, relay, Some(generation), state.clone());
        }
    }

    /// Tell every observation holding demand at this relay how far it has got.
    pub(crate) fn publish_state_for_relay(
        &self,
        relay: &RelaySessionKey,
        state: &RelaySourceState,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        for owner in self.registry.open_observations() {
            if self.registry.demand_id(owner, relay).is_some() {
                self.registry
                    .record_state(owner, relay, Some(generation), state.clone());
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
                .record_state(owner, relay, Some(generation), state.clone());
        }
    }

    /// Publish who shares each request, and what the plan could not carry.
    pub(crate) fn publish_plan(
        &self,
        relay: &RelaySessionKey,
        cohort: &[RelayDemand],
        planned: Option<&SubscriptionPlan>,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let revision = planned.map_or_else(
            || {
                slot.revision
                    .map_or(0, |revision| revision.sequence().get())
            },
            |plan| plan.revision.sequence().get(),
        );
        for item in cohort {
            let id = item.id();
            let shared_with = slot
                .serving(id)
                .map(|wire| slot.owners(wire))
                .unwrap_or_default();
            let shortfall = planned.and_then(|plan| {
                plan.shortfalls
                    .iter()
                    .find(|entry| entry.demand == id)
                    .map(|entry| RelayShortfall {
                        branches: vec![item.branch],
                        detail: BoundedText::new(format!("{:?}", entry.reason)),
                    })
            });
            self.registry
                .record_sharing(item.owner, relay, revision, shared_with, shortfall);
            self.registry.record_plan(
                item.owner,
                fava_query::DesiredPlanEvidence {
                    revision,
                    relays: vec![relay.clone()],
                    installed: slot.installed.len(),
                },
            );
        }
    }

    /// Report that an EOSE on one wire subscription proved less than the
    /// stored window, so no observation may read it as completeness.
    pub(crate) fn publish_shortfall(
        &self,
        relay: &RelaySessionKey,
        id: &fava_wire::SubscriptionId,
        completeness: fava_subscriptions::EoseCompleteness,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        let detail = BoundedText::new(format!("{completeness:?}"));
        for owner in slot.owners(id) {
            let branches = self
                .registry
                .demand_id(owner, relay)
                .map(|demand| vec![demand.branch])
                .unwrap_or_default();
            self.registry.record_sharing(
                owner,
                relay,
                slot.revision
                    .map_or(0, |revision| revision.sequence().get()),
                slot.owners(id),
                Some(RelayShortfall {
                    branches,
                    detail: detail.clone(),
                }),
            );
            self.registry.record_state(
                owner,
                relay,
                Some(generation),
                RelaySourceState::Open {
                    requested_at: nostr::types::Timestamp::now(),
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
            Some(slot.generation),
            slot.state.clone(),
            usize::from(slot.lease.is_some()),
            slot.installed
                .ids()
                .map(|id| {
                    diagnostics::wire_fact(
                        id.clone(),
                        self.registry.public_ids(slot.owners(id)),
                        slot.settled.get(id).copied().unwrap_or_default(),
                    )
                })
                .collect(),
            slot.reconnects,
        ));
    }
}

/// Classify a transport failure reason as an expired establish or idle deadline,
/// or else an ordinary disconnect.
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
