//! The desired wire plan for one relay session, expressed as a diff.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};

use fava_relay::RelaySessionKey;
use fava_transport::BoundedReason;
use fava_wire::SubscriptionId;
use nostr::filter::Filter;
use thiserror::Error;

use crate::demand::DemandId;

/// Monotonic identity of one desired plan for one relay session.
///
/// Authority: ARCH:1511 "plan diff values"; GOALS:426 (QUERY-010) stale
/// completion rejection.
///
/// The identity cannot be directly minted or advanced by a planner:
///
/// ```compile_fail
/// let _ = fava_subscriptions::PlanRevision::default();
/// ```
///
/// ```compile_fail
/// let _ = fava_subscriptions::PlanRevision::new(1);
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanRevision {
    authority: NonZeroU64,
    sequence: NonZeroU64,
}

impl PlanRevision {
    /// Opaque identity of the observation owner that minted this revision.
    #[must_use]
    pub const fn authority(self) -> NonZeroU64 {
        self.authority
    }

    /// Monotonic sequence within this revision authority.
    #[must_use]
    pub const fn sequence(self) -> NonZeroU64 {
        self.sequence
    }
}

impl fmt::Display for PlanRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.authority, self.sequence)
    }
}

static NEXT_PLAN_AUTHORITY: AtomicU64 = AtomicU64::new(0);

/// The sole minting capability for one desired-plan revision authority.
///
/// This capability is intentionally not cloneable. Planners receive an opaque
/// revision to copy into their answer; they never receive this issuer.
#[derive(Debug)]
pub struct PlanRevisionIssuer {
    authority: NonZeroU64,
    next: Option<NonZeroU64>,
}

impl PlanRevisionIssuer {
    /// Create one independent revision authority.
    ///
    /// # Errors
    ///
    /// Returns typed exhaustion rather than reusing an authority namespace.
    pub fn new() -> Result<Self, PlanRevisionExhausted> {
        let authority = NEXT_PLAN_AUTHORITY
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_add(1)
            })
            .ok()
            .and_then(|previous| previous.checked_add(1))
            .and_then(NonZeroU64::new)
            .ok_or(PlanRevisionExhausted::Authorities)?;
        Ok(Self {
            authority,
            next: NonZeroU64::new(1),
        })
    }

    /// Mint the next desired-plan revision under this authority.
    ///
    /// # Errors
    ///
    /// Returns typed exhaustion after the maximum sequence has been issued
    /// once. It never wraps, saturates, or returns the same revision twice.
    pub fn allocate(&mut self) -> Result<PlanRevision, PlanRevisionExhausted> {
        let sequence = self.next.ok_or(PlanRevisionExhausted::Sequence)?;
        self.next = sequence.get().checked_add(1).and_then(NonZeroU64::new);
        Ok(PlanRevision {
            authority: self.authority,
            sequence,
        })
    }
}

/// Exact desired-plan revision allocation refusal.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PlanRevisionExhausted {
    /// No fresh authority namespace remains in this process.
    #[error("plan-revision authority space is exhausted")]
    Authorities,
    /// This authority issued its final sequence and cannot issue another.
    #[error("plan-revision sequence is exhausted")]
    Sequence,
}

#[cfg(test)]
mod plan_revision_tests {
    use super::*;

    #[test]
    fn separate_authorities_never_mint_the_same_revision() {
        let mut first = PlanRevisionIssuer::new().expect("first authority");
        let mut second = PlanRevisionIssuer::new().expect("second authority");
        assert_ne!(first.allocate().unwrap(), second.allocate().unwrap());
    }

    #[test]
    fn final_sequence_is_issued_once_then_refused() {
        let mut revisions = PlanRevisionIssuer::new().expect("authority");
        revisions.next = NonZeroU64::new(u64::MAX);
        let last = revisions.allocate().expect("maximum issues once");
        assert_eq!(last.sequence().get(), u64::MAX);
        assert_eq!(revisions.allocate(), Err(PlanRevisionExhausted::Sequence));
    }
}

/// One wire subscription the plan wants opened.
///
/// Authority: ARCH:1500 (`wire: Vec<PlannedSubscription>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedSubscription {
    /// Wire id the planner allocated. Never a logical id.
    pub id: SubscriptionId,
    /// Filters for this REQ. A NIP-01 REQ may carry several.
    pub filters: Vec<Filter>,
    /// Logical demand this subscription serves.
    pub serves: BTreeSet<DemandId>,
}

/// One wire subscription the plan wants closed, with its reason.
///
/// Authority: ARCH:1513 "withdrawal identity".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawnSubscription {
    /// Wire id to CLOSE.
    pub id: SubscriptionId,
    /// Why this wire subscription lost its last logical holder.
    pub reason: WithdrawalReason,
}

/// Why a wire subscription is being withdrawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WithdrawalReason {
    /// Every `DemandId` it served has left the demand set.
    DemandWithdrawn {
        /// Demand that was still attributed to it at withdrawal.
        released: BTreeSet<DemandId>,
    },
    /// Its demand is now served by a different wire subscription.
    Regrouped {
        /// Wire subscription that now serves the demand.
        into: SubscriptionId,
    },
    /// It no longer fits the relay's declared constraints.
    ConstraintChanged,
}

