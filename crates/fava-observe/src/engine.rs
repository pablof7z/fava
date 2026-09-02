//! Reconciliation of logical demand into immutable relay work.
//!
//! One reconciliation owner per engine, one slot per relay connection, one
//! transport lease per slot — so a second observation at a connection Fava
//! already holds reuses it and never dials (`GOALS:936`). A relay may need
//! more than one connection at once: which slot serves a piece of work is a
//! live question asked of that slot's connection (can it still reach the
//! authority the work needs?), never a key a demand item is filed under. A
//! relay's slots therefore live in a `Vec`, scanned by reachability, exactly
//! as the transport's own connection registry is.
//!
//! Demand is never merged by this owner. New demand that no live request
//! already covers enters a per-relay pending cohort behind one fixed,
//! first-arrival-anchored window; at the window's edge the cohort is frozen and
//! handed to the planner **together with everything the transport actually
//! accepted**, because a planner that could not see the incumbents would have
//! to reopen them. The merge step still cannot widen a request that has already
//! reached the wire: the planner removes covered demand before it groups
//! anything, so an incumbent is never an operand of a merge. Demand arriving
//! after the freeze attaches to a covering incumbent or opens its own request
//! beside it.
//!
//! The refcount that decides withdrawal is the attribution fan-out on each live
//! request: it closes when, and only when, the last demand it serves goes away.
//! A survivor keeps its over-broad filter; the surplus is discarded by local
//! query evaluation.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_query::{BoundedText, RelaySourceState, Round, RoundIssuer, RoundsExhausted};
use fava_relay::Authority;
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_subscriptions::{
    InstalledSubscription, InstalledSubscriptions, PlanRevision, PlanRevisionExhausted,
    PlanRevisionIssuer, RelayDemand, RelayReadConstraints, SubscriptionPlan, SubscriptionPlanner,
    filter_covers, validate_plan,
};
use fava_transport::{
    OpenRelaySession, RelaySession, RelaySessionLease, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_wire::SubscriptionId;
use nostr::types::RelayUrl;

use crate::diagnostics;
use crate::operations;
use crate::registry::Registry;
use crate::slot::Slot;

/// Bounded report queue between provider work and the reconciliation owner.
const REPORTS: usize = 1_024;

/// Providers the observation owner uses to execute relay work.
#[derive(Clone)]
pub(crate) struct RelayProviders {
    pub(crate) transport: Arc<dyn Transport>,
    pub(crate) planner: Arc<dyn SubscriptionPlanner>,
    pub(crate) cache: Arc<dyn EventCache>,
    pub(crate) diagnostics: Arc<Diagnostics>,
    pub(crate) deadlines: TransportDeadlines,
    pub(crate) bounds: TransportBounds,
    pub(crate) admission_window: Duration,
}

/// Bounded sender the provider tasks report completions through.
#[derive(Clone)]
pub(crate) struct Reports {
    sender: tokio::sync::mpsc::Sender<Report>,
}

impl Reports {
    /// Deliver one completion, applying backpressure rather than dropping it.
    pub(crate) async fn send(&self, report: Report) {
        let _ = self.sender.send(report).await;
    }
}

/// One provider completion, always carrying the generation it was issued under.
///
/// The generation alone finds the exact slot a report belongs to: it is
/// engine-wide monotonic and reused by nothing, including a slot's own
/// reconnect, which mints a fresh one. A relay's other slots, and its slots'
/// own earlier generations, never match.
pub(crate) enum Report {
    Acquired {
        relay: RelayUrl,
        generation: Round,
        lease: Box<RelaySessionLease>,
    },
    Refused {
        relay: RelayUrl,
        generation: Round,
        detail: BoundedText,
    },
    Installed {
        relay: RelayUrl,
        generation: Round,
        revision: fava_subscriptions::PlanRevision,
        plan: Box<SubscriptionPlan>,
        opened: Vec<Option<SubscriptionId>>,
        /// The token governing each opened subscription's handle. Cancelling it
        /// drops the handle, which sends the relay its CLOSE.
        attending: Vec<(SubscriptionId, CancellationToken)>,
        closed: BTreeSet<SubscriptionId>,
    },
    Flush {
        relay: RelayUrl,
        generation: Round,
    },
    /// One session's connection state changed.
    Connection {
        relay: RelayUrl,
        generation: Round,
        state: Box<fava_transport::Connection>,
    },
    /// The connection carrying this relay's work was replaced.
    ConnectionReplaced {
        relay: RelayUrl,
        generation: Round,
    },
    /// The connection will never reach another state. The state it last
    /// reached already said why; this only ends the waiting.
    ConnectionEnded,
    /// One installed subscription carried something of its own.
    Carried {
        relay: RelayUrl,
        generation: Round,
        subscription: SubscriptionId,
        item: Box<fava_transport::SubscriptionItem>,
    },
    /// NIP-11 relay-information document fetched for one relay.
    Constraints {
        relay: RelayUrl,
        generation: Round,
        constraints: RelayReadConstraints,
    },
}

/// Single reconciliation owner for one engine instance.
pub(crate) struct Engine {
    pub(crate) registry: Arc<Registry>,
    pub(crate) providers: RelayProviders,
    pub(crate) runtime: Runtime,
    pub(crate) root: CancellationToken,
    pub(crate) reports: Reports,
    pub(crate) inbox: tokio::sync::mpsc::Receiver<Report>,
    /// Every slot this owner holds, by relay. A relay may need more than one
    /// connection at once; which one serves a piece of demand is decided by
    /// asking each slot whether it can still reach the authority that demand
    /// needs (`Slot::can_serve`), never by a key the demand is filed under.
    pub(crate) slots: BTreeMap<RelayUrl, Vec<Slot>>,
    /// Source of every round this owner issues.
    pub(crate) rounds: RoundIssuer,
    /// Source of every plan revision this owner issues, for every relay.
    ///
    /// Wire identity is minted from a revision, so a revision reused inside
    /// one transport session hands a reopened subscription the identity of a
    /// closed one — which `GOALS:426` (QUERY-010) forbids by name. The counter
    /// therefore lives here rather than in [`Slot`]: a slot is released the
    /// moment its relay's demand drains, while the socket behind it survives
    /// for as long as any other lease holder wants it, and the standard
    /// assembly gives publication a lease on the very same session key.
    /// Engine-wide monotonicity is stronger than the promise needs and costs
    /// one `u64` for the life of the engine.
    pub(crate) plan_revisions: PlanRevisionIssuer,
}

impl Engine {
    /// Start the reconciliation owner on the runtime.
    pub(crate) fn start(
        registry: Arc<Registry>,
        providers: RelayProviders,
        runtime: &Runtime,
    ) -> Result<(), crate::ObserveError> {
        let (sender, inbox) = tokio::sync::mpsc::channel(REPORTS);
        let root = runtime.cancellation_token();
        let engine = Self {
            registry,
            providers,
            runtime: runtime.clone(),
            root: root.clone(),
            reports: Reports { sender },
            inbox,
            slots: BTreeMap::new(),
            rounds: RoundIssuer::new()?,
            plan_revisions: PlanRevisionIssuer::new()?,
        };
        runtime
            .spawn_cancellable(TaskName("observe.engine"), root, engine.run())
            .map(|_| ())
            .map_err(|_| crate::ObserveError::EngineClosed)
    }

    async fn run(mut self) {
        let mut demand = self.registry.changes();
        let mut dirty = true;
        loop {
            if std::mem::replace(&mut dirty, false) {
                self.reconcile();
            }
            tokio::select! {
                biased;
                changed = demand.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    dirty = true;
                }
                report = self.inbox.recv() => {
                    let Some(report) = report else {
                        break;
                    };
                    dirty = self.accept(report);
                }
            }
        }
        for (_, bucket) in std::mem::take(&mut self.slots) {
            for slot in bucket {
                slot.cancel.cancel();
            }
        }
    }

    /// Bring relay work into agreement with current demand, without ever
    /// rewriting a request that has already reached the wire.
    ///
    /// Withdrawal flushes immediately: a CLOSE costs the relay nothing and the
    /// subscription slot it frees is budget a refused demand is waiting for.
    /// New demand waits for its admission window, because grouping has nothing
    /// to group until a cohort exists.
    fn reconcile(&mut self) {
        let desired = self.registry.desired();

        // A relay no longer named by any demand loses every slot it holds.
        let vacated: Vec<RelayUrl> = self
            .slots
            .keys()
            .filter(|relay| !desired.contains_key(*relay))
            .cloned()
            .collect();
        for relay in vacated {
            self.release_relay(&relay);
        }

        for (relay, items) in &desired {
            // Every authority this relay's demand currently needs must be
            // reachable from some slot; open one for whichever is not.
            for (item, authority) in items {
                if self.slot_index_for(relay, authority).is_none() {
                    self.establish(relay, authority, std::slice::from_ref(item));
                }
            }

            let generations: Vec<Round> = self
                .slots
                .get(relay)
                .map(|bucket| bucket.iter().map(|slot| slot.generation).collect())
                .unwrap_or_default();
            for generation in generations {
                let demand = self.demand_for_slot(relay, generation);
                // Nothing wants this slot at all: release it outright, the
                // same way a relay with no desired demand is released below.
                // `release_slot` cancels its token, which drops every
                // subscription handle it held and sends each one's own CLOSE
                // on the way out — there is no completion to wait for.
                if demand.is_empty() {
                    self.release_slot(relay, generation);
                    continue;
                }
                self.attach(relay, generation, &demand);
                let Some(slot) = self.slot(relay, generation) else {
                    continue;
                };
                if slot.orphaned(&demand) {
                    self.flush(relay, generation);
                    continue;
                }
                if slot.uncovered(&demand).is_empty() || slot.armed {
                    continue;
                }
                let Some(slot) = self.slot_mut(relay, generation) else {
                    continue;
                };
                slot.armed = true;
                self.arm(relay, generation);
            }
        }
    }

    /// Bind demand a running request already carries to that request.
    ///
    /// This is a refcount edit, not wire work: the subscription keeps its exact
    /// id and its exact filters, and no plan is computed. A joiner attaching to
    /// a request whose stored replay already ended is credited that completion
    /// straight away — the rows are in the local store its own sources read.
    fn attach(&mut self, relay: &RelayUrl, generation: Round, demand: &[RelayDemand]) {
        let mut credited = Vec::new();
        {
            let Some(slot) = self.slot_mut(relay, generation) else {
                return;
            };
            let mut entries: Vec<(SubscriptionId, InstalledSubscription)> = slot
                .installed
                .ids()
                .filter_map(|id| {
                    slot.installed
                        .get(id)
                        .map(|entry| (id.clone(), entry.clone()))
                })
                .collect();
            let mut changed = false;
            for item in demand {
                let id = item.id();
                if entries.iter().any(|(_, entry)| entry.serves.contains(&id)) {
                    continue;
                }
                let Some((wire, entry)) = entries.iter_mut().find(|(_, entry)| {
                    entry
                        .filters
                        .iter()
                        .any(|filter| filter_covers(filter, &item.filter))
                }) else {
                    continue;
                };
                entry.serves.insert(id);
                changed = true;
                if slot.settled.get(&*wire).copied().unwrap_or_default() {
                    credited.push(id);
                }
            }
            if !changed {
                return;
            }
            slot.installed = InstalledSubscriptions::from_entries(entries);
        }
        self.publish_plan(relay, generation, demand, None);
        for id in credited {
            self.registry.record_state(
                id.owner,
                relay,
                Some(generation),
                RelaySourceState::StoredEventsComplete {
                    at: nostr::types::Timestamp::now(),
                },
            );
        }
        self.publish_relay_diagnostic(relay, generation);
    }

    /// Acquire the relay connection needed to reach `authority`, once.
    ///
    /// Returns the generation of the slot now responsible for it, whether
    /// freshly opened or already in flight. `None` only when the round
    /// counter itself is exhausted.
    fn establish(
        &mut self,
        relay: &RelayUrl,
        authority: &Authority,
        demand: &[RelayDemand],
    ) -> Option<Round> {
        let index = if let Some(index) = self.slot_index_for(relay, authority) {
            index
        } else {
            let generation = match self.next_round() {
                Ok(generation) => generation,
                Err(error) => {
                    self.publish_owner_refusal(relay, demand, None, &error);
                    return None;
                }
            };
            let bucket = self.slots.entry(relay.clone()).or_default();
            bucket.push(Slot::new(self.root.child(), generation, *authority));
            bucket.len() - 1
        };
        let generation;
        {
            let bucket = self
                .slots
                .get_mut(relay)
                .expect("the slot was just found or inserted above");
            let slot = &mut bucket[index];
            if slot.busy || slot.session.is_some() {
                return Some(slot.generation);
            }
            slot.busy = true;
            slot.state = fava_diagnostics::RelaySessionState::Connecting;
            generation = slot.generation;
            operations::acquire(
                &self.runtime,
                &self.providers.transport,
                &self.reports,
                OpenRelaySession {
                    relay: relay.clone(),
                    authority: *authority,
                    deadlines: self.providers.deadlines,
                    bounds: self.providers.bounds,
                    reconnect_attempts: None,
                },
                generation,
                slot.cancel.clone(),
            );
        }
        self.publish_states(relay, demand, generation, &RelaySourceState::Connecting);
        Some(generation)
    }

    /// The next plan revision, never reused for the life of this engine.
    fn next_revision(&mut self) -> Result<PlanRevision, PlanRevisionExhausted> {
        self.plan_revisions.allocate()
    }

    pub(crate) fn next_round(&mut self) -> Result<Round, RoundsExhausted> {
        self.rounds.allocate()
    }

    /// Report a refusal from this owner itself — the round or revision
    /// counter is exhausted — rather than from any provider.
    ///
    /// `generation` names the slot being retired when one already existed;
    /// `None` when no slot was ever created for this demand.
    pub(crate) fn publish_owner_refusal(
        &self,
        relay: &RelayUrl,
        demand: &[RelayDemand],
        generation: Option<Round>,
        error: &impl std::fmt::Display,
    ) {
        let state = RelaySourceState::OwnerRefused {
            detail: BoundedText::new(error.to_string()),
        };
        match generation {
            Some(generation) => self.publish_states(relay, demand, generation, &state),
            None => {
                for item in demand {
                    self.registry
                        .record_state(item.owner, relay, None, state.clone());
                }
            }
        }
    }

    /// Arm one fixed, first-arrival-anchored admission window.
    pub(crate) fn arm(&self, relay: &RelayUrl, generation: Round) {
        operations::arm_admission(
            &self.runtime,
            &self.reports,
            relay.clone(),
            generation,
            self.providers.admission_window,
        );
    }

    /// Close the admission window and plan this relay session once.
    ///
    /// The planner receives the *complete* current demand this slot can serve
    /// and the plan the transport actually accepted, and answers with the
    /// diff. It is the only component that decides grouping, wire identity,
    /// and which running subscription has lost its last logical owner.
    pub(crate) fn flush(&mut self, relay: &RelayUrl, generation: Round) -> bool {
        let demand = self.demand_for_slot(relay, generation);
        let session;
        let installed;
        {
            let Some(slot) = self.slot_mut(relay, generation) else {
                return false;
            };
            slot.armed = false;
            // Demand cancelled before the deadline never reaches the planner.
            if slot.uncovered(&demand).is_empty() && !slot.orphaned(&demand) {
                return false;
            }
            let Some(live) = slot.session.clone() else {
                slot.armed = true;
                self.arm(relay, generation);
                return false;
            };
            session = live;
            installed = slot.installed.clone();
        }
        let revision = match self.next_revision() {
            Ok(revision) => revision,
            Err(error) => {
                self.publish_owner_refusal(relay, &demand, Some(generation), &error);
                return false;
            }
        };
        let constraints =
            self.slot_mut(relay, generation)
                .map_or_else(RelayReadConstraints::unknown, |slot| {
                    // The last revision this slot issued, so a completion for an
                    // earlier one is refused rather than installed.
                    slot.revision = Some(revision);
                    slot.constraints
                });
        let planned = self
            .providers
            .planner
            .plan(relay, &demand, &constraints, &installed, revision)
            .and_then(|plan| {
                validate_plan(relay, &demand, &constraints, &installed, &plan)
                    .map(|()| plan)
                    .map_err(|error| {
                        fava_subscriptions::SubscriptionPlanError::Encoding(
                            fava_transport::BoundedText::new(error.to_string()),
                        )
                    })
            });
        match planned {
            Ok(planned) => self.execute(relay, generation, &demand, &session, &planned),
            Err(error) => {
                self.providers.diagnostics.relay(diagnostics::refused_plan(
                    relay,
                    BoundedText::new(error.to_string()),
                ));
            }
        }
        false
    }

    /// Hand the plan's diff to the wire, opening before closing.
    fn execute(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        demand: &[RelayDemand],
        session: &Arc<dyn RelaySession>,
        planned: &SubscriptionPlan,
    ) {
        {
            let Some(slot) = self.slot_mut(relay, generation) else {
                return;
            };
            for id in planned.attribution.ids() {
                if let Some(entry) = planned.attribution.get(id) {
                    slot.completeness.insert(id.clone(), entry.completeness);
                }
            }
        }
        if planned.is_noop() {
            self.record_installed(relay, generation, planned, &[], &BTreeSet::new());
            self.publish_plan(relay, generation, demand, Some(planned));
            self.publish_relay_diagnostic(relay, generation);
            return;
        }
        let Some(slot) = self.slot(relay, generation) else {
            return;
        };
        operations::install_plan(
            &self.runtime,
            &self.reports,
            operations::Installing {
                relay: relay.clone(),
                generation,
                revision: planned.revision,
            },
            Arc::clone(session),
            planned.clone(),
            self.providers.deadlines.write,
            slot.cancel.clone(),
        );
        self.publish_relay_diagnostic(relay, generation);
    }

    /// Replace this slot's installed-subscription baseline with exactly what the
    /// transport accepted.
    ///
    /// On a plan the transport accepted in full this is
    /// `fava_subscriptions_testkit::apply_plan`, which the owner's own evidence
    /// asserts against. It differs only when a REQ was refused: the successor
    /// never opened, so its predecessor stays live and no CLOSE was sent.
    pub(crate) fn record_installed(
        &mut self,
        relay: &RelayUrl,
        generation: Round,
        planned: &SubscriptionPlan,
        opened: &[Option<SubscriptionId>],
        closed: &BTreeSet<SubscriptionId>,
    ) {
        let Some(slot) = self.slot_mut(relay, generation) else {
            return;
        };
        slot.installed = crate::plan::accepted(&slot.installed, planned, opened, closed);
        let live: BTreeSet<SubscriptionId> = slot.installed.ids().cloned().collect();
        slot.settled.retain(|id, _| live.contains(id));
        slot.completeness.retain(|id, _| live.contains(id));
    }

    /// Index, within this relay's slots, of one whose connection can still
    /// reach `authority`.
    pub(crate) fn slot_index_for(&self, relay: &RelayUrl, authority: &Authority) -> Option<usize> {
        slot_index_for(&self.slots, relay, authority)
    }

    pub(crate) fn slot(&self, relay: &RelayUrl, generation: Round) -> Option<&Slot> {
        slot(&self.slots, relay, generation)
    }

    pub(crate) fn slot_mut(&mut self, relay: &RelayUrl, generation: Round) -> Option<&mut Slot> {
        slot_mut(&mut self.slots, relay, generation)
    }

    /// Current demand this exact slot can serve, out of everything desired at
    /// this relay. Derived fresh every time, from the slot's live reachability
    /// against every current authority requirement — never stored, so a slot
    /// that stops being able to reach some of what it once served simply stops
    /// being handed that demand on the next reconcile pass.
    ///
    /// Assignment is exclusive, not merely reachable: an item goes to the
    /// *first* slot (in this relay's stable order) that can serve it, the
    /// same resolution `establish` itself uses. Filtering each slot only by
    /// its own `can_serve` would let two slots that are both still reachable
    /// for the same authority — a freshly opened, uncommitted connection
    /// reaches everyone, same as any other — both claim the same demand.
    ///
    /// Assignment is sticky once made: an item already attached to this
    /// slot's installed subscriptions stays here regardless of any later
    /// drift in live reachability — a relay's own unanswered challenge, for
    /// instance, stops a connection from being *chosen* for fresh anonymous
    /// work without evicting anonymous work already riding it. Only an item
    /// not yet attached anywhere is routed by live `can_serve`, and then
    /// exclusively: the *first* slot (in this relay's stable order) that can
    /// serve it, the same resolution `establish` itself uses. Filtering each
    /// slot only by its own `can_serve` would let two slots both still
    /// reachable for the same authority — a freshly opened, uncommitted
    /// connection reaches everyone, same as any other — both claim it.
    pub(crate) fn demand_for_slot(&self, relay: &RelayUrl, generation: Round) -> Vec<RelayDemand> {
        let Some(slot) = self.slot(relay, generation) else {
            return Vec::new();
        };
        let attached: BTreeSet<fava_subscriptions::DemandId> = slot
            .installed
            .ids()
            .filter_map(|id| slot.installed.get(id))
            .flat_map(|entry| entry.serves.iter().copied())
            .collect();
        self.registry
            .desired()
            .remove(relay)
            .unwrap_or_default()
            .into_iter()
            .filter(|(item, authority)| {
                attached.contains(&item.id())
                    || slot_index_for(&self.slots, relay, authority)
                        .and_then(|index| self.slots.get(relay)?.get(index))
                        .is_some_and(|candidate| candidate.generation == generation)
            })
            .map(|(item, _)| item)
            .collect()
    }

    /// Withdraw every live request and release every slot this relay holds.
    pub(crate) fn release_relay(&mut self, relay: &RelayUrl) {
        let Some(bucket) = self.slots.remove(relay) else {
            return;
        };
        for slot in bucket {
            self.teardown_slot(slot);
        }
        self.providers.diagnostics.forget_relay(relay);
    }

    /// Withdraw and release exactly one of this relay's slots.
    pub(crate) fn release_slot(&mut self, relay: &RelayUrl, generation: Round) {
        let Some(bucket) = self.slots.get_mut(relay) else {
            return;
        };
        let Some(index) = bucket.iter().position(|slot| slot.generation == generation) else {
            return;
        };
        let slot = bucket.remove(index);
        let emptied = bucket.is_empty();
        if emptied {
            self.slots.remove(relay);
            self.providers.diagnostics.forget_relay(relay);
        }
        self.teardown_slot(slot);
    }

    fn teardown_slot(&self, mut slot: Slot) {
        // Cancelling the slot drops every subscription handle it held, and each
        // handle sends the relay its own CLOSE on the way out.
        slot.cancel.cancel();
        let generation = slot.generation;
        if let Some(lease) = slot.lease.take() {
            self.release_lease(lease, generation);
        }
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

/// Index, within this relay's slots, of one whose connection can still reach
/// `authority`.
pub(crate) fn slot_index_for(
    slots: &BTreeMap<RelayUrl, Vec<Slot>>,
    relay: &RelayUrl,
    authority: &Authority,
) -> Option<usize> {
    slots
        .get(relay)?
        .iter()
        .position(|slot| slot.can_serve(authority))
}

/// Index, within this relay's slots, of the one stamped with this exact
/// generation.
fn slot_index_by_generation(
    slots: &BTreeMap<RelayUrl, Vec<Slot>>,
    relay: &RelayUrl,
    generation: Round,
) -> Option<usize> {
    slots
        .get(relay)?
        .iter()
        .position(|slot| slot.generation == generation)
}

pub(crate) fn slot<'a>(
    slots: &'a BTreeMap<RelayUrl, Vec<Slot>>,
    relay: &RelayUrl,
    generation: Round,
) -> Option<&'a Slot> {
    let index = slot_index_by_generation(slots, relay, generation)?;
    slots.get(relay)?.get(index)
}

pub(crate) fn slot_mut<'a>(
    slots: &'a mut BTreeMap<RelayUrl, Vec<Slot>>,
    relay: &RelayUrl,
    generation: Round,
) -> Option<&'a mut Slot> {
    let index = slot_index_by_generation(slots, relay, generation)?;
    slots.get_mut(relay)?.get_mut(index)
}
