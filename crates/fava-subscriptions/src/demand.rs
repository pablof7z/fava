//! Logical read demand assigned to one exact relay session.

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use nostr::filter::Filter;

/// Stable identity of one unit of logical relay demand.
///
/// Two observations of the same query produce two distinct `DemandId`s; one
/// observation's two branches also produce two. This is what lets a grouped
/// EOSE settle more than one logical query.
///
/// Authority: GOALS:1043 (RELAY-002) "The planner MUST preserve attribution
/// from every wire request back to the logical queries it serves."
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemandId {
    /// Observation that needs this demand.
    pub owner: ObservationId,
    /// Branch within that observation.
    pub branch: QueryBranchId,
}

/// One logical read demand assigned to one exact relay session.
///
/// Authority: ARCH:1492-1497, verbatim field set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayDemand {
    /// Observation that owns this demand.
    pub owner: ObservationId,
    /// Branch of that observation's query.
    pub branch: QueryBranchId,
    /// Exact NIP-01 filter requested from the relay.
    pub filter: Filter,
    /// Whole-query bounds that constrain safe merging.
    pub bounds: QueryBounds,
}

impl RelayDemand {
    /// Construct one exact logical relay demand.
    #[must_use]
    pub const fn new(
        owner: ObservationId,
        branch: QueryBranchId,
        filter: Filter,
        bounds: QueryBounds,
    ) -> Self {
        Self {
            owner,
            branch,
            filter,
            bounds,
        }
    }

    /// Logical identity of this demand.
    #[must_use]
    pub const fn id(&self) -> DemandId {
        DemandId {
            owner: self.owner,
            branch: self.branch,
        }
    }
}
