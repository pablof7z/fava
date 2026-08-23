//! Shared fixtures for the standard planner falsifiers.

use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{DeclaredLimit, DemandId, RelayDemand, RelayReadConstraints};
use nostr::filter::Filter;

/// The relay session every fixture plans against.
#[must_use]
pub fn relay() -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("wss://relay.example").expect("relay URL"),
        RelayAccess::public(),
    )
}

/// One observation identity.
#[must_use]
pub fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation identity"))
}

/// One root-branch demand with default bounds.
#[must_use]
pub fn demand(value: u64, filter: Filter) -> RelayDemand {
    RelayDemand::new(
        observation(value),
        QueryBranchId::ROOT,
        filter,
        QueryBounds::default(),
    )
}

/// One root-branch demand carrying an explicit whole-query result bound.
#[must_use]
pub fn bounded_demand(value: u64, filter: Filter, limit: u32) -> RelayDemand {
    RelayDemand::new(
        observation(value),
        QueryBranchId::ROOT,
        filter,
        QueryBounds {
            since: None,
            until: None,
            limit: NonZeroU32::new(limit),
        },
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

/// A declared limit.
#[must_use]
pub fn declared(value: usize) -> DeclaredLimit {
    DeclaredLimit::Declared(NonZeroUsize::new(value).expect("non-zero declared limit"))
}

/// Constraints declaring only a subscription ceiling.
#[must_use]
#[allow(dead_code, reason = "shared fixture; not every test file uses every helper")]
pub fn declaring_subscriptions(maximum: usize) -> RelayReadConstraints {
    RelayReadConstraints {
        max_subscriptions: declared(maximum),
        ..RelayReadConstraints::unknown()
    }
}
