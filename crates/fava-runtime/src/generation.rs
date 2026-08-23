//! Operation generation identity.
//!
//! `fava-query` owns this noun. `FROZEN-CONTRACTS.md` §0 places
//! `OperationGeneration` in `fava-query` and §5 has `fava-runtime` consume it
//! from there. §0 has not landed in this worktree, so the type is defined here
//! verbatim and this module is replaced by
//!
//! ```text
//! pub use fava_query::OperationGeneration;
//! ```
//!
//! the moment `fava-query` carries it. This is a pending import, not a second
//! authority: nothing in `fava-runtime` interprets the value, it only carries
//! it from the authorising call back to the owner's completion.

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
