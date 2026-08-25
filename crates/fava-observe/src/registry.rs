//! Installed observation identity, retained logical demand, and scoped relay evidence.
//!
//! The registry owns *which* observations exist, *what* logical demand each one
//! holds at each relay, and *what Fava currently knows* about every relay
//! serving them. It never merges two observations' demand: two equivalent
//! queries are two `DemandId`s with their own bounds, route origin, and
//! evidence (`GOALS:296`, QUERY-002 — sharing is permitted, erasing distinct
//! evidence is not). Merging is the planner's decision, made later, with every
//! logical demand still visible to it.

use std::collections::BTreeMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::sync::Mutex;

use fava_query::{
    DesiredPlanEvidence, ObservationId, OperationGeneration, QueryBranchId, QueryShortfall,
    RelayQueryEvidence, RelayShortfall, RelaySourceState, RouteOrigin, SourceEvent, SourceKind,
    SourceRetraction, SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_relay::RelaySessionKey;
use fava_runtime::{CancellationToken, TaskHandle};
use fava_state::{EventStateMutation, RelayEvent, mutations_for_event};
use fava_subscriptions::{DemandId, RelayDemand};
use tokio::sync::watch;

/// One observation's demand at one relay, with the evidence that follows it.
struct Assigned {
    demand: RelayDemand,
    evidence: RelayQueryEvidence,
}

/// Everything the owner retains for one installed observation.
struct Installed {
    cancel: CancellationToken,
    relays: BTreeMap<RelaySessionKey, Assigned>,
    plan: Option<DesiredPlanEvidence>,
    route_revision: Option<u64>,
    coalesced: u64,
    live: BTreeMap<RelaySessionKey, LiveState>,
    wake: watch::Sender<u64>,
    tasks: Vec<TaskHandle<Option<()>>>,
}

#[derive(Default)]
struct LiveState {
    revision: u64,
    events: BTreeMap<nostr::event::EventId, RelayEvent>,
    retractions: Vec<SourceRetraction>,
    refused: u64,
}

const LIVE_EVENTS_PER_SESSION: NonZeroUsize =
    NonZeroUsize::new(4_096).expect("the live retention bound is nonzero");

#[derive(Default)]
struct State {
    next: u64,
    revision: u64,
    observations: BTreeMap<ObservationId, Installed>,
}

/// Owner of installed observation identity, retained demand, and evidence.
pub(crate) struct Registry {
    state: Mutex<State>,
    demand_changed: watch::Sender<u64>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            state: Mutex::new(State::default()),
            demand_changed: watch::channel(0).0,
        }
    }
}

impl Registry {
    /// Install one observation and return its identity plus its wake input.
    pub(crate) fn install(&self, cancel: CancellationToken) -> Installation {
        let (wake, woken) = watch::channel(0);
        let mut state = self.lock();
        state.next = state.next.saturating_add(1);
        let id = ObservationId::new(
            NonZeroU64::new(state.next).expect("the observation counter starts at one"),
        );
        state.observations.insert(
            id,
            Installed {
                cancel: cancel.clone(),
                relays: BTreeMap::new(),
                plan: None,
                route_revision: None,
                coalesced: 0,
                live: BTreeMap::new(),
                wake,
                tasks: Vec::new(),
            },
        );
        Installation { id, cancel, woken }
    }

    /// Replace the relays one observation demands.
    ///
    /// Evidence survives for relays that stayed; a relay that left is dropped
    /// only when the caller says the demand is gone for good, and is otherwise
    /// retained as withdrawn evidence.
    pub(crate) fn assign(
        &self,
        id: ObservationId,
        branch: QueryBranchId,
        wanted: BTreeMap<RelaySessionKey, (RelayDemand, RouteOrigin)>,
        route_revision: Option<u64>,
        withdrawal: fava_query::RelayWithdrawal,
    ) {
        let mut state = self.lock();
        let Some(installed) = state.observations.get_mut(&id) else {
            return;
        };
        installed.route_revision = route_revision;
        let mut changed = false;
        for (session, assigned) in &mut installed.relays {
            if wanted.contains_key(session) {
                continue;
            }
            changed = true;
            assigned.evidence.state = RelaySourceState::Withdrawn { reason: withdrawal };
        }
        installed
            .relays
            .retain(|session, _| wanted.contains_key(session) || retain_withdrawn(withdrawal));
        installed
            .live
            .retain(|session, _| wanted.contains_key(session));
        for (session, (demand, route)) in wanted {
            installed.live.entry(session.clone()).or_default();
            if let Some(assigned) = installed.relays.get_mut(&session) {
                assigned.demand = demand;
                assigned.evidence.route = route;
            } else {
                changed = true;
                installed.relays.insert(
                    session.clone(),
                    Assigned {
                        demand,
                        evidence: planned_evidence(session, branch, route),
                    },
                );
            }
        }
        let revision = bump(&mut state.revision);
        if let Some(installed) = state.observations.get(&id) {
            installed.wake.send_replace(revision);
        }
        if changed {
            self.demand_changed.send_replace(revision);
        }
    }

