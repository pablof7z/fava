//! Installed observation identity, retained logical demand, and scoped relay evidence.
//!
//! The registry owns *which* observations exist, *what* logical demand each one
//! holds at each relay, and *what Fava currently knows* about every relay
//! serving them. It never merges two observations' demand: two equivalent
//! queries are two `DemandId`s with their own bounds, route origin, and
//! evidence (`GOALS:296`, QUERY-002 — sharing is permitted, erasing distinct
//! evidence is not). Merging is the planner's decision, made later, with every
//! logical demand still visible to it.

use std::collections::{BTreeMap, BTreeSet};
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
    public: ObservationId,
    active: bool,
    active_child: Option<ObservationId>,
    children: BTreeSet<ObservationId>,
    relays: BTreeMap<RelaySessionKey, Assigned>,
    plan: Option<DesiredPlanEvidence>,
    route_revision: Option<u64>,
    coalesced: u64,
    live: BTreeMap<RelaySessionKey, LiveState>,
    wake: watch::Sender<u64>,
    tasks: Vec<TaskHandle<Option<()>>>,
}

/// Relay events one observation retains itself because no selected store keeps
/// them, and how many valid transitions the retention bound refused.
#[derive(Default)]
struct LiveState {
    revision: u64,
    events: BTreeMap<nostr::event::EventId, RelayEvent>,
    retractions: Vec<SourceRetraction>,
    refused: u64,
}

const LIVE_EVENTS_PER_SESSION: NonZeroUsize =
    NonZeroUsize::new(4_096).expect("the live retention bound is nonzero");

