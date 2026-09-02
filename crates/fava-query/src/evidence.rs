//! Where one query's results came from and how far each relay and local source got.
//!
//! `fava-query` owns the *vocabulary* of evidence, not the facts. It owns the
//! guarantee that an application can tell "this relay told us it has nothing"
//! from "we never reached this relay" from "this relay refused us" from "we
//! stopped asking" (`GOALS:416`, `GOALS:422-426`). Every relay-scoped value
//! here is a *report* written by `fava-observe` from facts owned elsewhere:
//! this crate owns no relay session, no plan, and no refcount.

use std::num::NonZeroUsize;

use fava_relay::{BoundedText, Progress};
use fava_state::RetractionCause;
use nostr::event::EventId;
use nostr::types::{RelayUrl, Timestamp};

use crate::SourceRevision;
use crate::identity::{ObservationId, QueryBranchId, Round};

/// Role of one contribution to the universal query merge.
///
/// `LiveRelay` is the third role required by `GOALS:344-350` (QUERY-005):
/// "accept admitted live relay occurrences as current query input even when the
/// selected event cache does not retain them".
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// Signed relay-observed cache state.
    EventCache,
    /// Current accepted local revisions.
    WriteStore,
    /// Verified live occurrences admitted from one relay session, retained by
    /// no store.
    LiveRelay {
        /// Relay that served them.
        session: RelayUrl,
    },
}

/// Whether one continuous source observation is open, and why it ended.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SourceStatus {
    /// The provider's continuous observation remains open.
    #[default]
    Open,
    /// The provider's observation terminated after a coherent prior snapshot.
    Closed {
        /// Why it terminated. `ARCH:724` merge rule 5 requires the cause to
        /// survive as scoped evidence.
        cause: SourceTerminationCause,
    },
}

/// Why a source observation ended.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceTerminationCause {
    /// Fava closed the observation.
    LocalClose,
    /// The provider closed cleanly.
    ProviderClosed,
    /// The provider failed.
    ProviderFailed {
        /// Bounded provider reason.
        detail: BoundedText,
    },
    /// The engine is shutting down (distinct from a source failure,
    /// `GOALS:302`).
    Shutdown,
}

impl core::fmt::Display for SourceTerminationCause {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LocalClose => formatter.write_str("Fava closed the observation"),
            Self::ProviderClosed => formatter.write_str("the provider closed cleanly"),
            Self::ProviderFailed { detail } => {
                write!(formatter, "the provider failed: {}", detail.as_str())
            }
            Self::Shutdown => formatter.write_str("the engine is shutting down"),
        }
    }
}

/// One retained event a source removed, and the exact rule that removed it.
///
/// A removal is not the absence of an event: `GOALS:422` requires an
/// application to be able to tell a NIP-09 deletion from a supersession, an
/// expiry, or a provider's own capacity eviction. Collapsing all four to "the
/// id is gone from `events`" destroys that distinction at the source boundary,
/// so the cause travels with the revision that applied it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRetraction {
    /// The retained event the source removed.
    pub event_id: EventId,
    /// The rule that removed it.
    pub cause: RetractionCause,
}

impl SourceRetraction {
    /// Record one retraction fact.
    #[must_use]
    pub const fn new(event_id: EventId, cause: RetractionCause) -> Self {
        Self { event_id, cause }
    }

    /// Whether a Nostr event-state rule removed the event, as opposed to the
    /// provider removing it under its own bound or maintenance.
    ///
    /// An application that lost a retained event to
    /// [`RetractionCause::Evicted`] may still ask a relay for it; one that lost
    /// it to [`RetractionCause::Deleted`] must not.
    #[must_use]
    pub const fn is_protocol_rule(&self) -> bool {
        !matches!(self.cause, RetractionCause::Evicted)
    }
}

/// Which source this is, how far it has advanced, whether it is still open, and
/// which retained events it dropped on the way.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvidence {
    /// Query-source role.
    pub kind: SourceKind,
    /// Last coherent revision included in the result.
    pub revision: SourceRevision,
    /// Whether the continuous source observation remains open.
    pub status: SourceStatus,
    /// Retained events this source removed to reach the included revision, and
    /// why. Empty when the revision removed nothing.
    pub retractions: Vec<SourceRetraction>,
}

