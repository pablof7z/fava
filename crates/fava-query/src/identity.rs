//! Cross-crate identity primitives for observations, branches, and operations.
//!
//! These nouns are semantically owned by `fava-observe`, which is the only
//! crate that mints values. They are *defined* here because `fava-query` is the
//! lowest crate every neutral contract already depends on, so this is the only
//! placement that keeps the dependency arrow pointing from lifecycle owners to
//! contracts rather than the reverse (`ARCH:3050-3082`).

use core::fmt;
use core::num::{NonZeroU32, NonZeroU64};
use core::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

use nostr::types::Timestamp;

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
pub struct QueryBranchId(pub u32);

impl QueryBranchId {
    /// The single branch of an unbranched Query.
    pub const ROOT: Self = Self(0);
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
///
/// The identity cannot be directly minted or advanced by a consumer:
///
/// ```compile_fail
/// let _ = fava_query::OperationGeneration::default();
/// ```
///
/// ```compile_fail
/// let _ = fava_query::OperationGeneration::new(1);
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationGeneration {
    authority: NonZeroU64,
    sequence: NonZeroU64,
}

impl OperationGeneration {
    /// Opaque identity of the authority that minted this generation.
    #[must_use]
    pub const fn authority(self) -> NonZeroU64 {
        self.authority
    }

    /// Monotonic sequence within this identity's authority.
    #[must_use]
    pub const fn sequence(self) -> NonZeroU64 {
        self.sequence
    }
}

impl fmt::Display for OperationGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.authority, self.sequence)
    }
}

static NEXT_OPERATION_AUTHORITY: AtomicU64 = AtomicU64::new(0);

/// The sole minting capability for one operation-generation authority.
///
/// The capability is intentionally not cloneable. A component may create an
/// independent authority, but it cannot forge or advance a generation issued
/// by another authority.
#[derive(Debug)]
pub struct OperationGenerationIssuer {
    authority: NonZeroU64,
    next: Option<NonZeroU64>,
}

impl OperationGenerationIssuer {
    /// Create one independent generation authority.
    ///
    /// # Errors
    ///
    /// Returns typed exhaustion rather than reusing an authority namespace.
    pub fn new() -> Result<Self, OperationGenerationExhausted> {
        let authority = NEXT_OPERATION_AUTHORITY
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(|previous| previous.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or(OperationGenerationExhausted::Authorities)?;
        Ok(Self {
            authority,
            next: NonZeroU64::new(1),
        })
    }

    /// Mint the next generation under this authority.
    ///
    /// # Errors
    ///
    /// Returns typed exhaustion after the maximum sequence has been issued
    /// once. It never wraps, saturates, or returns the same identity twice.
    pub fn allocate(&mut self) -> Result<OperationGeneration, OperationGenerationExhausted> {
        let sequence = self.next.ok_or(OperationGenerationExhausted::Sequence)?;
        self.next = sequence.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(OperationGeneration {
            authority: self.authority,
            sequence,
        })
    }
}

/// Exact operation-generation allocation refusal.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum OperationGenerationExhausted {
    /// No fresh authority namespace remains in this process.
    #[error("operation-generation authority space is exhausted")]
    Authorities,
    /// This authority issued its final sequence and cannot issue another.
    #[error("operation-generation sequence is exhausted")]
    Sequence,
}

#[cfg(test)]
mod operation_generation_tests {
    use super::*;

    #[test]
    fn separate_authorities_never_mint_the_same_generation() {
        let mut first = OperationGenerationIssuer::new().expect("first authority");
        let mut second = OperationGenerationIssuer::new().expect("second authority");
        assert_ne!(first.allocate().unwrap(), second.allocate().unwrap());
    }

    #[test]
    fn final_sequence_is_issued_once_then_refused() {
        let mut generations = OperationGenerationIssuer::new().expect("authority");
        generations.next = NonZeroU64::new(u64::MAX);
        let last = generations.allocate().expect("maximum issues once");
        assert_eq!(last.sequence().get(), u64::MAX);
        assert_eq!(
            generations.allocate(),
            Err(OperationGenerationExhausted::Sequence)
        );
    }
}
