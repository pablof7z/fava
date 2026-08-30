//! Reconciliation of logical demand into immutable relay work.
//!
//! One reconciliation owner per engine, one slot per relay session, one
//! transport lease per slot — so a second observation at a relay Fava already
//! holds reuses the connection and never dials (`GOALS:936`).
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
use fava_query::{
    BoundedText, OperationGeneration, OperationGenerationExhausted, OperationGenerationIssuer,
    RelaySourceState,
};
use fava_relay::RelaySessionKey;
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_subscriptions::{
    InstalledSubscription, InstalledSubscriptions, PlanRevision, PlanRevisionExhausted,
    PlanRevisionIssuer, RelayDemand, RelayReadConstraints, SubscriptionPlan, SubscriptionPlanner,
    filter_covers, validate_plan,
};
use fava_transport::{
    OpenRelaySession, RelayInbound, RelaySession, RelaySessionLease, Transport, TransportBounds,
    TransportDeadlines,
};
use fava_wire::SubscriptionId;

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
pub(crate) enum Report {
    Acquired {
        relay: RelaySessionKey,
        generation: OperationGeneration,
        lease: Box<RelaySessionLease>,
    },
    Refused {
        relay: RelaySessionKey,
        generation: OperationGeneration,
        detail: BoundedText,
    },
    Installed {
        relay: RelaySessionKey,
        generation: OperationGeneration,
        revision: fava_subscriptions::PlanRevision,
        plan: Box<SubscriptionPlan>,
        opened: BTreeSet<SubscriptionId>,
        closed: BTreeSet<SubscriptionId>,
    },
    Flush {
        relay: RelaySessionKey,
        generation: OperationGeneration,
    },
    Inbound {
        relay: RelaySessionKey,
        generation: OperationGeneration,
        item: Box<RelayInbound>,
    },
    /// NIP-11 relay-information document fetched for one relay.
    Constraints {
        relay: RelaySessionKey,
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
    pub(crate) slots: BTreeMap<RelaySessionKey, Slot>,
    /// Source of every operation generation this owner issues.
    pub(crate) operation_generations: OperationGenerationIssuer,
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
            operation_generations: OperationGenerationIssuer::new()?,
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
        for (_, slot) in std::mem::take(&mut self.slots) {
            slot.cancel.cancel();
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
        let removed: Vec<RelaySessionKey> = self
            .slots
            .keys()
            .filter(|relay| !desired.contains_key(*relay))
            .cloned()
            .collect();
        for relay in removed {
            self.release(&relay);
        }
        for (relay, demand) in desired {
            self.establish(&relay, &demand);
            self.attach(&relay, &demand);
            let Some(slot) = self.slots.get(&relay) else {
                continue;
            };
            let generation = slot.generation;
            if slot.orphaned(&demand) {
                self.flush(&relay, generation);
                continue;
            }
            if slot.uncovered(&demand).is_empty() {
                continue;
            }
            let Some(slot) = self.slots.get_mut(&relay) else {
                continue;
            };
            if slot.armed {
                continue;
            }
            slot.armed = true;
            self.arm(&relay, generation);
        }
    }

    /// Bind demand a running request already carries to that request.
    ///
    /// This is a refcount edit, not wire work: the subscription keeps its exact
    /// id and its exact filters, and no plan is computed. A joiner attaching to
    /// a request whose stored replay already ended is credited that completion
    /// straight away — the rows are in the local store its own sources read.
    fn attach(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        let mut credited = Vec::new();
        let generation;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return;
            };
            generation = slot.generation;
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
        self.publish_plan(relay, demand, None);
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
        self.publish_relay_diagnostic(relay);
    }