impl SourceEvidence {
    /// Why this source removed one exact event to reach the included revision.
    #[must_use]
    pub fn retraction(&self, event_id: &EventId) -> Option<&RetractionCause> {
        self.retractions
            .iter()
            .find(|retraction| &retraction.event_id == event_id)
            .map(|retraction| &retraction.cause)
    }
}

// ------------------------------------------------------------ relay evidence

/// How far one relay session has got with one query, what demand it carries and
/// shares, what the plan leaves uncovered, and how it entered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayQueryEvidence {
    /// Exact relay.
    pub session: RelayUrl,
    /// Transport connection generation these facts belong to.
    pub generation: Option<Round>,
    /// Desired-plan revision under which this relay's demand was requested.
    /// A completion carrying an older revision is stale (`GOALS:426`).
    pub plan_revision: u64,
    /// Query branches whose demand this relay currently carries.
    pub branches: Vec<QueryBranchId>,
    /// How far this relay has got with the current request.
    pub state: RelaySourceState,
    /// Observations sharing the wire work behind this relay's demand,
    /// including this one (`ARCH:2072`, `GOALS:294-298`).
    pub shared_with: Vec<ObservationId>,
    /// Demand for this relay the current plan does not carry.
    pub shortfall: Option<RelayShortfall>,
    /// Whether this relay entered the query through automatic routing or an
    /// explicit relay set (`GOALS:473-481`, QUERY-014).
    pub route: RouteOrigin,
}

impl RelayQueryEvidence {
    /// Whether this relay has actually sent EOSE for the exact current request.
    ///
    /// The single predicate `GOALS:420-426` (QUERY-010) exists to protect: it
    /// is true only for [`RelaySourceState::StoredEventsComplete`].
    #[must_use]
    pub fn stored_events_complete(&self) -> bool {
        matches!(self.state, RelaySourceState::StoredEventsComplete { .. })
    }

    /// Whether this relay is currently able to deliver new events.
    #[must_use]
    pub fn is_live(&self) -> bool {
        matches!(
            self.state,
            RelaySourceState::Open { .. } | RelaySourceState::StoredEventsComplete { .. }
        )
    }
}

/// Exactly how far one relay has got with one query, and why it stopped if it did.
///
/// Every variant is distinct at the type level. No two of them may be produced
/// by the same underlying fact (`GOALS:422`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelaySourceState {
    /// Routing named this relay; no session has been acquired yet.
    Planned,
    /// A session is being established.
    Connecting,
    /// A session is live and the request is installed; no EOSE yet.
    Open {
        /// When the request was installed.
        requested_at: Timestamp,
    },
    /// The relay sent EOSE for the exact current request identity.
    StoredEventsComplete {
        /// When the EOSE arrived.
        at: Timestamp,
    },
    /// The relay sent CLOSED for the request.
    Refused {
        /// Verbatim, bounded relay text (`GOALS:1111`, RELAY-008).
        message: BoundedText,
        /// When it arrived.
        at: Timestamp,
    },
    /// The relay's connection has an outstanding NIP-42 challenge: it asked,
    /// and nothing has decided what to do about it yet.
    ///
    /// Carries the connection's own [`Progress`], the one place that fact is
    /// kept, rather than a second, query-side opinion about it. A connection
    /// that has since resolved the challenge — accepted, declined, or been
    /// refused — is reported through that resolution, not through this: this
    /// variant exists only while the question is still open.
    AuthenticationRequired {
        /// The connection's own record of how the challenge is going.
        progress: Progress,
        /// When the requirement was learned.
        at: Timestamp,
    },
    /// A Fava-owned deadline expired.
    TimedOut {
        /// Which deadline.
        deadline: RelayDeadline,
        /// The duration that expired, in milliseconds.
        after_ms: u64,
    },
    /// The session dropped and reconnect is in progress.
    Disconnected {
        /// Bounded reason.
        detail: BoundedText,
    },
    /// Reconnect budget is exhausted; this relay will not return by itself.
    Unreachable {
        /// Attempts actually made.
        attempts: usize,
        /// Bounded reason of the final attempt.
        detail: BoundedText,
    },
    /// A local lifecycle owner refused to issue fresh identity for this work.
    OwnerRefused {
        /// Typed refusal rendered into a bounded diagnostic reason.
        detail: BoundedText,
    },
    /// Fava withdrew this relay's demand (route withdrawal or query close).
    Withdrawn {
        /// Why.
        reason: RelayWithdrawal,
    },
}

