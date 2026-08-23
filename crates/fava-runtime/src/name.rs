//! Static names for spawned tasks and provider operation slots.
//!
//! A static name cannot be relay-, OS-, or application-supplied, so it needs no
//! bounded-text wrapper and can be retained in shortfall evidence unchanged.

use std::fmt;

/// Static name of one spawned task, for joins and diagnostics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TaskName(pub &'static str);

impl fmt::Display for TaskName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Static name of one provider operation slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationName(pub &'static str);

impl fmt::Display for OperationName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
