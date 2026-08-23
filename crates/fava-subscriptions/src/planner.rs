//! The replaceable planner contract and its refusal type.

use fava_state::RelaySessionKey;
use fava_transport::BoundedReason;
use fava_wire::SubscriptionId;
use thiserror::Error;

use crate::constraints::RelayReadConstraints;
use crate::demand::{DemandId, RelayDemand};
use crate::installed::InstalledSubscriptions;
use crate::plan::{PlanRevision, SubscriptionPlan};

/// Replaceable mapping from logical demand to exact Nostr subscriptions.
///
/// Authority: ARCH:1483-1490.
pub trait SubscriptionPlanner: Send + Sync {
    /// Produce the desired plan for one relay session.
    ///
    /// `demand` is the **complete current** logical demand for `relay` across
    /// every observation. An empty `demand` with a non-empty `installed` is a
    /// legal call whose correct answer is a plan that closes everything.
    ///
    /// # Errors
    ///
    /// [`SubscriptionPlanError`] only for inputs the planner cannot process at
    /// all. Demand the planner *understands* but cannot carry is a
    /// [`crate::SubscriptionShortfall`] inside `Ok`, never an error.
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
        installed: &InstalledSubscriptions,
        revision: PlanRevision,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}

/// Exact subscription planning refusal. Reserved for malformed input.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SubscriptionPlanError {
    /// Two demands in one call carry the same `DemandId`.
    #[error("duplicate logical demand: observation {:?} branch {:?}", .0.owner, .0.branch)]
    DuplicateDemand(DemandId),
    /// The planner allocated the same wire id twice within one plan.
    #[error("duplicate wire subscription id: {0}")]
    DuplicateSubscription(SubscriptionId),
    /// Exact Nostr REQ encoding failed before any handoff.
    #[error("REQ encoding failed: {0:?}")]
    Encoding(BoundedReason),
}
