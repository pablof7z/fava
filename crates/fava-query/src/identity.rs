//! Cross-crate read-side identity: observation, branch, bounds, generation.
//!
//! These four nouns are named by the `fava-subscriptions` contract
//! (`ARCH:1492-1497`) but are semantically owned by `fava-observe`. A neutral
//! contract crate must not depend on a lifecycle owner
//! (`ARCH:2984-3016`), so they are defined here — the lowest crate every
//! contract already depends on — and re-exported by the contracts that name
//! them. `fava-observe` remains the semantic owner: it is the only crate that
//! mints values.

use std::num::{NonZeroU32, NonZeroU64};

use nostr::types::Timestamp;

/// Identity of one open Observation. Minted only by `fava-observe`.
///
/// Authority: ARCH:1493 (`RelayDemand.owner: ObservationId`),
/// ARCH:2065 ("observation identity and open/close lifecycle").
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObservationId(NonZeroU64);

impl ObservationId {
    /// Construct from a monotonic non-zero counter owned by `fava-observe`.
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Stable numeric form for diagnostics and test assertions.
    #[must_use]
    pub const fn get(self) -> NonZeroU64 {
        self.0
    }
}

/// Identity of one branch of a composed Query within one Observation.
///
/// Authority: ARCH:1494 (`RelayDemand.branch: QueryBranchId`);
/// GOALS:401 (QUERY-008) "Per-branch and per-relay evidence MUST remain
/// associated with the branch and source that produced it."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QueryBranchId(pub u32);

impl QueryBranchId {
    /// The single branch of an unbranched Query.
    pub const ROOT: Self = Self(0);
}

/// Whole-query bounds carried with demand so a planner can refuse to merge
/// across differing bounds.
///
/// Authority: ARCH:1495 (`RelayDemand.bounds: QueryBounds`);
/// GOALS:1049 (RELAY-003) "MUST NOT merge across differences that would change
/// meaning, including incompatible time windows, relay-side limits".
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct QueryBounds {
    /// Inclusive lower time bound, when the query declares one.
    pub since: Option<Timestamp>,
    /// Inclusive upper time bound, when the query declares one.
    pub until: Option<Timestamp>,
    /// Per-request result limit, when the query declares one.
    pub limit: Option<NonZeroU32>,
}

/// Generation of one owner-authorized provider operation.
///
/// Any completion carrying a generation older than the owner's current
/// generation for that operation slot is stale and MUST NOT mutate state.
///
/// Authority: GOALS:426 (QUERY-010) "Reopening dropped demand MUST use fresh
/// request identity so a late EOSE or event from the old request cannot settle
/// the new one."; ARCH:1610 "Reconnected sessions are new authorities."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationGeneration(pub u64);

impl OperationGeneration {
    /// Advance to the next generation. Saturating: exhaustion is not a panic.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
