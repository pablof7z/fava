//! Provider completions arriving back at the reconciliation owner.
//!
//! Every completion carries the round it was issued under. A
//! completion whose generation the owner has moved past is refused, and any
//! provider resource it produced is released rather than installed.

use std::sync::Arc;

use fava_query::{BoundedText, ObservationId, RelaySourceState, Round, SourceCoverage};
use fava_relay::RelaySessionKey;
use fava_transport::RelaySessionLease;
use fava_wire::SubscriptionId;
use nostr::types::Timestamp;

use crate::diagnostics;
use crate::engine::{Engine, Report};
use crate::facts::failure_state;
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
                attending,
                closed,
            } => self.installed(
                &relay, generation, revision, &plan, &opened, attending, &closed,
            ),
            Report::Flush { relay, generation } => self.flush(&relay, generation),
            Report::Connection {
                relay,
                generation,
                state,
            } => self.connection(&relay, generation, *state),
            Report::ConnectionReplaced { relay, generation } => {
                self.connection_replaced(&relay, generation)
            }
            Report::ConnectionEnded => false,
            Report::Carried {
                relay,
                generation,
                subscription,
                item,
            } => self.carried(&relay, generation, &subscription, *item),
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
        generation: Round,
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
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    fn refused(
        &mut self,
        relay: &RelaySessionKey,
        generation: Round,
        detail: &BoundedText,
    ) -> bool {
        let next_generation = match self.next_round() {
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
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        true
    }

    /// Install exactly what the transport accepted, and report the request as
    /// open to every observation the accepted subscriptions now serve.
    #[allow(
        clippy::too_many_arguments,
        reason = "one plan's application names the relay, its generation, its revision, the plan, what opened, what is attending each opened subscription, and what closed"
    )]
    fn installed(
        &mut self,
        relay: &RelaySessionKey,
        generation: Round,
        revision: fava_subscriptions::PlanRevision,
        plan: &fava_subscriptions::SubscriptionPlan,
        opened: &[Option<fava_wire::SubscriptionId>],
        attending: Vec<(fava_wire::SubscriptionId, fava_runtime::CancellationToken)>,
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
        // Closing a subscription is dropping its handle: cancel the task that
        // holds it and its Drop sends the CLOSE.
        if let Some(slot) = self.slots.get_mut(relay) {
            for (id, token) in attending {
                slot.attending.insert(id, token);
            }
            for id in closed {
                if let Some(token) = slot.attending.remove(id) {
                    token.cancel();
                }
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
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    /// One session's connection state changed.
    fn connection(
        &mut self,
        relay: &RelaySessionKey,
        generation: Round,
        state: fava_transport::Connection,
    ) -> bool {
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        let fava_relay::Connectivity::Disconnected { detail, spent } = state.connectivity else {
            return false;
        };
        // A connection that may still return is reconnecting; one that has
        // spent its budget is unreachable, and says what it spent.
        let state = match spent {
            None => {
                self.slot_state(
                    relay,
                    fava_diagnostics::RelaySessionState::Reconnecting {
                        detail: detail.clone(),
                    },
                );
                RelaySourceState::Disconnected { detail }
            }
            Some(attempts) => {
                self.slot_state(
                    relay,
                    fava_diagnostics::RelaySessionState::Unreachable {
                        detail: detail.clone(),
                    },
                );
                RelaySourceState::Unreachable { attempts, detail }
            }
        };
        self.publish_state_for_relay(relay, &state);
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    /// Record how this relay's session now stands, if it is still tracked.
    fn slot_state(&mut self, relay: &RelaySessionKey, state: fava_diagnostics::RelaySessionState) {
        if let Some(slot) = self.slots.get_mut(relay) {
            slot.state = state;
        }
    }

    /// The connection carrying this relay's work was replaced.
    fn connection_replaced(&mut self, relay: &RelaySessionKey, generation: Round) -> bool {
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        self.reconnected(relay)
    }

    /// One installed subscription carried something of its own.
    fn carried(
        &mut self,
        relay: &RelaySessionKey,
        generation: Round,
        subscription: &SubscriptionId,
        item: fava_transport::SubscriptionItem,
    ) -> bool {
        let Some(slot) = self.slots.get(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        match item {
            fava_transport::SubscriptionItem::Event(event) => {
                self.carried_event(relay, subscription, *event)
            }
            fava_transport::SubscriptionItem::EndOfStoredEvents => {
                self.stored_complete(relay, subscription)
            }
            fava_transport::SubscriptionItem::Closed { reason } => {
                self.subscription_refused(relay, subscription, &reason)
            }
            fava_transport::SubscriptionItem::Lost { dropped } => {
                self.providers
                    .diagnostics
                    .limit(diagnostics::inbound_loss(relay, dropped));
                false
            }
            // The connection reader carries the same fact for the whole
            // session, so nothing is published twice here.
            fava_transport::SubscriptionItem::Ended(_) => false,
        }
    }

    /// A new generation is live: every request is void and the demand replays.
    fn reconnected(&mut self, relay: &RelaySessionKey) -> bool {
        let next_generation = match self.next_round() {
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
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    /// One event the relay attributed to one installed subscription.
    fn carried_event(
        &mut self,
        relay: &RelaySessionKey,
        subscription: &SubscriptionId,
        event: nostr::event::Event,
    ) -> bool {
        let Some(entry) = self
            .slots
            .get(relay)
            .and_then(|slot| slot.installed.get(subscription))
            .cloned()
        else {
            // The relay named a subscription this generation never installed.
            // It changes no session state; the observable proof is that the
            // event never reaches the event cache.
            return false;
        };
        // Admission authorizes on the whole accepted filter set, which is
        // NIP-01 REQ semantics.
        let accepted = std::collections::BTreeMap::from([(subscription.clone(), entry.filters)]);
        let Ok(relay_event) = fava_ingest::admit_subscription_event(
            relay,
            &accepted,
            subscription,
            event,
            Timestamp::now(),
        ) else {
            return false;
        };
        let owners = self
            .slots
            .get(relay)
            .map(|slot| slot.owners(subscription))
            .unwrap_or_default();
        for owner in owners {
            self.registry.record_live_event(owner, relay_event.clone());
        }
        let observed_at = relay_event.occurrence().observed_at;
        let _retention = self.providers.cache.admit(relay_event, observed_at);
        false
    }

    /// The relay has sent everything it stored for one installed subscription.
    fn stored_complete(&mut self, relay: &RelaySessionKey, id: &SubscriptionId) -> bool {
        let at = Timestamp::now();
        let proves = self
            .slots
            .get(relay)
            .map(|slot| slot.proves_completeness(id))
            .unwrap_or_default();
        if let Some(slot) = self.slots.get_mut(relay) {
            slot.settled.insert(id.clone(), true);
        }
        if proves == fava_subscriptions::EoseCompleteness::Proven {
            let owners = self
                .slots
                .get(relay)
                .and_then(|slot| slot.installed.get(id))
                .map(|entry| {
                    entry
                        .serves
                        .iter()
                        .map(|demand| demand.owner)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for owner in owners {
                let Some(filter) = self.registry.filter_for(owner, relay) else {
                    continue;
                };
                // Cache refusal is scoped to reusable completion. The EOSE
                // remains ordinary observation evidence and a later open simply
                // reacquires this source.
                let _ = self.providers.cache.retain_source_coverage(SourceCoverage {
                    session: relay.clone(),
                    filter,
                    completed_at: at,
                });
            }
            self.publish_for_subscription(
                relay,
                id,
                &RelaySourceState::StoredEventsComplete { at },
            );
        } else {
            // The relay ended a bounded request, not the stored window.
            // Claiming completeness would claim omitted work was completed
            // (GOALS:1066).
            self.publish_shortfall(relay, id, proves);
        }
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    /// The relay refused or ended one installed subscription, in its own words.
    fn subscription_refused(
        &mut self,
        relay: &RelaySessionKey,
        id: &SubscriptionId,
        message: &BoundedText,
    ) -> bool {
        let at = Timestamp::now();
        self.publish_for_subscription(
            relay,
            id,
            &RelaySourceState::Refused {
                message: message.clone(),
                at,
            },
        );
        self.publish_authentication(relay);
        self.publish_relay_diagnostic(relay);
        false
    }

    /// Withdraw every live request and release the relay's lease.
    pub(crate) fn release(&mut self, relay: &RelaySessionKey) {
        let Some(mut slot) = self.slots.remove(relay) else {
            return;
        };
        // Cancelling the slot drops every subscription handle it held, and each
        // handle sends the relay its own CLOSE on the way out.
        slot.cancel.cancel();
        let generation = slot.generation;
        if let Some(lease) = slot.lease.take() {
            operations::release(
                &self.runtime,
                lease,
                generation,
                self.providers.deadlines.close,
            );
        }
        self.providers.diagnostics.forget_relay(relay);
    }

    pub(crate) fn release_lease(&self, lease: Box<RelaySessionLease>, generation: Round) {
        operations::release(
            &self.runtime,
            lease,
            generation,
            self.providers.deadlines.close,
        );
    }
}
