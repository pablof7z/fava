//! Provider completions arriving back at the reconciliation owner.
//!
//! Every completion carries the round it was issued under. The round alone
//! finds the exact slot it belongs to — it is engine-wide monotonic and never
//! reused, including by that slot's own reconnect — so a completion whose
//! generation the owner has moved past is refused, and any provider resource
//! it produced is released rather than installed.

use std::sync::Arc;

use fava_query::{BoundedText, ObservationId, RelaySourceState, Round, SourceCoverage};
use fava_relay::Authority;
use fava_transport::{RelaySessionExt, RelaySessionLease};
use fava_wire::SubscriptionId;
use nostr::types::{RelayUrl, Timestamp};

use crate::diagnostics;
use crate::engine::{self, Engine, Report};
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
            Report::Constraints {
                relay,
                generation,
                constraints,
            } => {
                self.constraints_received(&relay, generation, constraints);
                false
            }
        }
    }

    fn constraints_received(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        constraints: fava_subscriptions::RelayReadConstraints,
    ) {
        if let Some(slot) = self.slot_mut(relay, generation) {
            slot.constraints = constraints;
        }
    }

    fn acquired(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        lease: Box<RelaySessionLease>,
    ) -> bool {
        let rearm;
        {
            let Some(slot) = engine::slot_mut(&mut self.slots, relay, generation) else {
                self.release_lease(lease, generation);
                return false;
            };
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
                generation,
                self.providers.deadlines.establish,
                slot.cancel.clone(),
            );
        }
        if rearm {
            self.arm(relay, generation);
        }
        self.publish_relay_diagnostic(relay, generation);
        false
    }

    fn refused(&mut self, relay: &RelayUrl, generation: Round, detail: &BoundedText) -> bool {
        let next_generation = match self.next_round() {
            Ok(generation) => generation,
            Err(error) => {
                let demand = self.demand_for_slot(relay, generation);
                self.publish_owner_refusal(relay, &demand, Some(generation), &error);
                return false;
            }
        };
        let lease;
        {
            let Some(slot) = engine::slot_mut(&mut self.slots, relay, generation) else {
                return false;
            };
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
        // The slot just advanced past `generation`, so its current state and
        // demand are read at `next_generation`.
        self.publish_state_for_relay(relay, next_generation, &failure_state(detail));
        self.publish_relay_diagnostic(relay, next_generation);
        true
    }

    /// Install exactly what the transport accepted, and report the request as
    /// open to every observation the newly opened subscriptions now serve.
    #[allow(
        clippy::too_many_arguments,
        reason = "one plan's application names the relay, its generation, its revision, the plan, what opened, what is attending each opened subscription, and what closed"
    )]
    fn installed(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        revision: fava_subscriptions::PlanRevision,
        plan: &fava_subscriptions::SubscriptionPlan,
        opened: &[Option<fava_wire::SubscriptionId>],
        attending: Vec<(fava_wire::SubscriptionId, fava_runtime::CancellationToken)>,
        closed: &std::collections::BTreeSet<fava_wire::SubscriptionId>,
    ) -> bool {
        {
            let Some(slot) = self.slot(relay, generation) else {
                return false;
            };
            if slot.revision != Some(revision) {
                return false;
            }
        }
        // Closing a subscription is dropping its handle: cancel the task that
        // holds it and its Drop sends the CLOSE.
        if let Some(slot) = self.slot_mut(relay, generation) {
            for (id, token) in attending {
                slot.attending.insert(id, token);
            }
            for id in closed {
                if let Some(token) = slot.attending.remove(id) {
                    token.cancel();
                }
            }
        }
        self.record_installed(relay, generation, plan, opened, closed);
        let demand = self.demand_for_slot(relay, generation);
        self.publish_plan(relay, generation, &demand, Some(plan));
        let Some(slot) = self.slot(relay, generation) else {
            return false;
        };
        let requested_at = Timestamp::now();
        // Only a subscription this plan actually opened had a transition to
        // report. A retained one keeps the state it already earned.
        let owners: Vec<ObservationId> = opened
            .iter()
            .flatten()
            .flat_map(|id| slot.owners(id))
            .collect();
        for owner in owners {
            self.registry.record_state(
                owner,
                relay,
                Some(generation),
                RelaySourceState::Open { requested_at },
            );
        }
        self.publish_relay_diagnostic(relay, generation);
        false
    }

    /// One session's connection state changed.
    fn connection(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        state: fava_transport::Connection,
    ) -> bool {
        let Some(slot) = self.slot(relay, generation) else {
            return false;
        };
        // The relay asked and nothing has decided what to do about it yet.
        // Reported only for a slot this very demand opened to reach an
        // account (`Authority::As`): an anonymous slot never asked to be
        // authenticated, so a relay that challenges it anyway is not this
        // demand's fact (`GOALS:1111`, RELAY-008's isolation). This is the
        // connection's own fact, published the moment it becomes true; a
        // later resolution (accepted, declined, refused) is reported through
        // its own arrival rather than a repeat of this one, so a relay's own
        // words about a specific subscription are never overwritten by a
        // stale restatement of "still asking".
        if matches!(slot.requested, Authority::As(_))
            && matches!(
                state.authentication.progress,
                fava_relay::Progress::Requested { .. }
            )
        {
            self.publish_state_for_relay(
                relay,
                generation,
                &RelaySourceState::AuthenticationRequired {
                    progress: state.authentication.progress.clone(),
                    at: Timestamp::now(),
                },
            );
            self.publish_relay_diagnostic(relay, generation);
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
                    generation,
                    fava_diagnostics::RelaySessionState::Reconnecting {
                        detail: detail.clone(),
                    },
                );
                RelaySourceState::Disconnected { detail }
            }
            Some(attempts) => {
                self.slot_state(
                    relay,
                    generation,
                    fava_diagnostics::RelaySessionState::Unreachable {
                        detail: detail.clone(),
                    },
                );
                RelaySourceState::Unreachable { attempts, detail }
            }
        };
        self.publish_state_for_relay(relay, generation, &state);
        self.publish_relay_diagnostic(relay, generation);
        false
    }

    /// Record how this slot's session now stands, if it is still tracked.
    fn slot_state(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        state: fava_diagnostics::RelaySessionState,
    ) {
        if let Some(slot) = self.slot_mut(relay, generation) {
            slot.state = state;
        }
    }

    /// The connection carrying this slot's work was replaced.
    fn connection_replaced(&mut self, relay: &RelayUrl, generation: Round) -> bool {
        if self.slot(relay, generation).is_none() {
            return false;
        }
        self.reconnected(relay, generation)
    }

    /// One installed subscription carried something of its own.
    fn carried(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        subscription: &SubscriptionId,
        item: fava_transport::SubscriptionItem,
    ) -> bool {
        if self.slot(relay, generation).is_none() {
            return false;
        }
        match item {
            fava_transport::SubscriptionItem::Event(event) => {
                self.carried_event(relay, generation, subscription, *event)
            }
            fava_transport::SubscriptionItem::EndOfStoredEvents => {
                self.stored_complete(relay, generation, subscription)
            }
            fava_transport::SubscriptionItem::Closed { reason } => {
                self.subscription_refused(relay, generation, subscription, &reason)
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
    fn reconnected(&mut self, relay: &RelayUrl, generation: Round) -> bool {
        let next_generation = match self.next_round() {
            Ok(generation) => generation,
            Err(error) => {
                let demand = self.demand_for_slot(relay, generation);
                self.publish_owner_refusal(relay, &demand, Some(generation), &error);
                return false;
            }
        };
        let next;
        let armed;
        {
            let Some(slot) = engine::slot_mut(&mut self.slots, relay, generation) else {
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
            let demand = self.demand_for_slot(relay, next);
            let Some(slot) = self.slot_mut(relay, next) else {
                return false;
            };
            // Every request on the previous generation is void, so all of this
            // slot's demand is unsent again and re-enters admission.
            slot.armed = !slot.uncovered(&demand).is_empty();
            armed = slot.armed;
        }
        if armed {
            self.arm(relay, next);
        }
        self.publish_state_for_relay(relay, next, &RelaySourceState::Connecting);
        self.publish_relay_diagnostic(relay, next);
        false
    }

    /// What this slot's live connection has proved to it right now.
    ///
    /// An event admitted while nothing has been proved, or while an answer is
    /// still in flight or was refused, travels as unauthenticated: only a
    /// relay verdict of `Authenticated` is a fact this relay actually acted
    /// on.
    fn current_authority(&self, relay: &RelayUrl, generation: Round) -> Authority {
        self.slot(relay, generation)
            .and_then(|slot| slot.session.as_ref())
            .map_or(Authority::Unauthenticated, |session| {
                RelaySessionExt::connection(session)
                    .borrow()
                    .authentication
                    .established
                    .map_or(Authority::Unauthenticated, Authority::As)
            })
    }

    /// One event the relay attributed to one installed subscription.
    fn carried_event(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        subscription: &SubscriptionId,
        event: nostr::event::Event,
    ) -> bool {
        let Some(entry) = self
            .slot(relay, generation)
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
            &self.current_authority(relay, generation),
            &accepted,
            subscription,
            event,
            Timestamp::now(),
        ) else {
            return false;
        };
        let owners = self
            .slot(relay, generation)
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
    fn stored_complete(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        id: &SubscriptionId,
    ) -> bool {
        let at = Timestamp::now();
        let proves = self
            .slot(relay, generation)
            .map(|slot| slot.proves_completeness(id))
            .unwrap_or_default();
        if let Some(slot) = self.slot_mut(relay, generation) {
            slot.settled.insert(id.clone(), true);
        }
        if proves == fava_subscriptions::EoseCompleteness::Proven {
            let owners = self
                .slot(relay, generation)
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
                generation,
                id,
                &RelaySourceState::StoredEventsComplete { at },
            );
        } else {
            // The relay ended a bounded request, not the stored window.
            // Claiming completeness would claim omitted work was completed
            // (GOALS:1066).
            self.publish_shortfall(relay, generation, id, proves);
        }
        self.publish_relay_diagnostic(relay, generation);
        false
    }

    /// The relay refused or ended one installed subscription, in its own words.
    fn subscription_refused(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        id: &SubscriptionId,
        message: &BoundedText,
    ) -> bool {
        let at = Timestamp::now();
        self.publish_for_subscription(
            relay,
            generation,
            id,
            &RelaySourceState::Refused {
                message: message.clone(),
                at,
            },
        );
        self.publish_relay_diagnostic(relay, generation);
        false
    }
}