/// Attribution from every wire subscription back to the logical demand it serves.
///
/// Authority: ARCH:1501 (`attribution: SubscriptionAttribution`);
/// GOALS:1043; ARCH:2044 (ingest attributes "to an accepted wire subscription
/// and logical demand").
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubscriptionAttribution {
    entries: BTreeMap<SubscriptionId, AttributedSubscription>,
}

/// The complete attribution record for one wire subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributedSubscription {
    /// Filters accepted under this wire id. An inbound event must match at
    /// least one of them (ARCH:2046).
    pub filters: Vec<Filter>,
    /// Every logical demand this wire subscription serves. An EOSE on this
    /// wire id settles every one of them.
    pub serves: BTreeSet<DemandId>,
    /// Whether an EOSE on this wire id is proof the stored window is complete.
    pub completeness: EoseCompleteness,
}

/// What an EOSE on one wire subscription actually proves.
///
/// The planner is the only component that knows both the filter it sent and
/// what the relay declared, so it records the fact here rather than leaving the
/// evidence layer to re-derive it from a filter it never saw.
///
/// Authority: GOALS:1066 (RELAY-004) "MUST NOT ... claim omitted work was
/// completed."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EoseCompleteness {
    /// The relay finished the complete stored window for every demand served.
    #[default]
    Proven,
    /// The request carries a result-count bound, so the relay stopped at that
    /// count rather than at the end of the window. EOSE proves nothing about
    /// completeness for any demand this subscription serves.
    LimitedRequest,
    /// The relay declared a default filter limit, so even an unbounded request
    /// is truncated at a count the client never chose.
    RelayDefaultLimit,
}

impl SubscriptionAttribution {
    /// Construct from entries.
    #[must_use]
    pub fn from_entries(
        entries: impl IntoIterator<Item = (SubscriptionId, AttributedSubscription)>,
    ) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Attribution for one wire id, or `None` when the relay named a wire id
    /// Fava never accepted. `None` is the only correct response to an
    /// unattributable frame.
    #[must_use]
    pub fn get(&self, id: &SubscriptionId) -> Option<&AttributedSubscription> {
        self.entries.get(id)
    }

    /// Logical demand served by one wire id; empty when unattributed.
    #[must_use]
    pub fn serves(&self, id: &SubscriptionId) -> &BTreeSet<DemandId> {
        static EMPTY: std::sync::OnceLock<BTreeSet<DemandId>> = std::sync::OnceLock::new();
        self.entries
            .get(id)
            .map_or_else(|| EMPTY.get_or_init(BTreeSet::new), |entry| &entry.serves)
    }

    /// Every wire id, ascending.
    pub fn ids(&self) -> impl Iterator<Item = &SubscriptionId> {
        self.entries.keys()
    }

    /// Number of attributed wire subscriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is attributed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Demand the plan could not carry, attributed and typed, inside a plan that
/// still succeeded for the rest.
///
/// Authority: ARCH:1502 (`shortfalls: Vec<SubscriptionShortfall>`), ARCH:1512,
/// ARCH:1536, GOALS:1066 "MUST NOT ... claim omitted work was completed".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionShortfall {
    /// Exact logical demand omitted from this plan.
    pub demand: DemandId,
    /// Why it was omitted.
    pub reason: ShortfallReason,
}

/// Why exact demand could not be carried.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShortfallReason {
    /// The relay's declared subscription count is already fully used.
    SubscriptionsExhausted {
        /// Wire subscriptions required to carry all demand exactly.
        required: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// No exact encoding of this demand fits the declared message bound.
    MessageTooLarge {
        /// Smallest exact encoding the planner could produce.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// The relay's declared filter limit is below the demand's own limit.
    FilterLimitExceeded {
        /// Limit the demand requires.
        required: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// No wire id short enough to satisfy the declared id-length limit could be
    /// allocated without collision.
    SubscriptionIdTooLong {
        /// Declared maximum characters.
        maximum: usize,
    },
    /// The planner refuses to express this demand exactly on this relay.
    NotExpressible {
        /// Bounded planner reason.
        detail: BoundedReason,
    },
}

/// The desired plan for one relay session, expressed as a diff against what is
/// currently installed.
///
/// Authority: ARCH:1499-1503 (name and the `attribution` / `shortfalls`
/// fields), ARCH:1511 "plan diff values", ARCH:1513 "withdrawal identity".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionPlan {
    /// Exact relay session this plan applies to.
    pub relay: RelaySessionKey,
    /// Monotonic revision of the desired plan.
    pub revision: PlanRevision,
    /// Wire subscriptions to open now. Never contains an installed id.
    pub open: Vec<PlannedSubscription>,
    /// Installed wire subscriptions that survive this replan untouched.
    /// No frame is emitted for these.
    pub retain: Vec<SubscriptionId>,
    /// Installed wire subscriptions to CLOSE now.
    pub close: Vec<WithdrawnSubscription>,
    /// Complete attribution for the plan's *resulting* installed set, i.e.
    /// `open` plus `retain`.
    pub attribution: SubscriptionAttribution,
    /// Demand this plan does not carry.
    pub shortfalls: Vec<SubscriptionShortfall>,
}

impl SubscriptionPlan {
    /// Whether this plan changes anything on the wire.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.open.is_empty() && self.close.is_empty()
    }

    /// Wire ids the plan expects to be installed after execution.
    pub fn installed_after(&self) -> impl Iterator<Item = &SubscriptionId> {
        self.open
            .iter()
            .map(|planned| &planned.id)
            .chain(self.retain.iter())
    }
}
