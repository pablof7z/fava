//! Reconciliation of the complete logical demand into installed relay work.
//!
//! One reconciliation owner per engine. It holds one slot per relay session,
//! and each slot holds exactly one transport lease — so a second observation at
//! a relay Fava already holds reuses the connection and never dials
//! (`GOALS:936`). It never merges demand: the planner receives every
//! observation's demand for the relay and answers with the diff. The refcount
//! that decides when a wire subscription is withdrawn is the planner's
//! attribution, `wire id -> {DemandId}`, so a REQ survives until its last
//! serving demand leaves.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_query::{
    BoundedText, ObservationId, OperationGeneration, RelayDeadline, RelayShortfall,
    RelaySourceState,
};
use fava_runtime::{CancellationToken, Runtime, TaskName};
use fava_state::{RelaySessionKey, Timestamp};
use fava_subscriptions::{
    DemandId, InstalledSubscription, InstalledSubscriptions, PlanRevision, RelayDemand,
    RelayReadConstraints, ShortfallReason, SubscriptionPlan, SubscriptionPlanner, validate_plan,
};
use fava_transport::{
    OpenRelaySession, RelayInbound, RelaySession, RelaySessionLease, Transport, TransportBounds,
    TransportDeadlines, TransportFailure,
};
use fava_wire::SubscriptionId;

use crate::diagnostics;
use crate::ingest;
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
        revision: PlanRevision,
        withdrawn: Vec<SubscriptionId>,
    },
    Inbound {
        relay: RelaySessionKey,
        generation: OperationGeneration,
        item: Box<RelayInbound>,
    },
}

struct Slot {
    generation: OperationGeneration,
    cancel: CancellationToken,
    lease: Option<Box<RelaySessionLease>>,
    session: Option<Arc<dyn RelaySession>>,
    installed: InstalledSubscriptions,
    attribution: BTreeMap<SubscriptionId, Vec<DemandId>>,
    complete: BTreeMap<SubscriptionId, bool>,
    revision: PlanRevision,
    busy: bool,
    state: fava_diagnostics::RelaySessionState,
    reconnects: usize,
}

impl Slot {
    fn new(cancel: CancellationToken) -> Self {
        Self {
            generation: OperationGeneration(1),
            cancel,
            lease: None,
            session: None,
            installed: InstalledSubscriptions::empty(),
            attribution: BTreeMap::new(),
            complete: BTreeMap::new(),
            revision: PlanRevision(0),
            busy: false,
            state: fava_diagnostics::RelaySessionState::Connecting,
            reconnects: 0,
        }
    }

    /// Void everything installed on the previous generation.
    ///
    /// Work already issued is cancelled at its next boundary rather than
    /// aborted, so an operation that produced a provider resource always
    /// reaches the owner and the owner always releases it.
    fn advance(&mut self, root: &CancellationToken) -> OperationGeneration {
        self.cancel.cancel();
        self.cancel = root.child();
        self.installed = InstalledSubscriptions::empty();
        self.attribution.clear();
        self.complete.clear();
        self.busy = false;
        self.generation = self.generation.next();
        self.generation
    }

    fn owners(&self, id: &SubscriptionId) -> Vec<ObservationId> {
        let mut owners: Vec<ObservationId> = self
            .attribution
            .get(id)
            .into_iter()
            .flatten()
            .map(|demand| demand.owner)
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }

    fn serving(&self, demand: DemandId) -> Option<&SubscriptionId> {
        self.attribution
            .iter()
            .find(|(_, served)| served.contains(&demand))
            .map(|(id, _)| id)
    }
}

/// Single reconciliation owner for one engine instance.
pub(crate) struct Engine {
    registry: Arc<Registry>,
    providers: RelayProviders,
    runtime: Runtime,
    root: CancellationToken,
    reports: Reports,
    inbox: tokio::sync::mpsc::Receiver<Report>,
    slots: BTreeMap<RelaySessionKey, Slot>,
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

