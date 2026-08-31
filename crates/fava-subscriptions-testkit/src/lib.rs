//! The conformance kit every subscription planner must pass.
//!
//! `ARCH:3148` requires an external planner to "pass the same conformance kit
//! as the standard provider". That kit is two things: the executable rules in
//! [`fava_subscriptions::validate_plan`], and the differential in this crate —
//! grouped and ungrouped planning of the same demand must ask for the same
//! events, attribute them to the same logical demand, keep one observation's
//! results out of another's, and withdraw the same demand on cancellation.

mod differential;
mod scenario;

pub use differential::{
    DifferentialReport, assert_planners_agree, assert_withdrawal_agrees, delivered_to,
    settled_by_eose_on,
};
pub use scenario::{
    PlannerScenario, all_opened, apply_plan, assert_conformant,
    assert_partial_withdrawal_leaves_the_wire_alone, assert_running_subscriptions_are_immutable,
};
