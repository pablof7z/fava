//! Provider completions arriving back at the reconciliation owner.
//!
//! Every completion carries the operation generation it was issued under. A
//! completion whose generation the owner has moved past is refused, and any
//! provider resource it produced is released rather than installed.

use std::sync::Arc;

use fava_query::{BoundedText, ObservationId, OperationGeneration, RelaySourceState};
use fava_relay::RelaySessionKey;
use fava_transport::{RelayInbound, RelaySessionLease};
use fava_wire::SubscriptionId;
use nostr::types::Timestamp;

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
            Report::Installed {
                relay,
                generation,
                revision,
                plan,
                opened,
                closed,
            } => self.installed(&relay, generation, revision, &plan, &opened, &closed),
            Report::Flush { relay, generation } => self.flush(&relay, generation),
            Report::Inbound {
                relay,
                generation,
                item,
            } => self.inbound(&relay, generation, *item),
            Report::Constraints { relay, constraints } => {
                self.constraints_received(&relay, constraints);
                false
            }
        }
    }

    fn constraints_received(
        &mut self,
        relay: &RelaySessionKey,
        constraints: fava_subscriptions::RelayReadConstraints,
    ) {
        if let Some(slot) = self.slots.get_mut(relay) {
            slot.constraints = constraints;
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
                self.release_lease(lease, generation);
                return false;
            };
            if slot.generation != generation {
                self.release_lease(lease, generation);
                return false;
            }
            let session = Arc::clone(lease.session());
            slot.lease = Some(lease);
            slot.session = Some(Arc::clone(&session));
            slot.busy = false;
            slot.state = fava_diagnostics::RelaySessionState::Open;
            rearm = !slot.armed;
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
            operations::fetch_constraints(
                &self.runtime,
                &self.reports,
                relay.clone(),
                self.providers.deadlines.establish,
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
        let next_generation = match self.next_operation_generation() {
            Ok(generation) => generation,
            Err(error) => {
                let demand = self.registry.desired().remove(relay).unwrap_or_default();
                self.publish_owner_refusal(relay, &demand, &error);
                return false;
            }
        };
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
            slot.advance(&self.root, next_generation);
        }
        {
            if let Some(lease) = lease {
                self.release_lease(lease, generation);
            }
        }
        self.publish_state_for_relay(relay, &failure_state(detail));
        self.publish_relay_diagnostic(relay);
        true
    }

    /// Install exactly what the transport accepted, and report the request as
    /// open to every observation the accepted subscriptions now serve.
    fn installed(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        revision: fava_subscriptions::PlanRevision,
        plan: &fava_subscriptions::SubscriptionPlan,
        opened: &std::collections::BTreeSet<fava_wire::SubscriptionId>,
        closed: &std::collections::BTreeSet<fava_wire::SubscriptionId>,
    ) -> bool {
        {
            let Some(slot) = self.slots.get(relay) else {
                return false;
            };
            if slot.generation != generation || slot.revision != Some(revision) {
                return false;
            }
        }
        self.record_installed(relay, plan, opened, closed);
        let demand = self.registry.desired().remove(relay).unwrap_or_default();
        self.publish_plan(relay, &demand, Some(plan));
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        let requested_at = Timestamp::now();
        let owners: Vec<ObservationId> = slot
            .installed
            .ids()
            .filter_map(|id| slot.installed.get(id))
            .flat_map(|entry| entry.serves.iter().map(|demand| demand.owner))
            .collect();
        for owner in owners {
            self.registry.record_state(
                owner,
                relay,
                Some(generation),
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
        let next_generation = match self.next_operation_generation() {
            Ok(generation) => generation,
            Err(error) => {
                let demand = self.registry.desired().remove(relay).unwrap_or_default();
                self.publish_owner_refusal(relay, &demand, &error);
                return false;
            }
        };
        let next;
        let armed;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            slot.reconnects = slot.reconnects.saturating_add(1);
            slot.state = fava_diagnostics::RelaySessionState::Open;
            next = slot.advance(&self.root, next_generation);
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
        {
            let demand = self.registry.desired().remove(relay).unwrap_or_default();
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            // Every request on the previous generation is void, so all of this
            // relay's demand is unsent again and re-enters admission.
            slot.armed = !slot.uncovered(&demand).is_empty();
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
        let installed = slot.installed.clone();
        let outcome = ingest::accept(relay, &installed, frame);
        match outcome {
            ingest::Accepted::Nothing => {}
            ingest::Accepted::Event {
                subscription,
                relay_event,
            } => {
                let owners = self
                    .slots
                    .get(relay)
                    .map(|slot| slot.owners(&subscription))
                    .unwrap_or_default();
                for owner in owners {
                    self.registry
                        .record_live_event(owner, relay_event.as_ref().clone());
                }
                let observed_at = relay_event.occurrence().observed_at;
                let _retention = self.providers.cache.admit(*relay_event, observed_at);
            }
            ingest::Accepted::StoredEventsComplete(id) => {
                let at = Timestamp::now();
                let proves = self
                    .slots
                    .get(relay)
                    .map(|slot| slot.proves_completeness(&id))
                    .unwrap_or_default();
                if let Some(slot) = self.slots.get_mut(relay) {
                    slot.settled.insert(id.clone(), true);
                }
                if proves == fava_subscriptions::EoseCompleteness::Proven {
                    self.publish_for_subscription(
                        relay,
                        &id,
                        &RelaySourceState::StoredEventsComplete { at },
                    );
                } else {
                    // The relay ended a bounded request, not the stored window.
                    // Claiming completeness would claim omitted work was
                    // completed (GOALS:1066).
                    self.publish_shortfall(relay, &id, proves);
                }
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
            ingest::Accepted::Unattributed(_detail) => {
                // A frame naming demand this generation never installed is
                // refused before admission. It changes no session state, so
                // the session's own record stays as it is; the observable
                // proof is that the event never reaches the event cache.
            }
        }
    }

    /// Withdraw every live request and release the relay's lease.
    pub(crate) fn release(&mut self, relay: &RelaySessionKey) {
        let Some(mut slot) = self.slots.remove(relay) else {
            return;
        };
        slot.cancel.cancel();
        let subscriptions: Vec<SubscriptionId> = slot.installed.ids().cloned().collect();
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

    pub(crate) fn release_lease(
        &self,
        lease: Box<RelaySessionLease>,
        generation: OperationGeneration,
    ) {
        operations::release(
            &self.runtime,
            lease,
            Vec::new(),
            generation,
            self.providers.deadlines.write,
            self.providers.deadlines.close,
        );
    }
}