    /// Attach one owner-held task. A retained task is never aborted: it
    /// observes the installed cancellation and releases what it owns itself.
    pub(crate) fn attach(&self, id: ObservationId, task: TaskHandle<Option<()>>) {
        let mut state = self.lock();
        if let Some(installed) = state.observations.get_mut(&id) {
            installed.tasks.push(task);
        }
    }

    /// Close one observation: cancel its work and release its demand.
    pub(crate) fn withdraw(&self, id: ObservationId) {
        let mut state = self.lock();
        let Some(installed) = state.observations.remove(&id) else {
            return;
        };
        installed.cancel.cancel();
        installed.wake.send_replace(u64::MAX);
        drop(installed.tasks);
        let revision = bump(&mut state.revision);
        self.demand_changed.send_replace(revision);
    }

    /// The complete current logical demand for every relay, never deduplicated.
    pub(crate) fn desired(&self) -> BTreeMap<RelaySessionKey, Vec<RelayDemand>> {
        let state = self.lock();
        let mut desired: BTreeMap<RelaySessionKey, Vec<RelayDemand>> = BTreeMap::new();
        for installed in state.observations.values() {
            for (session, assigned) in &installed.relays {
                if matches!(assigned.evidence.state, RelaySourceState::Withdrawn { .. }) {
                    continue;
                }
                desired
                    .entry(session.clone())
                    .or_default()
                    .push(assigned.demand.clone());
            }
        }
        desired
    }

    /// Replace one relay's contribution state for one observation.
    pub(crate) fn record_state(
        &self,
        id: ObservationId,
        session: &RelaySessionKey,
        generation: OperationGeneration,
        next: RelaySourceState,
    ) {
        self.update(id, |installed| {
            let Some(assigned) = installed.relays.get_mut(session) else {
                return false;
            };
            let changed =
                assigned.evidence.state != next || assigned.evidence.generation != generation;
            assigned.evidence.generation = generation;
            assigned.evidence.state = next;
            changed
        });
    }

    /// Record which observations share the wire work behind one relay's demand.
    pub(crate) fn record_sharing(
        &self,
        id: ObservationId,
        session: &RelaySessionKey,
        plan_revision: u64,
        shared_with: Vec<ObservationId>,
        shortfall: Option<RelayShortfall>,
    ) {
        self.update(id, |installed| {
            let Some(assigned) = installed.relays.get_mut(session) else {
                return false;
            };
            let changed = assigned.evidence.plan_revision != plan_revision
                || assigned.evidence.shared_with != shared_with
                || assigned.evidence.shortfall != shortfall;
            assigned.evidence.plan_revision = plan_revision;
            assigned.evidence.shared_with = shared_with;
            assigned.evidence.shortfall = shortfall;
            changed
        });
    }

    /// Replace the desired-plan evidence backing one observation's demand.
    pub(crate) fn record_plan(&self, id: ObservationId, plan: DesiredPlanEvidence) {
        self.update(id, |installed| {
            let changed = installed.plan.as_ref() != Some(&plan);
            installed.plan = Some(plan);
            changed
        });
    }

    /// Count current-state revisions superseded before delivery.
    pub(crate) fn record_coalesced(&self, id: ObservationId, dropped: u64) {
        self.update(id, |installed| {
            installed.coalesced = installed.coalesced.saturating_add(dropped);
            false
        });
    }

    /// Atomically apply one admitted relay event to an observation-owned live source.
    pub(crate) fn record_live_event(&self, id: ObservationId, relay_event: RelayEvent) {
        self.update(id, |installed| {
            let session = relay_event.occurrence().session.clone();
            let Some(live) = installed.live.get_mut(&session) else {
                return false;
            };
            let current = live.events.values().cloned().collect::<Vec<_>>();
            let now = relay_event.occurrence().observed_at;
            let mutations = mutations_for_event(&current, relay_event, now);
            if mutations.is_empty() {
                return false;
            }
            let mut next_events = live.events.clone();
            let mut next_retractions = Vec::new();
            for mutation in mutations {
                match mutation {
                    EventStateMutation::Upsert(incoming) => {
                        next_events.insert(incoming.event().id, incoming);
                    }
                    EventStateMutation::Retract {
                        event_id,
                        session: retracted_session,
                        cause,
                    } => {
                        if retracted_session == session && next_events.remove(&event_id).is_some() {
                            next_retractions.push(SourceRetraction::new(event_id, cause));
                        }
                    }
                }
            }
            // Retractions describe only the transition to one live-source
            // revision. Overflow is itself a new refused revision, so it must
            // not republish removals that belonged to the preceding accepted
            // replacement or deletion.
            live.retractions.clear();
            if next_events.len() > LIVE_EVENTS_PER_SESSION.get() {
                live.refused = live.refused.saturating_add(1);
                live.revision = live.revision.saturating_add(1);
                return true;
            }
            live.events = next_events;
            live.retractions = next_retractions;
            live.revision = live.revision.saturating_add(1);
            true
        });
    }

