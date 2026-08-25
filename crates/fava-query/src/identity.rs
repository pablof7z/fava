//! Cross-crate identity primitives for observations, branches, and operations.
//!
//! These nouns are semantically owned by `fava-observe`, which is the only
//! crate that mints values. They are *defined* here because `fava-query` is the
//! lowest crate every neutral contract already depends on, so this is the only
//! placement that keeps the dependency arrow pointing from lifecycle owners to
//! contracts rather than the reverse (`ARCH:3050-3082`).

use core::num::{NonZeroU32, NonZeroU64};

use fava_state::Timestamp;

/// Identity of one open Observation. Minted only by `fava-observe`.
///
/// Authority: `ARCH:1493` (`RelayDemand.owner: ObservationId`),
/// `ARCH:2065` ("observation identity and open/close lifecycle").
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

/// The single minting authority for [`ObservationId`].
///
/// One value is minted per *observation*, never per relay and never per
/// reconnect. A logical query fanned out to N relays is still one observation:
/// minting per relay would give the same query N owners, and grouped relay
/// demand could then never be attributed back to one observation, so a
/// grouped EOSE could not settle it. Reconnecting is a new *operation
/// generation* ([`OperationGeneration`]), not a new observation.
///
/// The allocator therefore belongs to whatever owns observation lifecycle
/// (`fava-observe`), and lives here only because [`ObservationId`] does.
#[derive(Debug, Default)]
pub struct ObservationIds {
    next: core::sync::atomic::AtomicU64,
}

impl ObservationIds {
    /// A fresh allocator.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: core::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Mint the identity of one newly opened observation.
    ///
    /// Returns `None` only when the counter is exhausted, which callers must
    /// refuse rather than wrap: reusing an identity would let one observation
    /// settle another's demand.
    pub fn allocate(&self) -> Option<ObservationId> {
        let sequence = self
            .next
            .fetch_update(
                core::sync::atomic::Ordering::SeqCst,
                core::sync::atomic::Ordering::SeqCst,
                |value| value.checked_add(1),
            )
            .ok()?
            .checked_add(1)?;
        NonZeroU64::new(sequence).map(ObservationId::new)
    }
}

/// Identity of one branch of a composed Query within one Observation.
///
/// Authority: `ARCH:1494` (`RelayDemand.branch: QueryBranchId`);
/// `GOALS:401` (QUERY-008) "Per-branch and per-relay evidence MUST remain
/// associated with the branch and source that produced it."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QueryBranchId(u32);

impl QueryBranchId {
    /// The single branch of an unbranched Query.
    pub const ROOT: Self = Self(0);

    /// Mint a branch id. Only `fava-observe` should call this.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Raw branch id for diagnostics.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Whole-query bounds carried with demand so a planner can refuse to merge
/// across differing bounds.
///
/// Authority: `ARCH:1495` (`RelayDemand.bounds: QueryBounds`);
/// `GOALS:1055` (RELAY-003) "MUST NOT merge across differences that would
/// change meaning, including incompatible time windows, relay-side limits".
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
/// Authority: `GOALS:426` (QUERY-010) "Reopening dropped demand MUST use fresh
/// request identity so a late EOSE or event from the old request cannot settle
/// the new one."; `ARCH:1610` "Reconnected sessions are new authorities."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationGeneration(u64);

impl OperationGeneration {
    /// Mint one generation value. Only `fava-observe` and transport crates that
    /// receive the initial value from `fava-observe` should call this.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Raw generation counter, for diagnostics and transport-layer encoding.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Advance to the next generation. Saturating: exhaustion is not a panic.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}
