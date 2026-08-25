//! The replaceable planner contract and its refusal type.

use fava_relay::RelaySessionKey;
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
///
/// # What the planner may do with `installed`
///
/// A subscription the relay is already serving is **immutable**. `installed`
/// exists so the planner can answer four questions and no others:
///
/// * **attach** — is this demand's traffic already arriving under a running
///   subscription, so that it needs no subscription of its own?
/// * **residual budget** — how much of a declared subscription ceiling is left
///   after what is already running?
/// * **refcount** — which running subscriptions have lost their last logical
///   owner and may now close?
/// * **append position** — which wire ids are already taken.
///
/// `installed` must **never** reach grouping or identity. A planner that feeds
/// running demand back into its merge pass rewrites live subscriptions every
/// time demand grows: the relay re-serves the whole stored window for demand it
/// had already settled, and the waste is quadratic in the number of growth
/// steps. [`crate::validate_plan`] enforces the consequence as CR-1.
///
/// # What the caller must guarantee
///
/// The planner is a pure function. Everything below is the demand owner's
/// obligation, and a planner cannot detect a caller that breaks it.
///
/// * **Completeness.** `demand` is the complete current logical demand for this
///   exact relay session, across every observation. A demand omitted from the
///   slice is read as withdrawn.
/// * **Fresh revision.** `revision` is strictly greater for every plan whose
///   content could differ from the last, and is never reused within one
///   transport session. Wire identity is minted from it, so a reused revision
///   can hand a reopened subscription the identity of a closed one — which
///   `GOALS:426` (QUERY-010) forbids by name, because a late EOSE for the old
///   request would then settle the new one.
/// * **Truthful baseline.** `installed` is exactly what the transport accepted
///   on the current session generation, and is empty on a fresh generation.
/// * **Open before close.** A [`crate::WithdrawnSubscription`] whose reason is
///   [`crate::WithdrawalReason::Regrouped`] names its successor. The executor
///   sends that successor's REQ first and withholds the CLOSE until the
///   successor is locally accepted; if the successor is refused, the
///   predecessor stays live and no CLOSE is sent. An in-place re-REQ under an
///   existing id is never correct: the following EOSE names only the shared
///   id and cannot say which filter generation it completed.
///
/// # The admission cohort
///
/// Grouping has nothing to group unless demand is batched before it reaches the
/// planner, and batching is a lifecycle decision rather than a planning one, so
/// the demand owner holds the cohort:
///
/// * the first demand not already covered by `installed` arms a **fixed 10ms,
///   first-arrival-anchored** deadline; later arrivals join the cohort and
///   **never extend** it;
/// * the deadline delays only unsent wire work — a new observation still
///   projects from the local cache immediately;
/// * cancellation before the deadline drops the demand with no call to `plan`;
/// * demand reported as [`crate::ShortfallReason::SubscriptionsExhausted`]
///   stays pending and is retried in a later window, and a withdrawal that
///   frees budget arms one;
/// * on flush the owner calls `plan` **once** per affected relay session.
///
/// A sliding deadline is wrong: it starves under a steady arrival stream.
pub trait SubscriptionPlanner: Send + Sync {
    /// Produce the desired plan for one relay session.
    ///
    /// `demand` is the **complete current** logical demand for `relay` across
    /// every observation. An empty `demand` with a non-empty `installed` is a
    /// legal call whose correct answer is a plan that closes everything.
    ///
    /// The answer must be a function of the demand *set*, never of the order
    /// the slice happens to arrive in (CR-3).
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
