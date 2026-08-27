//! Exact mapping from logical relay demand to Nostr subscriptions.
//!
//! The planner owns the whole mapping from *logical* demand to *wire*
//! subscriptions and nothing else: it allocates every wire [`fava_wire::SubscriptionId`],
//! decides grouping and splitting, and proves grouping did not change meaning
//! (RELAY-003). It owns the diff — given the complete current demand for one
//! relay and the set currently installed on that relay's session, it decides
//! what to open, what to leave untouched, and what to close, and the close list
//! *is* withdrawal identity. It owns typed in-plan shortfall: a plan that
//! carries 60 of 64 filters is a [`SubscriptionPlan`] with four
//! [`SubscriptionShortfall`] entries, not an `Err`. It owns [`validate_plan`],
//! the conformance rules that define semantic equivalence for any planner.
//!
//! It owns no socket, no route policy, no observation state, and no refcount:
//! the planner is told the truth about demand and answers.

mod conformance;
mod constraints;
mod coverage;
mod demand;
mod installed;
mod plan;
mod planner;

use std::num::{NonZeroU32, NonZeroUsize};

pub use conformance::{PlanConformanceError, validate_plan};
pub use constraints::{DeclaredLimit, RelayReadConstraints};
pub use coverage::filter_covers;
pub use demand::{DemandId, RelayDemand};
use fava_query::Query;
/// Cross-crate read-side identity, re-exported from its neutral home so a
/// planner never has to depend on the lifecycle owner that mints it.
pub use fava_query::{ObservationId, OperationGeneration, QueryBounds, QueryBranchId};
pub use installed::{InstalledSubscription, InstalledSubscriptions};
use nostr::filter::Filter;
pub use plan::{
    AttributedSubscription, EoseCompleteness, PlanRevision, PlanRevisionExhausted,
    PlanRevisionIssuer, PlannedSubscription, ShortfallReason, SubscriptionAttribution,
    SubscriptionPlan, SubscriptionShortfall, WithdrawalReason, WithdrawnSubscription,
};
pub use planner::{SubscriptionPlanError, SubscriptionPlanner};

/// Convert one public Query into the exact NIP-01 relay demand of one
/// observation branch.
#[must_use]
pub fn demand_for_query(owner: ObservationId, branch: QueryBranchId, query: &Query) -> RelayDemand {
    let mut filter = Filter::new();
    if let Some(ids) = &query.selection().ids {
        filter = filter.ids(ids.iter().copied());
    }
    if let Some(authors) = &query.selection().authors {
        filter = filter.authors(authors.iter().copied());
    }
    if let Some(kinds) = &query.selection().kinds {
        filter = filter.kinds(kinds.iter().copied());
    }
    for (key, values) in &query.selection().tag_values {
        filter = filter.custom_tags(*key, values.iter().cloned());
    }
    if let Some(limit) = query.result_limit() {
        filter = filter.limit(limit.get());
    }
    RelayDemand::new(owner, branch, filter, bounds_for_query(query))
}

/// Whole-query bounds a planner must not merge across.
fn bounds_for_query(query: &Query) -> QueryBounds {
    QueryBounds {
        since: None,
        until: None,
        limit: query.result_limit().and_then(narrow_limit),
    }
}

/// Narrow a whole-query result bound into the wire's 32-bit limit space.
fn narrow_limit(limit: NonZeroUsize) -> Option<NonZeroU32> {
    NonZeroU32::new(u32::try_from(limit.get()).unwrap_or(u32::MAX))
}
