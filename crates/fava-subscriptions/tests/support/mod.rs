//! Shared fixtures for the conformance falsifiers.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions::{
    AttributedSubscription, DemandId, EoseCompleteness, PlanRevision, PlannedSubscription,
    RelayDemand, SubscriptionAttribution, SubscriptionPlan,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;
use nostr::types::RelayUrl;

/// The relay session every fixture plans against.
#[must_use]
pub fn relay() -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse("wss://relay.example").expect("relay URL"),
        access: RelayAccess::Public,
    }
}

/// One observation identity.
#[must_use]
pub fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity"))
}

/// One root-branch demand for the given observation and filter.
#[must_use]
pub fn demand(value: u64, filter: Filter) -> RelayDemand {
    RelayDemand::new(
        observation(value),
        QueryBranchId::ROOT,
        filter,
        QueryBounds::default(),
    )
}

/// The logical identity of the root branch of one observation.
#[must_use]
pub fn demand_id(value: u64) -> DemandId {
    DemandId {
        owner: observation(value),
        branch: QueryBranchId::ROOT,
    }
}

/// A wire id.
#[must_use]
pub fn wire(name: &str) -> SubscriptionId {
    SubscriptionId::new(name)
}

/// A plan that opens one wire subscription carrying `filters` for `serves`.
#[must_use]
#[allow(
    dead_code,
    reason = "shared fixture; not every test file uses every helper"
)]
pub fn opening(
    id: &SubscriptionId,
    filters: Vec<Filter>,
    serves: BTreeSet<DemandId>,
) -> SubscriptionPlan {
    SubscriptionPlan {
        relay: relay(),
        revision: PlanRevision(1),
        open: vec![PlannedSubscription {
            id: id.clone(),
            filters: filters.clone(),
            serves: serves.clone(),
        }],
        retain: Vec::new(),
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([(
            id.clone(),
            AttributedSubscription {
                filters,
                serves,
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    }
}
