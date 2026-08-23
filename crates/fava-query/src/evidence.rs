//! Scoped evidence attached to one query result.
//!
//! `fava-query` owns the *vocabulary* of evidence, not the facts. It owns the
//! guarantee that an application can tell "this relay told us it has nothing"
//! from "we never reached this relay" from "this relay refused us" from "we
//! stopped asking" (`GOALS:416`, `GOALS:422-426`). Every relay-scoped value
//! here is a *report* written by `fava-observe` from facts owned elsewhere:
//! this crate owns no relay session, no plan, and no refcount.

use std::num::NonZeroUsize;

use fava_state::{RelaySessionKey, RelayUrl, Timestamp};

use crate::SourceRevision;
use crate::identity::{ObservationId, OperationGeneration, QueryBranchId};

/// Role of one contribution to the universal query merge.
///
/// `LiveRelay` is the third role required by `GOALS:344-350` (QUERY-005):
/// "accept admitted live relay occurrences as current query input even when the
/// selected event cache does not retain them".
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// Signed relay-observed cache state.
    EventCache,
    /// Current accepted local materializations.
    WriteStore,
    /// Verified live occurrences admitted from one relay session, retained by
    /// no store.
    LiveRelay {
        /// Relay session that served them.
        session: RelaySessionKey,
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

/// Revision and lifecycle fact for one independent local source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEvidence {
    /// Query-source role.
    pub kind: SourceKind,
    /// Last coherent revision included in the result.
    pub revision: SourceRevision,
    /// Whether the continuous source observation remains open.
    pub status: SourceStatus,
}

// ------------------------------------------------------------ relay evidence

/// What Fava currently knows about one relay session serving one query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayQueryEvidence {
    /// Relay and access authority.
    pub session: RelaySessionKey,
    /// Transport connection generation these facts belong to.
    pub generation: OperationGeneration,
    /// Desired-plan revision under which this relay's demand was requested.
    /// A completion carrying an older revision is stale (`GOALS:426`).
    pub plan_revision: u64,
    /// Query branches whose demand this relay currently carries.
    pub branches: Vec<QueryBranchId>,
    /// Current state of this relay's contribution.
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

/// Exact state of one relay's contribution to one query.
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
    /// The relay demands NIP-42 authentication for this request.
    AuthenticationRequired {
        /// Current authentication state for this session.
        state: AuthenticationState,
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

/// NIP-42 state for one relay session, as seen by a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthenticationState {
    /// A challenge arrived; no policy decision yet.
    ChallengeReceived,
    /// The application's policy declined to authenticate.
    Declined,
    /// AUTH was sent; no relay verdict yet.
    Attempted,
    /// The relay accepted AUTH but still refuses the request.
    AcceptedButStillRefused,
    /// The relay rejected AUTH.
    Rejected {
        /// Verbatim, bounded relay text.
        message: BoundedText,
    },
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

/// The desired subscription plan currently backing this query's relay demand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredPlanEvidence {
    /// Monotonic desired-plan revision (`fava_subscriptions::PlanRevision`).
    pub revision: u64,
    /// Relay sessions the plan covers.
    pub relays: Vec<RelaySessionKey>,
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
    /// A whole-query result bound truncated the result.
    ResultLimitApplied {
        /// The bound applied.
        limit: NonZeroUsize,
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

/// Complete scoped evidence for one query result.
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
    pub fn relay(&self, session: &RelaySessionKey) -> Option<&RelayQueryEvidence> {
        self.relays.iter().find(|entry| &entry.session == session)
    }

    /// Every session at one relay URL, across relay-access identities.
    pub fn relays_at<'a>(
        &'a self,
        relay: &'a RelayUrl,
    ) -> impl Iterator<Item = &'a RelayQueryEvidence> {
        self.relays
            .iter()
            .filter(move |entry| &entry.session.relay == relay)
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

/// Owner-supplied text retained under a Fava-owned byte bound.
///
/// Identical semantics to `fava_transport::BoundedReason`; duplicated here so
/// `fava-query` keeps zero contract dependencies. `MAX_BYTES` is 512 in both.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedText {
    text: String,
    truncated_bytes: usize,
}

impl BoundedText {
    /// Maximum retained bytes.
    pub const MAX_BYTES: usize = 512;

    /// Retain at most `MAX_BYTES`, recording how many were dropped.
    #[must_use]
    pub fn new(text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        let mut end = text.len().min(Self::MAX_BYTES);
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            text: text[..end].to_owned(),
            truncated_bytes: text.len() - end,
        }
    }

    /// Retained text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Bytes dropped by the bound.
    #[must_use]
    pub const fn truncated_bytes(&self) -> usize {
        self.truncated_bytes
    }
}
