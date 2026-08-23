//! One relay session's owned state: its lease, its live requests, and the
//! cohort of demand that has not reached the wire yet.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_query::{ObservationId, OperationGeneration};
use fava_runtime::CancellationToken;
use fava_subscriptions::{
    DemandId, InstalledSubscription, InstalledSubscriptions, PlanRevision, RelayDemand,
};
use fava_transport::{RelaySession, RelaySessionLease};
use fava_wire::SubscriptionId;

use crate::admission::LiveSubscription;

pub(crate) struct Slot {
    pub(crate) generation: OperationGeneration,
    pub(crate) cancel: CancellationToken,
    pub(crate) lease: Option<Box<RelaySessionLease>>,
    pub(crate) session: Option<Arc<dyn RelaySession>>,
    pub(crate) live: BTreeMap<SubscriptionId, LiveSubscription>,
    pub(crate) retired: BTreeSet<SubscriptionId>,
    pub(crate) pending: BTreeMap<DemandId, RelayDemand>,
    pub(crate) armed: bool,
    pub(crate) revision: PlanRevision,
    pub(crate) busy: bool,
    pub(crate) state: fava_diagnostics::RelaySessionState,
    pub(crate) reconnects: usize,
}

impl Slot {
    pub(crate) fn new(cancel: CancellationToken) -> Self {
        Self {
            generation: OperationGeneration(1),
            cancel,
            lease: None,
            session: None,
            live: BTreeMap::new(),
            retired: BTreeSet::new(),
            pending: BTreeMap::new(),
            armed: false,
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
    pub(crate) fn advance(&mut self, root: &CancellationToken) -> OperationGeneration {
        self.cancel.cancel();
        self.cancel = root.child();
        self.live.clear();
        self.retired.clear();
        self.armed = false;
        self.busy = false;
        self.generation = self.generation.next();
        self.generation
    }

    /// Every demand this slot currently serves or is holding for admission.
    pub(crate) fn held(&self) -> BTreeSet<DemandId> {
        self.live
            .values()
            .flat_map(|entry| entry.serves.iter().copied())
            .chain(self.pending.keys().copied())
            .collect()
    }

    pub(crate) fn owners(&self, id: &SubscriptionId) -> Vec<ObservationId> {
        let mut owners: Vec<ObservationId> = self
            .live
            .get(id)
            .into_iter()
            .flat_map(|entry| entry.serves.iter().map(|demand| demand.owner))
            .collect();
        owners.sort_unstable();
        owners.dedup();
        owners
    }

    pub(crate) fn serving(&self, demand: DemandId) -> Option<&SubscriptionId> {
        self.live
            .iter()
            .find(|(_, entry)| entry.serves.contains(&demand))
            .map(|(id, _)| id)
    }

    /// The installed view inbound attribution resolves against.
    pub(crate) fn installed(&self) -> InstalledSubscriptions {
        InstalledSubscriptions::from_entries(self.live.iter().map(|(id, entry)| {
            (
                id.clone(),
                InstalledSubscription {
                    filters: entry.filters.clone(),
                    serves: entry.serves.clone(),
                },
            )
        }))
    }
}