/// Every installed observation, the counter that mints their identities, and the
/// revision that wakes readers when aggregate demand changes.
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
    /// Exact filter retained for one observation's demand at one source.
    pub(crate) fn filter_for(
        &self,
        id: ObservationId,
        session: &RelaySessionKey,
    ) -> Option<nostr::filter::Filter> {
        self.lock()
            .observations
            .get(&id)
            .and_then(|installed| installed.relays.get(session))
            .map(|assigned| assigned.demand.filter.clone())
    }

    /// Install one application-visible observation.
    pub(crate) fn install(&self, cancel: CancellationToken) -> Installation {
        self.install_for(cancel, None)
            .expect("a root observation has no parent to disappear")
    }

    /// Install one concrete generation for an existing public observation.
    pub(crate) fn install_child(
        &self,
        cancel: CancellationToken,
        public: ObservationId,
    ) -> Option<Installation> {
        self.install_for(cancel, Some(public))
    }

    fn install_for(
        &self,
        cancel: CancellationToken,
        public: Option<ObservationId>,
    ) -> Option<Installation> {
        let (wake, woken) = watch::channel(0);
        let mut state = self.lock();
        if public.is_some_and(|id| !state.observations.contains_key(&id)) {
            return None;
        }
        state.next = state.next.saturating_add(1);
        let id = ObservationId::new(
            NonZeroU64::new(state.next).expect("the observation counter starts at one"),
        );
        state.observations.insert(
            id,
            Installed {
                cancel: cancel.clone(),
                public: public.unwrap_or(id),
                active: public.is_none(),
                active_child: None,
                children: BTreeSet::new(),
                relays: BTreeMap::new(),
                plan: None,
                route_revision: None,
                coalesced: 0,
                live: BTreeMap::new(),
                wake,
                tasks: Vec::new(),
            },
        );
        if let Some(public) = public {
            state
                .observations
                .get_mut(&public)
                .expect("the parent was checked before child installation")
                .children
                .insert(id);
        }
        Some(Installation { id, cancel, woken })
    }

    /// Make one concrete generation current and synchronously retire its predecessor.
    pub(crate) fn activate_child(&self, public: ObservationId, child: ObservationId) -> bool {
        let mut state = self.lock();
        let child_matches = state
            .observations
            .get(&child)
            .is_some_and(|installed| installed.public == public && !installed.active);
        let parent_matches = state
            .observations
            .get(&public)
            .is_some_and(|parent| parent.children.contains(&child));
        if !child_matches || !parent_matches {
            return false;
        }
        let retired = state
            .observations
            .get_mut(&public)
            .expect("the parent was checked above")
            .active_child
            .replace(child);
        state
            .observations
            .get_mut(&child)
            .expect("the child was checked above")
            .active = true;
        if let Some(retired) = retired.filter(|retired| *retired != child) {
            if let Some(parent) = state.observations.get_mut(&public) {
                parent.children.remove(&retired);
            }
            if let Some(installed) = state.observations.remove(&retired) {
                installed.cancel.cancel();
                installed.wake.send_replace(u64::MAX);
                drop(installed.tasks);
            }
        }
        let revision = bump(&mut state.revision);
        self.demand_changed.send_replace(revision);
        true
    }

    /// Execute one publication or delivery while its exact owner remains active.
    pub(crate) fn with_publication_evidence<R>(
        &self,
        public: ObservationId,
        id: ObservationId,
        action: impl FnOnce(&ObservationEvidence) -> R,
    ) -> Option<R> {
        let state = self.lock();
        let installed = state.observations.get(&id)?;
        let publishable = if public == id {
            installed.active
        } else {
            installed.active
                && installed.public == public
                && state
                    .observations
                    .get(&public)
                    .is_some_and(|parent| parent.active_child == Some(id))
        };
        publishable.then(|| action(&evidence_for(&state, installed)))
    }

    /// Translate, sort, and deduplicate live identities for public evidence.
    pub(crate) fn public_ids(
        &self,
        values: impl IntoIterator<Item = ObservationId>,
    ) -> Vec<ObservationId> {
        let state = self.lock();
        let mut values: Vec<_> = values
            .into_iter()
            .filter_map(|id| state.observations.get(&id).map(|entry| entry.public))
            .collect();
        values.sort_unstable();
        values.dedup();
        values
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
        if installed.public != id
            && let Some(parent) = state.observations.get_mut(&installed.public)
        {
            parent.children.remove(&id);
            if parent.active_child == Some(id) {
                parent.active_child = None;
            }
        }
        let children = installed.children.clone();
        installed.cancel.cancel();
        installed.wake.send_replace(u64::MAX);
        drop(installed.tasks);
        for child in children {
            if let Some(child) = state.observations.remove(&child) {
                child.cancel.cancel();
                child.wake.send_replace(u64::MAX);
                drop(child.tasks);
            }
        }
        let revision = bump(&mut state.revision);
        self.demand_changed.send_replace(revision);
    }

    /// The complete current logical demand for every relay, never deduplicated.
    pub(crate) fn desired(&self) -> BTreeMap<RelaySessionKey, Vec<RelayDemand>> {
        let state = self.lock();
        let mut desired: BTreeMap<RelaySessionKey, Vec<RelayDemand>> = BTreeMap::new();
        for installed in state.observations.values() {
            if !installed.active {
                continue;
            }
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

    /// Replace how far one relay has got with one observation's demand.
    pub(crate) fn record_state(
        &self,
        id: ObservationId,
        session: &RelaySessionKey,
        generation: Option<OperationGeneration>,
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

    /// Note which observations share one relay's wire work, under which plan
    /// revision, and what that plan omits.
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

    /// Replace the desired subscription plan backing one observation's demand.
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

    /// Everything one observation currently reports about its relays, its plan, and
    /// what it lost.
    pub(crate) fn evidence(&self, id: ObservationId) -> ObservationEvidence {
        let state = self.lock();
        let Some(installed) = state.observations.get(&id) else {
            return ObservationEvidence::default();
        };
        evidence_for(&state, installed)
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

/// One observation's per-relay progress, the plan and route revision behind it,
/// and the updates and live events it dropped.
#[derive(Default)]
pub(crate) struct ObservationEvidence {
    pub(crate) relays: Vec<RelayQueryEvidence>,
    pub(crate) plan: Option<DesiredPlanEvidence>,
    pub(crate) coalesced: u64,
    pub(crate) live_shortfalls: Vec<QueryShortfall>,
    pub(crate) route_revision: Option<u64>,
}

fn evidence_for(state: &State, installed: &Installed) -> ObservationEvidence {
    let mut relays: Vec<_> = installed
        .relays
        .values()
        .map(|assigned| assigned.evidence.clone())
        .collect();
    for relay in &mut relays {
        relay.shared_with = relay
            .shared_with
            .iter()
            .filter_map(|id| state.observations.get(id).map(|entry| entry.public))
            .collect();
        relay.shared_with.sort_unstable();
        relay.shared_with.dedup();
    }
    ObservationEvidence {
        relays,
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

/// A route withdrawal keeps its evidence (QUERY-014); a closing observation
/// keeps nothing, because the handle that would read it is gone.
const fn retain_withdrawn(withdrawal: fava_query::RelayWithdrawal) -> bool {
    matches!(withdrawal, fava_query::RelayWithdrawal::RouteWithdrawn)
}

/// First report for a relay routing has named but no session has reached yet.
fn planned_evidence(
    session: RelaySessionKey,
    branch: QueryBranchId,
    route: RouteOrigin,
) -> RelayQueryEvidence {
    RelayQueryEvidence {
        session,
        generation: None,
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

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::{Arc, Barrier, mpsc};

    use fava_query::{Query, QueryBranchId, RelayWithdrawal, RouteOrigin};
    use fava_relay::{RelayAccess, RelaySessionKey};
    use fava_runtime::{Runtime, RuntimeConfig};

    use super::Registry;

    #[test]
    fn provisional_children_never_contribute_demand_and_parent_close_removes_all() {
        let registry = Registry::default();
        let runtime = runtime();
        let parent = registry.install(runtime.cancellation_token());
        let first = registry
            .install_child(runtime.cancellation_token(), parent.id)
            .expect("parent exists");
        let second = registry
            .install_child(runtime.cancellation_token(), parent.id)
            .expect("parent exists");
        assign(&registry, first.id);
        assign(&registry, second.id);

        assert!(registry.desired().is_empty());
        assert!(registry.activate_child(parent.id, first.id));
        assert_eq!(registry.desired().values().flatten().count(), 1);

        registry.withdraw(parent.id);
        assert!(registry.desired().is_empty());
        assert!(registry.open_observations().is_empty());
    }

    #[test]
    fn close_cannot_cross_an_active_diagnostic_commit() {
        let registry = Arc::new(Registry::default());
        let runtime = runtime();
        let parent = registry.install(runtime.cancellation_token());
        let child = registry
            .install_child(runtime.cancellation_token(), parent.id)
            .expect("parent exists");
        assert!(registry.activate_child(parent.id, child.id));
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let publisher = {
            let registry = Arc::clone(&registry);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            std::thread::spawn(move || {
                registry.with_publication_evidence(parent.id, child.id, |_| {
                    entered.wait();
                    release.wait();
                })
            })
        };
        entered.wait();
        let (close_done, received) = mpsc::channel();
        let withdrawal = {
            let registry = Arc::clone(&registry);
            std::thread::spawn(move || {
                registry.withdraw(parent.id);
                close_done.send(()).expect("close completion reports");
            })
        };
        assert!(
            received
                .recv_timeout(std::time::Duration::from_millis(20))
                .is_err()
        );
        release.wait();
        assert_eq!(publisher.join().expect("publisher completes"), Some(()));
        withdrawal.join().expect("withdrawal completes");
        assert!(
            registry
                .with_publication_evidence(parent.id, child.id, |_| ())
                .is_none()
        );
    }

    fn assign(registry: &Registry, id: fava_query::ObservationId) {
        let session = RelaySessionKey {
            relay: fava_query::RelayUrl::parse("wss://relay.example").expect("relay URL"),
            access: RelayAccess::Public,
        };
        let query = Query::events()
            .from_relays([session.relay.clone()])
            .expect("one relay is bounded");
        registry.assign(
            id,
            QueryBranchId::ROOT,
            [(
                session,
                (
                    fava_subscriptions::demand_for_query(id, QueryBranchId::ROOT, &query),
                    RouteOrigin::Explicit,
                ),
            )]
            .into_iter()
            .collect(),
            None,
            RelayWithdrawal::ObservationClosed,
        );
    }

    fn runtime() -> Runtime {
        let bound = NonZeroUsize::new(8).expect("nonzero");
        Runtime::new(RuntimeConfig {
            default_channel_depth: bound,
            max_tasks: bound,
            max_provider_operations: bound,
        })
    }
}
