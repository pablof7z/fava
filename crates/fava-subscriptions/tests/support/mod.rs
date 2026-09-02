//! Shared fixtures for the conformance falsifiers.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use fava_subscriptions::{
    DemandId, EoseCompleteness, PlanRevision, PlanRevisionIssuer, PlannedSubscription, RelayDemand,
    SubscriptionAttribution, SubscriptionPlan,
};
use fava_wire::SubscriptionId;
use nostr::filter::Filter;
use nostr::types::RelayUrl;

/// The relay session every fixture plans against.
#[must_use]
pub fn relay() -> RelayUrl {
    RelayUrl::parse("wss://relay.example").expect("relay URL")
}

/// One observation identity.
#[must_use]
pub fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity"))
}

/// One independently minted plan revision at the requested small sequence.
#[must_use]
pub fn revision(sequence: u64) -> PlanRevision {
    let mut revisions = PlanRevisionIssuer::new().expect("revision authority");
    let mut current = revisions.allocate().expect("first revision");
    for _ in 1..sequence {
        current = revisions.allocate().expect("requested revision");
    }
    current
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
    _id: &SubscriptionId,
    filters: Vec<Filter>,
    serves: BTreeSet<DemandId>,
) -> SubscriptionPlan {
    SubscriptionPlan {
        relay: relay(),
        revision: revision(1),
        open: vec![PlannedSubscription {
            filters,
            serves,
            completeness: EoseCompleteness::Proven,
        }],
        retain: Vec::new(),
        close: Vec::new(),
        // Nothing is retained, so nothing carries a wire id to attribute.
        attribution: SubscriptionAttribution::default(),
        shortfalls: Vec::new(),
    }
}
