//! The pending-admission cohort and the coverage test that lets demand attach.
//!
//! A wire subscription that has reached the socket is immutable. Demand that
//! has *not* reached the wire batches in one fixed, first-arrival-anchored,
//! non-sliding window and is compiled as a single cohort; the planner removes
//! demand a live request already carries before it merges anything, so the
//! merge step structurally cannot widen a request that has already reached the
//! wire. Demand arriving after the freeze either attaches to an incumbent
//! that already physically covers it, or opens its own request alongside it.
//!
//! Rewriting a running subscription costs the relay a full re-serve of the
//! window it already served, and the cost is quadratic in the number of growth
//! steps. It is never taken.
//!
//! **Containment has exactly one implementation.** The owner decides whether a
//! window must be armed and which running request a joiner attaches to; the
//! planner decides the same thing when it admits a cohort. Two answers are one
//! defect: an owner that judges demand covered when the planner would not is
//! silent under-fetch, because the owner then arms no window and the planner is
//! never asked. Both read [`fava_subscriptions::filter_covers`], the neutral
//! contract's predicate, and neither keeps a copy.

use std::collections::BTreeSet;
use std::time::Duration;

use fava_subscriptions::{DemandId, RelayDemand};

/// Fixed wire-admission window, anchored at the first uncovered demand.
///
/// Repeated arming while a window is pending never extends it: a sliding
/// deadline starves under a steady arrival stream.
pub(crate) const ADMISSION_WINDOW: Duration = Duration::from_millis(10);

/// The demand identities one cohort carries.
pub(crate) fn identities(cohort: &[RelayDemand]) -> BTreeSet<DemandId> {
    cohort.iter().map(RelayDemand::id).collect()
}
