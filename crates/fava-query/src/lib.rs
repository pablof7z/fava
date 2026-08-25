//! Declarative event queries, local source contracts, and application snapshots.

mod evidence;
mod identity;
mod selection;

use std::collections::BTreeSet;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use fava_relay::RelayAccess;
use fava_state::{RelayEvent, RelayOccurrences};
use fava_write::{EventValue, LocalWriteEvent, PublicationEvidence};
pub use nostr::event::{EventId, Kind};
pub use nostr::key::PublicKey;
pub use nostr::types::{RelayUrl, Timestamp};
pub use selection::{FilterSelection, SingleLetterTag};

pub use evidence::{
    AuthenticationState, BoundedText, DesiredPlanEvidence, QueryEvidence, QueryShortfall,
    RelayDeadline, RelayQueryEvidence, RelayShortfall, RelaySourceState, RelayWithdrawal,
    RouteOrigin, SourceEvidence, SourceKind, SourceRetraction, SourceStatus,
    SourceTerminationCause,
};
pub use identity::{
    ObservationId, ObservationIds, OperationGeneration, QueryBounds, QueryBranchId,
};
use thiserror::Error;

/// Relays Fava should ask for acquisition.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum QueryAcquisition {
    /// Use the application-selected automatic router chain.
    Automatic,
    /// Ask exactly this non-empty relay set and bypass automatic routing.
    Explicit(BTreeSet<RelayUrl>),
}

/// Evidence authority required for a record to enter the result.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ResultAuthority {
    /// Matching events from any configured local source may appear.
    AnyLocal,
    /// A record requires actual relay evidence from this exact set.
    OnlyRelays(BTreeSet<RelayUrl>),
}

/// Acquisition and result authority, kept separate in query identity.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct QuerySourcePolicy {
    /// Where Fava asks.
    acquisition: QueryAcquisition,
    /// Which evidence may enter the result.
    authority: ResultAuthority,
}

impl Default for QuerySourcePolicy {
    fn default() -> Self {
        Self {
            acquisition: QueryAcquisition::Automatic,
            authority: ResultAuthority::AnyLocal,
        }
    }
}

impl QuerySourcePolicy {
    /// Where Fava asks for events.
    #[must_use]
    pub const fn acquisition(&self) -> &QueryAcquisition {
        &self.acquisition
    }

    /// Evidence required for an event to enter the result.
    #[must_use]
    pub const fn authority(&self) -> &ResultAuthority {
        &self.authority
    }
}

/// Whether a query may create relay demand.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Freshness {
    /// Use configured local sources only.
    CacheOnly,
    /// Keep relay demand live. This is the ordinary default.
    #[default]
    Live,
}

/// Deterministic application-facing ordering.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum QueryOrdering {
    /// Newest timestamp first, then greatest event id.
    #[default]
    NewestFirst,
    /// Oldest timestamp first, then least event id.
    OldestFirst,
}

/// Declarative request for events.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Query {
    /// Event selection.
    selection: FilterSelection,
    /// Acquisition and provenance authority.
    source: QuerySourcePolicy,
    /// Relay access.
    access: RelayAccess,
    /// Whether live relay demand is permitted.
    freshness: Freshness,
    /// Deterministic result order.
    ordering: QueryOrdering,
    /// Whole-query result bound.
    limit: Option<NonZeroUsize>,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            selection: FilterSelection::default(),
            source: QuerySourcePolicy::default(),
            access: RelayAccess::Public,
            freshness: Freshness::Live,
            ordering: QueryOrdering::NewestFirst,
            limit: None,
        }
    }
}

impl Query {
    /// Select exact relay access before acquisition opens.
    #[must_use]
    pub fn with_relay_access(mut self, access: RelayAccess) -> Self {
        self.access = access;
        self
    }