    /// Bring installed relay work into agreement with the current demand.
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
            self.advance(&relay, &demand);
        }
    }

    /// Advance one relay slot toward its desired plan without blocking.
    fn advance(&mut self, relay: &RelaySessionKey, demand: &[RelayDemand]) {
        let slot = self
            .slots
            .entry(relay.clone())
            .or_insert_with(|| Slot::new(self.root.child()));
        if slot.busy {
            return;
        }
        let Some(session) = slot.session.clone() else {
            slot.busy = true;
            slot.state = fava_diagnostics::RelaySessionState::Connecting;
            let generation = slot.generation;
            let cancel = slot.cancel.clone();
            let request = OpenRelaySession {
                key: relay.clone(),
                deadlines: self.providers.deadlines,
                bounds: self.providers.bounds,
                reconnect_attempts: None,
            };
            operations::acquire(
                &self.runtime,
                &self.providers.transport,
                &self.reports,
                request,
                generation,
                cancel,
            );
            self.publish_states(relay, demand, &RelaySourceState::Connecting);
            return;
        };
        slot.revision = PlanRevision(slot.revision.0.saturating_add(1));
        let revision = slot.revision;
        let constraints = RelayReadConstraints::unknown();
        let planned =
            self.providers
                .planner
                .plan(relay, demand, &constraints, &slot.installed, revision);
        let planned = match planned.and_then(|plan| {
            validate_plan(relay, demand, &constraints, &slot.installed, &plan)
                .map(|()| plan)
                .map_err(|error| {
                    fava_subscriptions::SubscriptionPlanError::Encoding(
                        fava_transport::BoundedReason::new(error.to_string()),
                    )
                })
        }) {
            Ok(planned) => planned,
            Err(error) => {
                self.publish_states(
                    relay,
                    demand,
                    &RelaySourceState::Withdrawn {
                        reason: fava_query::RelayWithdrawal::RouteWithdrawn,
                    },
                );
                self.providers.diagnostics.relay(diagnostics::refused_plan(
                    relay,
                    BoundedText::new(error.to_string()),
                ));
                return;
            }
        };
        self.install(relay, demand, &session, &planned);
    }

    fn install(
        &mut self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        session: &Arc<dyn RelaySession>,
        planned: &SubscriptionPlan,
    ) {
        let Some(slot) = self.slots.get_mut(relay) else {
            return;
        };
        let revision = planned.revision;
        let mut installed = Vec::new();
        let mut attribution = BTreeMap::new();
        for id in planned.installed_after() {
            let Some(entry) = planned.attribution.get(id) else {
                continue;
            };
            installed.push((
                id.clone(),
                InstalledSubscription {
                    filters: entry.filters.clone(),
                    serves: entry.serves.clone(),
                },
            ));
            attribution.insert(id.clone(), entry.serves.iter().copied().collect::<Vec<_>>());
        }
        let opened: Vec<SubscriptionId> =
            planned.open.iter().map(|entry| entry.id.clone()).collect();
        slot.installed = InstalledSubscriptions::from_entries(installed);
        slot.attribution = attribution;
        slot.complete.retain(|id, _| slot.attribution.contains_key(id));
        for id in &opened {
            slot.complete.insert(id.clone(), false);
        }
        if planned.is_noop() {
            self.publish_plan(relay, demand, planned);
            self.publish_relay_diagnostic(relay);
            return;
        }
        slot.busy = true;
        let generation = slot.generation;
        operations::apply(
            &self.runtime,
            &self.reports,
            operations::Installing {
                relay: relay.clone(),
                generation,
                revision,
            },
            Arc::clone(session),
            planned.open.clone(),
            planned.close.clone(),
            self.providers.deadlines.write,
        );
        self.publish_plan(relay, demand, planned);
        self.publish_relay_diagnostic(relay);
    }

    /// Accept one provider completion, refusing every superseded generation.
    fn accept(&mut self, report: Report) -> bool {
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
            Report::Applied {
                relay,
                generation,
                revision,
                withdrawn,
            } => self.applied(&relay, generation, revision, &withdrawn),
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
        operations::listen(
            &self.runtime,
            &self.reports,
            relay.clone(),
            generation,
            &session,
            slot.cancel.clone(),
        );
        self.publish_relay_diagnostic(relay);
        true
    }

    fn refused(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        detail: &BoundedText,
    ) -> bool {
        let Some(slot) = self.slots.get_mut(relay) else {
            return false;
        };
        if slot.generation != generation {
            return false;
        }
        slot.state = fava_diagnostics::RelaySessionState::Unreachable {
            detail: detail.clone(),
        };
        let lease = slot.lease.take();
        slot.session = None;
        slot.advance(&self.root);
        if let Some(lease) = lease {
            self.release_lease(lease);
        }
        let state = failure_state(detail);
        self.publish_state_for_relay(relay, &state);
        self.publish_relay_diagnostic(relay);
        false
    }

    fn applied(
        &mut self,
        relay: &RelaySessionKey,
        generation: OperationGeneration,
        revision: PlanRevision,
        withdrawn: &[SubscriptionId],
    ) -> bool {
        let Some(slot) = self.slots.get_mut(relay) else {
            return false;
        };
        if slot.generation != generation || slot.revision != revision {
            return false;
        }
        for id in withdrawn {
            slot.complete.remove(id);
        }
        slot.busy = false;
        let requested_at = Timestamp::now();
        let observations: Vec<(ObservationId, SubscriptionId)> = slot
            .attribution
            .iter()
            .flat_map(|(id, served)| served.iter().map(|demand| (demand.owner, id.clone())))
            .collect();
        for (owner, _) in observations {
            self.registry.record_state(
                owner,
                relay,
                generation,
                RelaySourceState::Open { requested_at },
            );
        }
        self.publish_relay_diagnostic(relay);
        true
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
            RelayInbound::Frame {
                identity, frame, ..
            } => {
                self.frame(relay, &identity, &frame);
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
            RelayInbound::Reconnected { identity, .. } => {
                let Some(slot) = self.slots.get_mut(relay) else {
                    return false;
                };
                slot.reconnects = slot.reconnects.saturating_add(1);
                slot.state = fava_diagnostics::RelaySessionState::Open;
                let next = slot.advance(&self.root);
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
                let _ = identity;
                self.publish_state_for_relay(relay, &RelaySourceState::Connecting);
                self.publish_relay_diagnostic(relay);
                true
            }
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

    fn frame(
        &mut self,
        relay: &RelaySessionKey,
        identity: &fava_transport::RelaySessionIdentity,
        frame: &[u8],
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let outcome = ingest::accept(
            self.providers.cache.as_ref(),
            relay,
            &slot.installed,
            frame,
        );
        match outcome {
            ingest::Accepted::Nothing | ingest::Accepted::Event => {}
            ingest::Accepted::StoredEventsComplete(id) => {
                let at = Timestamp::now();
                if let Some(slot) = self.slots.get_mut(relay) {
                    slot.complete.insert(id.clone(), true);
                }
                self.publish_for_subscription(relay, &id, |_| {
                    RelaySourceState::StoredEventsComplete { at }
                });
                self.publish_relay_diagnostic(relay);
            }
            ingest::Accepted::Refused { id, message } => {
                let at = Timestamp::now();
                self.publish_for_subscription(relay, &id, move |_| RelaySourceState::Refused {
                    message: message.clone(),
                    at,
                });
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
        let _ = identity;
    }

    /// Withdraw every installed subscription and release the relay's lease.
    fn release(&mut self, relay: &RelaySessionKey) {
        let Some(mut slot) = self.slots.remove(relay) else {
            return;
        };
        slot.cancel.cancel();
        let subscriptions: Vec<SubscriptionId> = slot.installed.ids().cloned().collect();
        let generation = slot.generation;
        if let Some(lease) = slot.lease.take() {
            operations::withdraw(
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

    fn release_lease(&self, lease: Box<RelaySessionLease>) {
        operations::withdraw(
            &self.runtime,
            lease,
            Vec::new(),
            OperationGeneration(0),
            self.providers.deadlines.write,
            self.providers.deadlines.close,
        );
    }

    /// Publish one state to every observation currently demanding this relay.
    fn publish_states(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        state: &RelaySourceState,
    ) {
        let generation = self
            .slots
            .get(relay)
            .map_or(OperationGeneration(0), |slot| slot.generation);
        for item in demand {
            self.registry
                .record_state(item.owner, relay, generation, state.clone());
        }
    }

    fn publish_state_for_relay(&self, relay: &RelaySessionKey, state: &RelaySourceState) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        for owner in self.registry.open_observations() {
            if self.registry.demand_id(owner, relay).is_some() {
                self.registry
                    .record_state(owner, relay, generation, state.clone());
            }
        }
    }

    fn publish_for_subscription(
        &self,
        relay: &RelaySessionKey,
        id: &SubscriptionId,
        state: impl Fn(ObservationId) -> RelaySourceState,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let generation = slot.generation;
        for owner in slot.owners(id) {
            self.registry
                .record_state(owner, relay, generation, state(owner));
        }
    }

    /// Publish plan-scoped facts: who shares each wire subscription, and what
    /// demand the plan could not carry.
    fn publish_plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        planned: &SubscriptionPlan,
    ) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        let revision = planned.revision.0;
        for item in demand {
            let id = item.id();
            let shared_with = slot
                .serving(id)
                .map(|wire| slot.owners(wire))
                .unwrap_or_default();
            let shortfall = planned
                .shortfalls
                .iter()
                .find(|entry| entry.demand == id)
                .map(|entry| RelayShortfall {
                    branches: vec![item.branch],
                    detail: BoundedText::new(shortfall_detail(&entry.reason)),
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

    fn publish_relay_diagnostic(&self, relay: &RelaySessionKey) {
        let Some(slot) = self.slots.get(relay) else {
            return;
        };
        self.providers.diagnostics.relay(diagnostics::relay_fact(
            relay,
            slot.generation,
            slot.state.clone(),
            slot.lease.is_some().into(),
            slot.installed
                .ids()
                .map(|id| {
                    diagnostics::wire_fact(
                        id.clone(),
                        slot.owners(id),
                        slot.complete.get(id).copied().unwrap_or_default(),
                    )
                })
                .collect(),
            slot.reconnects,
        ));
    }
}

fn failure_state(detail: &BoundedText) -> RelaySourceState {
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

fn shortfall_detail(reason: &ShortfallReason) -> String {
    format!("{reason:?}")
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

fn _assert_unused(_: TransportFailure) {}