    /// Complete current live relay snapshots for one observation.
    pub(crate) fn live_snapshots(&self, id: ObservationId) -> Vec<SourceSnapshot> {
        let state = self.lock();
        let Some(installed) = state.observations.get(&id) else {
            return Vec::new();
        };
        installed
            .live
            .iter()
            .map(|(session, live)| SourceSnapshot {
                kind: SourceKind::LiveRelay {
                    session: session.clone(),
                },
                revision: SourceRevision(live.revision),
                status: SourceStatus::Open,
                events: live
                    .events
                    .values()
                    .cloned()
                    .map(SourceEvent::Relay)
                    .collect(),
                retractions: live.retractions.clone(),
            })
            .collect()
    }

    /// Current scoped evidence for one observation.
    pub(crate) fn evidence(&self, id: ObservationId) -> ObservationEvidence {
        let state = self.lock();
        let Some(installed) = state.observations.get(&id) else {
            return ObservationEvidence::default();
        };
        ObservationEvidence {
            relays: installed
                .relays
                .values()
                .map(|assigned| assigned.evidence.clone())
                .collect(),
            plan: installed.plan.clone(),
            coalesced: installed.coalesced,
            live_shortfalls: installed
                .live
                .iter()
                .filter(|(_, live)| live.refused > 0)
                .map(|(session, live)| QueryShortfall::LiveRetentionLimit {
                    session: session.clone(),
                    limit: LIVE_EVENTS_PER_SESSION,
                    refused: live.refused,
                })
                .collect(),
            route_revision: installed.route_revision,
        }
    }

    /// Every currently installed observation, ascending.
    pub(crate) fn open_observations(&self) -> Vec<ObservationId> {
        self.lock().observations.keys().copied().collect()
    }

    /// The demand identity one observation holds at one relay.
    pub(crate) fn demand_id(
        &self,
        id: ObservationId,
        session: &RelaySessionKey,
    ) -> Option<DemandId> {
        self.lock()
            .observations
            .get(&id)
            .and_then(|installed| installed.relays.get(session))
            .map(|assigned| assigned.demand.id())
    }

    /// Await a change to the aggregate desired demand.
    pub(crate) fn changes(&self) -> watch::Receiver<u64> {
        self.demand_changed.subscribe()
    }

    fn update(&self, id: ObservationId, change: impl FnOnce(&mut Installed) -> bool) {
        let mut state = self.lock();
        let Some(installed) = state.observations.get_mut(&id) else {
            return;
        };
        if !change(installed) {
            return;
        }
        let revision = bump(&mut state.revision);
        if let Some(installed) = state.observations.get(&id) {
            installed.wake.send_replace(revision);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// What one installed observation received at install time.
pub(crate) struct Installation {
    pub(crate) id: ObservationId,
    pub(crate) cancel: CancellationToken,
    pub(crate) woken: watch::Receiver<u64>,
}

/// The owner-held evidence one observation currently reports.
#[derive(Default)]
pub(crate) struct ObservationEvidence {
    pub(crate) relays: Vec<RelayQueryEvidence>,
    pub(crate) plan: Option<DesiredPlanEvidence>,
    pub(crate) coalesced: u64,
    pub(crate) live_shortfalls: Vec<QueryShortfall>,
    pub(crate) route_revision: Option<u64>,
}

/// A route withdrawal keeps its evidence (QUERY-014); a closing observation
/// keeps nothing, because the handle that would read it is gone.
const fn retain_withdrawn(withdrawal: fava_query::RelayWithdrawal) -> bool {
    matches!(withdrawal, fava_query::RelayWithdrawal::RouteWithdrawn)
}

fn planned_evidence(
    session: RelaySessionKey,
    branch: QueryBranchId,
    route: RouteOrigin,
) -> RelayQueryEvidence {
    RelayQueryEvidence {
        session,
        generation: OperationGeneration(0),
        plan_revision: 0,
        branches: vec![branch],
        state: RelaySourceState::Planned,
        shared_with: Vec::new(),
        shortfall: None,
        route,
    }
}

fn bump(revision: &mut u64) -> u64 {
    *revision = revision.saturating_add(1);
    *revision
}