    /// Ask exactly these relays while retaining ordinary local result visibility.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError::EmptyExplicitRelays`] for an empty relay set.
    pub fn from_relays(
        mut self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Self, QueryError> {
        let relays = non_empty_relays(relays)?;
        self.source = QuerySourcePolicy {
            acquisition: QueryAcquisition::Explicit(relays),
            authority: ResultAuthority::AnyLocal,
        };
        Ok(self)
    }

    /// Ask exactly these relays and require actual provenance from that set.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError::EmptyExplicitRelays`] for an empty relay set.
    pub fn only_from_relays(
        mut self,
        relays: impl IntoIterator<Item = RelayUrl>,
    ) -> Result<Self, QueryError> {
        let relays = non_empty_relays(relays)?;
        self.source = QuerySourcePolicy {
            acquisition: QueryAcquisition::Explicit(relays.clone()),
            authority: ResultAuthority::OnlyRelays(relays),
        };
        Ok(self)
    }

    /// Use local sources without creating relay demand.
    #[must_use]
    pub const fn cache_only(mut self) -> Self {
        self.freshness = Freshness::CacheOnly;
        self
    }

    /// Apply one whole-query result bound.
    ///
    /// # Errors
    ///
    /// Returns [`QueryError::ZeroLimit`] when `limit` is zero.
    pub fn limit(mut self, limit: usize) -> Result<Self, QueryError> {
        self.limit = Some(NonZeroUsize::new(limit).ok_or(QueryError::ZeroLimit)?);
        Ok(self)
    }

    /// Select oldest-first ordering.
    #[must_use]
    pub const fn oldest_first(mut self) -> Self {
        self.ordering = QueryOrdering::OldestFirst;
        self
    }

    /// Acquisition and result-authority policy.
    #[must_use]
    pub const fn source(&self) -> &QuerySourcePolicy {
        &self.source
    }

    /// Relay access.
    #[must_use]
    pub const fn access(&self) -> &RelayAccess {
        &self.access
    }

    /// Whether this query may create live relay demand.
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    /// Deterministic result ordering.
    #[must_use]
    pub const fn ordering(&self) -> QueryOrdering {
        self.ordering
    }

    /// Whole-query result bound.
    #[must_use]
    pub const fn result_limit(&self) -> Option<NonZeroUsize> {
        self.limit
    }
}

fn non_empty_relays(
    relays: impl IntoIterator<Item = RelayUrl>,
) -> Result<BTreeSet<RelayUrl>, QueryError> {
    let relays: BTreeSet<_> = relays.into_iter().collect();
    if relays.is_empty() {
        Err(QueryError::EmptyExplicitRelays)
    } else {
        Ok(relays)
    }
}

/// Query refusal before any source or relay work opens.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum QueryError {
    /// Explicit acquisition requires at least one relay.
    #[error("explicit relay acquisition requires a non-empty relay set")]
    EmptyExplicitRelays,
    /// Whole-query limits must be positive.
    #[error("query limit must be greater than zero")]
    ZeroLimit,
}

/// Monotonic revision owned by one query source.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevision(pub u64);

/// One source contribution to the universal query merge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceEvent {
    /// One atomic live or retained signed relay contribution.
    Relay(RelayEvent),
    /// Current local materialization and publication evidence from a write store.
    Local(LocalWriteEvent),
}

/// Complete current answer from one local source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSnapshot {
    /// Query-source role.
    pub kind: SourceKind,
    /// Monotonic provider-owned revision.
    pub revision: SourceRevision,
    /// Current lifecycle fact for this independently owned source.
    pub status: SourceStatus,
    /// Complete current contributions for this opened source query.
    pub events: Vec<SourceEvent>,
    /// Retained events this revision removed, and the exact rule that removed
    /// each one. A snapshot that only lists what survives cannot tell a NIP-09
    /// deletion from a capacity eviction, so the cause travels with the
    /// revision that applied it.
    pub retractions: Vec<SourceRetraction>,
}

impl SourceSnapshot {
    /// Empty initial snapshot for a source role.
    #[must_use]
    pub fn empty(kind: SourceKind) -> Self {
        Self {
            kind,
            revision: SourceRevision(0),
            status: SourceStatus::Open,
            events: Vec::new(),
            retractions: Vec::new(),
        }
    }

    /// Current state for a source role, retracting nothing.
    #[must_use]
    pub fn current(kind: SourceKind, revision: SourceRevision, events: Vec<SourceEvent>) -> Self {
        Self {
            kind,
            revision,
            status: SourceStatus::Open,
            events,
            retractions: Vec::new(),
        }
    }
}

/// Future returned by a source observation.
pub type SourceChangeFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SourceSnapshot, QuerySourceClosed>> + Send + 'a>>;

/// Continuous changes belonging to one source open.
pub trait SourceChanges: Send {
    /// Await the next complete source snapshot.
    fn next_change(&mut self) -> SourceChangeFuture<'_>;

    /// Release exactly the work owned by this source observation.
    fn close(&mut self);
}

/// Initial source snapshot and its gapless later sequence.
pub struct OpenedQuerySource {
    /// Complete current state at the open boundary.
    pub initial: SourceSnapshot,
    /// Later complete revisions.
    pub changes: Box<dyn SourceChanges>,
}

/// Neutral contract implemented by independent local source providers.
pub trait QuerySource: Send + Sync {
    /// Open one continuous local observation.
    ///
    /// # Errors
    ///
    /// Returns [`QuerySourceError`] when the provider cannot establish one
    /// coherent initial snapshot plus later revision sequence.
    fn open(&self, query: &Query) -> Result<OpenedQuerySource, QuerySourceError>;
}

/// Local source open refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum QuerySourceError {
    /// Provider is no longer able to open work.
    #[error("query source is closed")]
    Closed,
    /// Provider-specific refusal retained as bounded scoped evidence.
    #[error("query source refused open: {}", .0.as_str())]
    Refused(BoundedText),
}

/// Terminal source observation fact.
///
/// Termination travels on the error channel, not as a final `Ok` snapshot: a
/// provider's last coherent state and the fact that it ended are two different
/// facts, and every production provider drives its later revisions through a
/// lossy latest-state channel that may coalesce a trailing snapshot away. The
/// cause therefore rides the terminal value itself, and the consumer stamps it
/// onto the last coherent snapshot it already holds ([`Self::status`]).
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("query source observation closed: {cause}")]
pub struct QuerySourceClosed {
    /// Why the provider's observation ended.
    pub cause: SourceTerminationCause,
}

