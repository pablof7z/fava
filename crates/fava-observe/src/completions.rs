//! Provider completions arriving back at the reconciliation owner.
//!
//! Every completion carries the operation generation it was issued under. A
//! completion whose generation the owner has moved past is refused, and any
//! provider resource it produced is released rather than installed.

use std::sync::Arc;

use fava_query::{BoundedText, ObservationId, OperationGeneration, RelaySourceState};
use fava_state::{RelaySessionKey, Timestamp};
use fava_transport::{RelayInbound, RelaySessionLease};
use fava_wire::SubscriptionId;

use crate::diagnostics;
use crate::engine::{Engine, Report};
use crate::facts::failure_state;
use crate::ingest;
use crate::operations;

impl Engine {
    /// Accept one provider completion, refusing every superseded generation.
    pub(crate) fn accept(&mut self, report: Report) -> bool {
        match report {
            Report::Acquired {
                relay,
                generation,
                lease,
            } => self.acquired(&relay, generation, lease),
            Report::Refused {
                relay,
                generation,
                detail,
            } => self.refused(&relay, generation, &detail),
            Report::Applied { relay, generation } => self.applied(&relay, generation),
            Report::Flush { relay, generation } => self.flush(&relay, generation),
            Report::Inbound {
                relay,
                generation,
                item,
            } => self.inbound(&relay, generation, *item),
        }
    }