/// Which Fava-owned deadline expired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayDeadline {
    /// Session establishment.
    Establish,
    /// Frame write.
    Write,
    /// Inbound silence.
    Idle,
    /// Close handshake.
    Close,
}

/// Why Fava stopped asking one relay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayWithdrawal {
    /// No router still contributes this destination (`GOALS:479`).
    RouteWithdrawn,
    /// The observation closed.
    ObservationClosed,
    /// The engine is shutting down.
    Shutdown,
}

/// Whether a relay entered a query automatically or explicitly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteOrigin {
    /// Named by the query's explicit relay set.
    Explicit,
    /// Contributed by automatic routing at this route revision.
    Automatic {
        /// Route revision that contributed it.
        revision: u64,
    },
}

/// Demand for one relay that the current plan does not carry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayShortfall {
    /// Branches whose demand is omitted.
    pub branches: Vec<QueryBranchId>,
    /// Bounded reason, produced from `fava_subscriptions::ShortfallReason`.
    pub detail: BoundedText,
}

// ------------------------------------------------------------- plan evidence

/// Revision, relay coverage, and installed-subscription count of the plan behind
/// this query's relay demand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredPlanEvidence {
    /// Monotonic desired-plan revision (`fava_subscriptions::PlanRevision`).
    pub revision: u64,
    /// Relays the plan covers.
    pub relays: Vec<RelayUrl>,
    /// Wire subscriptions installed for this observation's demand.
    pub installed: usize,
}

// ----------------------------------------------------------------- shortfall

/// Query-scoped loss or limit that is not attributable to one relay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryShortfall {
    /// Intermediate revisions were superseded before delivery. Bounded loss is
    /// explicit and typed (`GOALS:434`, QUERY-011).
    CoalescedUpdates {
        /// Revisions dropped since the last delivered snapshot.
        dropped: u64,
    },
    /// A whole-query event bound discarded matching events before delivery.
    ResultLimitApplied {
        /// The bound applied.
        limit: NonZeroUsize,
    },
    /// The observation owner refused an otherwise valid live transition
    /// because retaining it would exceed one exact session's live-state bound.
    LiveRetentionLimit {
        /// Exact relay whose live state reached the bound.
        session: RelayUrl,
        /// Maximum live events retained for that exact session.
        limit: NonZeroUsize,
        /// Valid transitions refused since this session's live state opened.
        refused: u64,
    },
    /// A source could not be opened at all.
    SourceUnavailable {
        /// Which role.
        kind: SourceKind,
        /// Bounded reason.
        detail: BoundedText,
    },
}

// ------------------------------------------------------------ query evidence

/// Which sources and relay sessions produced one result, how far each got, and
/// what was lost reaching it.
///
/// Authority: `ARCH:700-716` (`QuerySnapshot.evidence`), `GOALS:393-401`
/// (QUERY-008), `GOALS:403-418` (QUERY-009), `GOALS:420-428` (QUERY-010).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryEvidence {
    /// Latest scoped local source facts included in this exact result.
    pub sources: Vec<SourceEvidence>,
    /// Latest scoped relay facts for every relay this query has used or
    /// intends to use.
    pub relays: Vec<RelayQueryEvidence>,
    /// The desired plan behind the relay facts, when this query has relay
    /// demand.
    pub plan: Option<DesiredPlanEvidence>,
    /// Query-scoped loss and limits.
    pub shortfalls: Vec<QueryShortfall>,
}

impl QueryEvidence {
    /// Evidence for one exact relay session.
    #[must_use]
    pub fn relay(&self, session: &RelayUrl) -> Option<&RelayQueryEvidence> {
        self.relays.iter().find(|entry| &entry.session == session)
    }

    /// Evidence for one local source role.
    #[must_use]
    pub fn source(&self, kind: &SourceKind) -> Option<&SourceEvidence> {
        self.sources.iter().find(|entry| &entry.kind == kind)
    }

    /// Whether every relay this query uses has sent EOSE for its current
    /// request. Never a claim about the network (`GOALS:403-418`, QUERY-009).
    #[must_use]
    pub fn all_relays_stored_events_complete(&self) -> bool {
        !self.relays.is_empty()
            && self
                .relays
                .iter()
                .all(RelayQueryEvidence::stored_events_complete)
    }
}