impl QuerySourceClosed {
    /// Terminate with an exact cause.
    #[must_use]
    pub const fn new(cause: SourceTerminationCause) -> Self {
        Self { cause }
    }

    /// Fava released this observation.
    #[must_use]
    pub const fn local_close() -> Self {
        Self::new(SourceTerminationCause::LocalClose)
    }

    /// The provider ended its own observation cleanly. Only this cause is
    /// evidence that the provider had nothing further to say.
    #[must_use]
    pub const fn provider_closed() -> Self {
        Self::new(SourceTerminationCause::ProviderClosed)
    }

    /// The provider failed. This proves nothing about the answer.
    #[must_use]
    pub fn provider_failed(detail: impl AsRef<str>) -> Self {
        Self::new(SourceTerminationCause::ProviderFailed {
            detail: BoundedText::new(detail),
        })
    }

    /// The engine is tearing down.
    #[must_use]
    pub const fn shutdown() -> Self {
        Self::new(SourceTerminationCause::Shutdown)
    }

    /// The terminal lifecycle fact to stamp onto the last coherent snapshot.
    #[must_use]
    pub fn status(&self) -> SourceStatus {
        SourceStatus::Closed {
            cause: self.cause.clone(),
        }
    }
}

/// Application-facing event plus exact currently known evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventRecord {
    id: EventId,
    event: EventValue,
    relay_occurrences: RelayOccurrences,
    publication: Option<PublicationEvidence>,
}

impl EventRecord {
    /// Construct a record whose event has a stable deterministic id.
    ///
    /// # Errors
    ///
    /// Returns [`QueryEvaluationError::MissingEventId`] for a non-finalized
    /// unsigned event body.
    pub fn new(
        event: EventValue,
        relay_occurrences: RelayOccurrences,
        publication: Option<PublicationEvidence>,
    ) -> Result<Self, QueryEvaluationError> {
        let id = event.id().ok_or(QueryEvaluationError::MissingEventId)?;
        if id != relay_occurrences.event_id() {
            return Err(QueryEvaluationError::RelayOccurrenceEventMismatch {
                event: id,
                occurrences: relay_occurrences.event_id(),
            });
        }
        Ok(Self {
            id,
            event,
            relay_occurrences,
            publication,
        })
    }

    /// Stable event id.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }

    /// Borrow the event without exposing independent record mutation.
    #[must_use]
    pub const fn event(&self) -> &EventValue {
        &self.event
    }

    /// Borrow the exact event-id-bound relay occurrences.
    #[must_use]
    pub const fn relay_occurrences(&self) -> &RelayOccurrences {
        &self.relay_occurrences
    }

    /// Borrow local accepted publication evidence, when present.
    #[must_use]
    pub const fn publication(&self) -> Option<&PublicationEvidence> {
        self.publication.as_ref()
    }

    /// Event timestamp.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.event.created_at()
    }
}

/// Monotonic revision delivered by one live observation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QueryRevision(pub u64);

/// Complete immutable current query state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySnapshot {
    /// Observation-owned delivered revision.
    pub revision: QueryRevision,
    /// Deduplicated, ordered event records.
    pub events: Arc<[EventRecord]>,
    /// Exact source revisions used for this result.
    pub evidence: QueryEvidence,
}

impl QuerySnapshot {
    /// Construct an evaluated snapshot before observation revision assignment.
    #[must_use]
    pub fn evaluated(events: Vec<EventRecord>, sources: &[SourceSnapshot]) -> Self {
        Self {
            revision: QueryRevision(0),
            events: events.into(),
            evidence: QueryEvidence {
                sources: sources
                    .iter()
                    .map(|source| SourceEvidence {
                        kind: source.kind.clone(),
                        revision: source.revision,
                        status: source.status.clone(),
                        retractions: source.retractions.clone(),
                    })
                    .collect(),
                ..QueryEvidence::default()
            },
        }
    }
}

/// Replaceable strategy for exact local query evaluation.
pub trait QueryEvaluator: Send + Sync {
    /// Evaluate one query over complete current source snapshots.
    /// # Errors
    ///
    /// Returns [`QueryEvaluationError`] for invalid source values or refusal.
    fn evaluate(
        &self,
        query: &Query,
        sources: &[SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError>;
}

/// Scoped local evaluation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum QueryEvaluationError {
    /// A supposedly accepted local event violated the source contract.
    #[error("query source supplied an event without a stable id")]
    MissingEventId,
    /// Relay occurrences were bound to another event id.
    #[error("event {event} cannot carry relay occurrences for {occurrences}")]
    RelayOccurrenceEventMismatch {
        /// Event id being constructed.
        event: EventId,
        /// Event id carried by the occurrence aggregate.
        occurrences: EventId,
    },
    /// Evaluator-specific refusal, retained under a Fava-owned byte bound.
    #[error("query evaluator refused current sources: {}", .0.as_str())]
    Refused(BoundedText),
}