    fn acquired(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        lease: Box<RelaySessionLease>,
    ) -> bool {
        let rearm;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                self.release_lease(lease);
                return false;
            };
            if slot.generation != generation {
                self.release_lease(lease);
                return false;
            }
            let session = Arc::clone(lease.session());
            slot.lease = Some(lease);
            slot.session = Some(Arc::clone(&session));
            slot.busy = false;
            slot.state = fava_diagnostics::RelaySessionState::Open;
            rearm = !slot.pending.is_empty() && !slot.armed;
            if rearm {
                slot.armed = true;
            }
            operations::listen(
                &self.runtime,
                &self.reports,
                relay.clone(),
                generation,
                &session,
                slot.cancel.clone(),
            );
        }
        if rearm {
            self.arm(relay, generation);
        }
        self.publish_relay_diagnostic(relay);
        false
    }

    fn refused(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        detail: &BoundedText,
    ) -> bool {
        let lease;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            if slot.generation != generation {
                return false;
            }
            slot.state = fava_diagnostics::RelaySessionState::Unreachable {
                detail: detail.clone(),
            };
            lease = slot.lease.take();
            slot.session = None;
            slot.advance(&self.root);
        }
        self.replay(relay);
        {
            if let Some(lease) = lease {
                self.release_lease(lease);
            }
        }
        self.publish_state_for_relay(relay, &failure_state(detail));
        self.publish_relay_diagnostic(relay);
        true
    }

    /// Restore this relay's pending cohort from the demand the registry holds.
    ///
    /// Every request on the previous generation is void, so all of this
    /// relay's current demand is unsent again and re-enters admission.
    fn replay(&mut self, relay: &RelaySessionKey) {
        let demand = self.registry.desired().remove(relay).unwrap_or_default();
        if let Some(slot) = self.slots.get_mut(relay) {
            slot.pending = demand
                .into_iter()
                .map(|item| (item.id(), item))
                .collect();
        }
    }

    fn applied(&mut self, relay: &RelaySessionKey, generation: OperationGeneration) -> bool {
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        let requested_at = Timestamp::now();
        let owners: Vec<ObservationId> = slot
            .live
            .values()
            .flat_map(|entry| entry.serves.iter().map(|demand| demand.owner))
            .collect();
        for owner in owners {
            self.registry.record_state(
                owner,
                relay,
                generation,
                RelaySourceState::Open { requested_at },
            );
        }
        self.publish_relay_diagnostic(relay);
        false
    }

    fn inbound(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        item: RelayInbound,
    ) -> bool {
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        match item {
            RelayInbound::Frame { frame, .. } => {
                self.frame(relay, &frame);
                false
            }
            RelayInbound::Disconnected { reason, .. } => {
                let detail = BoundedText::new(format!("{reason:?}"));
                if let Some(slot) = self.slots.get_mut(relay) {
                    slot.state = fava_diagnostics::RelaySessionState::Reconnecting {
                        detail: detail.clone(),
                    };
                }
                self.publish_state_for_relay(relay, &RelaySourceState::Disconnected { detail });
                self.publish_relay_diagnostic(relay);
                false
            }
            RelayInbound::Reconnected { .. } => self.reconnected(relay),
            RelayInbound::ReconnectExhausted {
                attempts, reason, ..
            } => {
                let detail = BoundedText::new(format!("{reason:?}"));
                if let Some(slot) = self.slots.get_mut(relay) {
                    slot.state = fava_diagnostics::RelaySessionState::Unreachable {
                        detail: detail.clone(),
                    };
                }
                self.publish_state_for_relay(
                    relay,
                    &RelaySourceState::Unreachable { attempts, detail },
                );
                self.publish_relay_diagnostic(relay);
                false
            }
            RelayInbound::Lost { dropped, .. } => {
                self.providers
                    .diagnostics
                    .limit(diagnostics::inbound_loss(relay, dropped));
                false
            }
        }
    }

    /// A new generation is live: every request is void and the demand replays.
    fn reconnected(&mut self, relay: &RelaySessionKey) -> bool {
        let next;
        let armed;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            slot.reconnects = slot.reconnects.saturating_add(1);
            slot.state = fava_diagnostics::RelaySessionState::Open;
            next = slot.advance(&self.root);
            let session = slot.session.clone();
            if let Some(session) = session {
                operations::listen(
                    &self.runtime,
                    &self.reports,
                    relay.clone(),
                    next,
                    &session,
                    slot.cancel.clone(),
                );
            }
        }
        self.replay(relay);
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            slot.armed = !slot.pending.is_empty();
            armed = slot.armed;
        }
        if armed {
            self.arm(relay, next);
        }
        self.publish_state_for_relay(relay, &RelaySourceState::Connecting);
        self.publish_relay_diagnostic(relay);
        false
    }

    fn frame(&mut self, relay: &RelaySessionKey, frame: &[u8]) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let installed = slot.installed();
        let outcome = ingest::accept(self.providers.cache.as_ref(), relay, &installed, frame);
        match outcome {
            ingest::Accepted::Nothing | ingest::Accepted::Event => {}
            ingest::Accepted::StoredEventsComplete(id) => {
                let at = Timestamp::now();
                if let Some(slot) = self.slots.get_mut(relay)
                    && let Some(entry) = slot.live.get_mut(&id)
                {
                    entry.stored_events_complete = true;
                }
                self.publish_for_subscription(
                    relay,
                    &id,
                    &RelaySourceState::StoredEventsComplete { at },
                );
                self.publish_relay_diagnostic(relay);
            }
            ingest::Accepted::Refused { id, message } => {
                let at = Timestamp::now();
                self.publish_for_subscription(
                    relay,
                    &id,
                    &RelaySourceState::Refused { message, at },
                );
                self.publish_relay_diagnostic(relay);
            }
            ingest::Accepted::AuthenticationRequired => {
                let at = Timestamp::now();
                self.publish_state_for_relay(
                    relay,
                    &RelaySourceState::AuthenticationRequired {
                        state: fava_query::AuthenticationState::ChallengeReceived,
                        at,
                    },
                );
                self.publish_relay_diagnostic(relay);
            }
            ingest::Accepted::Unattributed(detail) => {
                self.providers
                    .diagnostics
                    .relay(diagnostics::refused_plan(relay, detail));
            }
        }
    }

    /// Withdraw every live request and release the relay's lease.
    pub(crate) fn release(&mut self, relay: &RelaySessionKey) {
        let Some(mut slot) = self.slots.remove(relay) else {
            return;
        };
        slot.cancel.cancel();
        let subscriptions: Vec<SubscriptionId> = slot.live.keys().cloned().collect();
        let generation = slot.generation;
        if let Some(lease) = slot.lease.take() {
            operations::release(
                &self.runtime,
                lease,
                subscriptions,
                generation,
                self.providers.deadlines.write,
                self.providers.deadlines.close,
            );
        }
        self.providers.diagnostics.forget_relay(relay);
    }

    pub(crate) fn release_lease(&self, lease: Box<RelaySessionLease>) {
        operations::release(
            &self.runtime,
            lease,
            Vec::new(),
            OperationGeneration(0),
            self.providers.deadlines.write,
            self.providers.deadlines.close,
        );
    }}
