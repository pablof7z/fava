//! Reconciliation of logical demand into immutable relay work.
//!
//! One reconciliation owner per engine, one slot per relay session, one
//! transport lease per slot — so a second observation at a relay Fava already
//! holds reuses the connection and never dials (`GOALS:936`).
//!
//! Demand is never merged by this owner. New demand that no live request
//! already covers enters a per-relay pending cohort behind one fixed,
//! first-arrival-anchored window; at the window's edge the cohort is frozen and
//! compiled by the planner **against an empty incumbent namespace**, so the
//! merge step structurally cannot widen a request that has already reached the
//! wire. Demand arriving after the freeze attaches to a covering incumbent or
//! opens its own request beside it.
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
    BoundedText, OperationGeneration, RelaySourceState,
};
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{
    DemandId, InstalledSubscriptions, PlanRevision, RelayDemand, RelayReadConstraints,
    SubscriptionPlan, SubscriptionPlanner, validate_plan,
};
use fava_transport::{
    OpenRelaySession, RelayInbound, RelaySession, RelaySessionLease, Transport, TransportBounds,
    TransportDeadlines,
};

use crate::admission::{self, LiveSubscription};
use crate::slot::Slot;
use crate::diagnostics;
use crate::operations;
use crate::registry::Registry;

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
    Applied {
        relay: RelaySessionKey,
        generation: OperationGeneration,
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
}