    /// Acquire the relay session this slot needs, once.
    fn establish(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        if !self.slots.contains_key(relay) {
            let generation = match self.next_operation_generation() {
                Ok(generation) => generation,
                Err(error) => {
                    self.publish_owner_refusal(relay, demand, &error);
                    return;
                }
            };
            self.slots
                .insert(relay.clone(), Slot::new(self.root.child(), generation));
        }
        let generation;
        {
            let slot = self.slots.get_mut(relay).expect("slot was inserted above");
            if slot.busy || slot.session.is_some() {
                return;
            }
            slot.busy = true;
            slot.state = fava_diagnostics::RelaySessionState::Connecting;
            generation = slot.generation;
            operations::acquire(
                &self.runtime,
                &self.providers.transport,
                &self.reports,
                OpenRelaySession {
                    key: relay.clone(),
                    deadlines: self.providers.deadlines,
                    bounds: self.providers.bounds,
                    reconnect_attempts: None,
                },
                generation,
                slot.cancel.clone(),
            );
        }
        self.publish_states(relay, demand, generation, &RelaySourceState::Connecting);
    }
    /// The next plan revision, never reused for the life of this engine.
    fn next_revision(&mut self) -> Result<PlanRevision, PlanRevisionExhausted> {
        self.plan_revisions.allocate()
    }

    pub(crate) fn next_operation_generation(
        &mut self,
    ) -> Result<OperationGeneration, OperationGenerationExhausted> {
        self.operation_generations.allocate()
    }

    pub(crate) fn publish_owner_refusal(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        error: &impl std::fmt::Display,
    ) {
        let state = RelaySourceState::OwnerRefused {
            detail: BoundedText::new(error.to_string()),
        };
        if let Some(slot) = self.slots.get(relay) {
            self.publish_states(relay, demand, slot.generation, &state);
        } else {
            for item in demand {
                self.registry
                    .record_state(item.owner, relay, None, state.clone());
            }
        }
    }

    /// Arm one fixed, first-arrival-anchored admission window.
    pub(crate) fn arm(&self, relay: &RelaySessionKey, generation: OperationGeneration) {
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
    /// The planner receives the *complete* current demand for the session and
    /// the plan the transport actually accepted, and answers with the diff. It
    /// is the only component that decides grouping, wire identity, and which
    /// running subscription has lost its last logical owner.
    pub(crate) fn flush(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
    ) -> bool {
        let demand = self.registry.desired().remove(relay).unwrap_or_default();
        let session;
        let installed;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            if slot.generation != generation {
                return false;
            }
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
                self.publish_owner_refusal(relay, &demand, &error);
                return false;
            }
        };
        let constraints =
            self.slots
                .get_mut(relay)
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
                            fava_transport::BoundedReason::new(error.to_string()),
                        )
                    })
            });
        match planned {
            Ok(planned) => self.execute(relay, &demand, &session, &planned),
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
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        session: &Arc<dyn RelaySession>,
        planned: &SubscriptionPlan,
    ) {
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return;
            };
            for id in planned.attribution.ids() {
                if let Some(entry) = planned.attribution.get(id) {
                    slot.completeness.insert(id.clone(), entry.completeness);
                }
            }
        }
        if planned.is_noop() {
            self.record_installed(relay, planned, &BTreeSet::new(), &BTreeSet::new());
            self.publish_plan(relay, demand, Some(planned));
            self.publish_relay_diagnostic(relay);
            return;
        }
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        operations::install_plan(
            &self.runtime,
            &self.reports,
            operations::Installing {
                relay: relay.clone(),
                generation: slot.generation,
                revision: planned.revision,
            },
            Arc::clone(session),
            planned.clone(),
            self.providers.deadlines.write,
        );
        self.publish_relay_diagnostic(relay);
    }

    /// Replace this relay's installed-subscription baseline with exactly what the
    /// transport accepted.
    ///
    /// On a plan the transport accepted in full this is
    /// `fava_subscriptions_testkit::apply_plan`, which the owner's own evidence
    /// asserts against. It differs only when a REQ was refused: the successor
    /// never opened, so its predecessor stays live and no CLOSE was sent.
    pub(crate) fn record_installed(
        &mut self,
        relay: &RelaySessionKey,
        planned: &SubscriptionPlan,
        opened: &BTreeSet<SubscriptionId>,
        closed: &BTreeSet<SubscriptionId>,
    ) {
        let Some(slot) = self.slots.get_mut(relay) else {
            return;
        };
        slot.installed = crate::plan::accepted(&slot.installed, planned, opened, closed);
        let live: BTreeSet<SubscriptionId> = slot.installed.ids().cloned().collect();
        slot.settled.retain(|id, _| live.contains(id));
        slot.completeness.retain(|id, _| live.contains(id));
    }
}
