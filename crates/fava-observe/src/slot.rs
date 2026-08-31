//! One relay session's owned state: its lease, the plan the transport actually
//! accepted, and whether a demand cohort is waiting for its admission window.

use std::collections::BTreeMap;
use std::sync::Arc;

use fava_query::{ObservationId, OperationGeneration};
use fava_runtime::CancellationToken;
use fava_subscriptions::{
    DemandId, EoseCompleteness, InstalledSubscriptions, PlanRevision, RelayDemand,
    RelayReadConstraints, filter_covers,
};
use fava_transport::{RelaySession, RelaySessionLease};
use fava_wire::SubscriptionId;

use crate::admission;

pub(crate) struct Slot {
    pub(crate) generation: OperationGeneration,
    pub(crate) cancel: CancellationToken,
    pub(crate) lease: Option<Box<RelaySessionLease>>,
    pub(crate) session: Option<Arc<dyn RelaySession>>,
    /// Relay-declared read limits, updated once per session from NIP-11.
    pub(crate) constraints: RelayReadConstraints,
    /// Exactly what the transport accepted on the current generation.
    pub(crate) installed: InstalledSubscriptions,
    /// The task holding each installed subscription's handle. Cancelling one
    /// drops its handle, and dropping a handle closes the subscription.
    pub(crate) attending: BTreeMap<SubscriptionId, CancellationToken>,
    /// What an EOSE on each installed wire subscription actually proves.
    pub(crate) completeness: BTreeMap<SubscriptionId, EoseCompleteness>,
    /// Wire subscriptions the relay has sent EOSE for on this generation.
    pub(crate) settled: BTreeMap<SubscriptionId, bool>,
    /// The last plan revision this slot issued. The counter it is stamped
    /// from lives on the engine and outlives the slot, because wire identity
    /// is minted from it and a slot dies whenever its relay's demand drains.
    pub(crate) revision: Option<PlanRevision>,
    /// A fixed admission window is pending for this relay.
    pub(crate) armed: bool,
    pub(crate) busy: bool,
    /// Connectivity of this relay session, independent of any query.
    pub(crate) state: fava_diagnostics::RelaySessionState,
    pub(crate) reconnects: usize,
}

impl Slot {
    pub(crate) fn new(cancel: CancellationToken, generation: OperationGeneration) -> Self {
        Self {
            generation,
            cancel,
            lease: None,
            session: None,
            constraints: RelayReadConstraints::unknown(),
            installed: InstalledSubscriptions::empty(),
            attending: BTreeMap::new(),
            completeness: BTreeMap::new(),
            settled: BTreeMap::new(),
            revision: None,
            armed: false,
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
    pub(crate) fn advance(
        &mut self,
        root: &CancellationToken,
        generation: OperationGeneration,
    ) -> OperationGeneration {
        self.cancel.cancel();
        self.cancel = root.child();
        // The connection is gone, so every handle it carried is already over.
        self.attending.clear();
        self.installed = InstalledSubscriptions::empty();
        self.completeness.clear();
        self.settled.clear();
        self.armed = false;
        self.busy = false;
        self.generation = generation;
        self.generation
    }

    /// Demand whose traffic no running subscription already carries.
    ///
    /// This is what arms the admission window: work that has not reached the
    /// wire, and only that.
    pub(crate) fn uncovered<'a>(&'a self, demand: &'a [RelayDemand]) -> Vec<&'a RelayDemand> {
        demand.iter().filter(|item| !self.covers(item)).collect()
    }

    /// Whether any running subscription has lost its last serving demand.
    pub(crate) fn orphaned(&self, demand: &[RelayDemand]) -> bool {
        let wanted = admission::identities(demand);
        self.installed.ids().any(|id| {
            self.installed
                .get(id)
                .is_some_and(|entry| entry.serves.iter().all(|held| !wanted.contains(held)))
        })
    }

    fn covers(&self, demand: &RelayDemand) -> bool {
        self.installed.ids().any(|id| {
            self.installed.get(id).is_some_and(|entry| {
                entry.serves.contains(&demand.id())
                    || entry
                        .filters
                        .iter()
                        .any(|filter| filter_covers(filter, &demand.filter))
            })
        })
    }

    pub(crate) fn owners(&self, id: &SubscriptionId) -> Vec<ObservationId> {
        let mut owners: Vec<ObservationId> = self
            .installed
            .get(id)
            .into_iter()
            .flat_map(|entry| entry.serves.iter().map(|demand| demand.owner))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }

    pub(crate) fn serving(&self, demand: DemandId) -> Option<&SubscriptionId> {
        self.installed.ids().find(|id| {
            self.installed
                .get(id)
                .is_some_and(|entry| entry.serves.contains(&demand))
        })
    }

    /// Whether an EOSE on one wire subscription proves its stored window ended.
    pub(crate) fn proves_completeness(&self, id: &SubscriptionId) -> EoseCompleteness {
        self.completeness.get(id).copied().unwrap_or_default()
    }
}