impl Engine {
    /// Start the reconciliation owner on the runtime.
    pub(crate) fn start(
        registry: Arc<Registry>,
        providers: RelayProviders,
        runtime: &Runtime,
    ) -> Result<(), fava_runtime::RuntimeError> {
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
        };
        runtime
            .spawn_cancellable(TaskName("observe.engine"), root, engine.run())
            .map(|_| ())
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
            self.withdraw_departed(&relay, &demand);
            self.admit(&relay, &demand);
            self.establish(&relay, &demand);
        }
    }

    /// Close every request whose last serving demand has gone away.
    fn withdraw_departed(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        let wanted = admission::identities(demand);
        let mut closing = Vec::new();
        let mut rearm = false;
        let generation;
        let session;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return;
            };
            slot.pending.retain(|id, _| wanted.contains(id));
            for (id, entry) in &mut slot.live {
                entry.serves.retain(|held| wanted.contains(held));
                if entry.serves.is_empty() {
                    closing.push(id.clone());
                }
            }
            if closing.is_empty() {
                return;
            }
            for id in &closing {
                slot.live.remove(id);
                slot.retired.insert(id.clone());
            }
            generation = slot.generation;
            session = slot.session.clone();
            // A withdrawal can free relay capacity a refused demand needs.
            if !slot.pending.is_empty() && !slot.armed {
                slot.armed = true;
                rearm = true;
            }
        }
        if let Some(session) = session {
            operations::withdraw_subscriptions(
                &self.runtime,
                &self.reports,
                relay.clone(),
                generation,
                session,
                closing,
                self.providers.deadlines.write,
            );
        }
        if rearm {
            self.arm(relay, generation);
        }
        self.publish_relay_diagnostic(relay);
    }

    /// Attach new demand to a covering request, or hold it for admission.
    fn admit(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        let mut attached = Vec::new();
        let mut arm = false;
        let generation;
        {
            let slot = self
                .slots
                .entry(relay.clone())
                .or_insert_with(|| Slot::new(self.root.child()));
            generation = slot.generation;
            let held = slot.held();
            for item in demand {
                let id = item.id();
                if held.contains(&id) {
                    continue;
                }
                let covering = slot
                    .live
                    .iter()
                    .find(|(_, entry)| admission::attaches(entry, &item.filter))
                    .map(|(wire, entry)| (wire.clone(), entry.stored_events_complete));
                if let Some((wire, complete)) = covering {
                    if let Some(entry) = slot.live.get_mut(&wire) {
                        entry.serves.insert(id);
                    }
                    if complete {
                        attached.push(id);
                    }
                    continue;
                }
                slot.pending.insert(id, item.clone());
                if !slot.armed {
                    slot.armed = true;
                    arm = true;
                }
            }
        }
        // A late joiner missed the stored replay, but the rows are already in
        // the local store its own sources read: credit it the earned fact.
        for id in attached {
            self.registry.record_state(
                id.owner,
                relay,
                generation,
                RelaySourceState::StoredEventsComplete {
                    at: Timestamp::now(),
                },
            );
        }
        if arm {
            self.arm(relay, generation);
        }
    }

    /// Acquire the relay session this slot needs, once.
    fn establish(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        let generation;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return;
            };
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

    /// Freeze the pending cohort and compile it in an empty incumbent namespace.
    pub(crate) fn flush(&mut self, relay: &RelaySessionKey, generation: OperationGeneration) -> bool {
        let cohort;
        let session;
        let revision;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return false;
            };
            if slot.generation != generation {
                return false;
            }
            slot.armed = false;
            if slot.pending.is_empty() {
                return false;
            }
            let Some(live) = slot.session.clone() else {
                // No session yet. The cohort waits; establishment re-arms.
                slot.armed = true;
                self.arm(relay, generation);
                return false;
            };
            slot.revision = PlanRevision(slot.revision.0.saturating_add(1));
            revision = slot.revision;
            cohort = slot.pending.values().cloned().collect::<Vec<_>>();
            session = live;
        }
        let constraints = RelayReadConstraints::unknown();
        // The incumbent namespace is empty by construction: the merge step can
        // see only the cohort, so it cannot widen a request already on the wire.
        let empty = InstalledSubscriptions::empty();
        let planned = self
            .providers
            .planner
            .plan(relay, &cohort, &constraints, &empty, revision)
            .and_then(|plan| {
                validate_plan(relay, &cohort, &constraints, &empty, &plan)
                    .map(|()| plan)
                    .map_err(|error| {
                        fava_subscriptions::SubscriptionPlanError::Encoding(
                            fava_transport::BoundedReason::new(error.to_string()),
                        )
                    })
            });
        match planned {
            Ok(planned) => self.install(relay, &cohort, &session, &planned),
            Err(error) => {
                self.providers.diagnostics.relay(diagnostics::refused_plan(
                    relay,
                    BoundedText::new(error.to_string()),
                ));
            }
        }
        false
    }

    /// Append the cohort's requests beside the incumbents, never over them.
    fn install(
        &mut self,
        relay: &RelaySessionKey,
        cohort: &[RelayDemand],
        session: &Arc<dyn RelaySession>,
        planned: &SubscriptionPlan,
    ) {
        let mut opening = Vec::new();
        let mut settled = Vec::new();
        let mut reused = Vec::new();
        let generation;
        {
            let Some(slot) = self.slots.get_mut(relay) else {
                return;
            };
            generation = slot.generation;
            let mut carried = BTreeSet::new();
            for candidate in &planned.open {
                let serves: BTreeSet<DemandId> = planned
                    .attribution
                    .get(&candidate.id)
                    .map(|entry| entry.serves.clone())
                    .unwrap_or_default();
                carried.extend(serves.iter().copied());
                let covering = slot
                    .live
                    .iter()
                    .find(|(_, entry)| admission::attaches_all(entry, &candidate.filters))
                    .map(|(wire, entry)| (wire.clone(), entry.stored_events_complete));
                if let Some((wire, complete)) = covering {
                    if let Some(entry) = slot.live.get_mut(&wire) {
                        entry.serves.extend(serves.iter().copied());
                    }
                    if complete {
                        settled.extend(serves.iter().copied());
                    }
                    continue;
                }
                if admission::is_retired(&slot.retired, &candidate.id) {
                    reused.push(candidate.id.clone());
                }
                slot.live.insert(
                    candidate.id.clone(),
                    LiveSubscription {
                        filters: candidate.filters.clone(),
                        serves,
                        stored_events_complete: false,
                    },
                );
                opening.push(candidate.clone());
            }
            // Demand the plan carried leaves the cohort; demand it refused for
            // a declared limit stays pending and is retried in a later window.
            slot.pending.retain(|id, _| !carried.contains(id));
        }
        for id in reused {
            self.providers.diagnostics.relay(diagnostics::refused_plan(
                relay,
                BoundedText::new(format!(
                    "subscription planner reused the retired wire id {id}"
                )),
            ));
        }
        for demand in settled {
            self.registry.record_state(
                demand.owner,
                relay,
                generation,
                RelaySourceState::StoredEventsComplete {
                    at: Timestamp::now(),
                },
            );
        }
        self.publish_plan(relay, cohort, planned);
        self.publish_relay_diagnostic(relay);
        if !opening.is_empty() {
            operations::open_subscriptions(
                &self.runtime,
                &self.reports,
                relay.clone(),
                generation,
                Arc::clone(session),
                opening,
                self.providers.deadlines.write,
            );
        }
    }
}
