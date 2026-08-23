# Frozen neutral contracts — Wave 1

**Status:** FROZEN. Five agents implement against this document simultaneously.
Nothing below changes without a written amendment appended to §9.

**Tree:** audit tree `f5922f3`; line citations verified against
`docs/spec/ARCHITECTURE.md` and
`docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` as they exist in the
working tree of branch `architecture/remediation-live-query-ownership`.
Where a per-area audit cites a different line for the same sentence, the audit
was written against an older numbering; the quoted sentence is authoritative,
not the number.

**Authority precedence:** `ARCHITECTURE.md` > `FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md`
> `docs/internals/vocabulary.toml` > this document > audit reports.
This document only *resolves* what those leave open. Every resolution is
marked `DECIDED:`.

**Citation convention:** `ARCH:NNNN` = `docs/spec/ARCHITECTURE.md` line NNNN.
`GOALS:NNNN` = `docs/spec/FULL_FAVA_REWRITE_SPEC_GOALS_AND_OBJECTIVES.md` line NNNN.
`VOCAB:NNNN` = `docs/internals/vocabulary.toml` line NNNN.

**No adapters.** All five contracts are breaking changes. No compatibility
shim, no deprecated alias, no `#[doc(hidden)]` bridge. `crates/fava/src/relay.rs`
and every current call site are expected to stop compiling; Wave 3 rewrites them.

---

## 0. Cross-crate primitives — where the shared nouns live

Three identity types are named by `ARCH:1492-1497` inside `fava-subscriptions`'
contract but are semantically owned by `fava-observe`. A neutral contract crate
must not depend on a lifecycle owner (`ARCH:2984-3016`, dependency direction:
`domain values and pure rules` ← `neutral contracts` ← `universal lifecycle owners`).

`DECIDED: ObservationId, QueryBranchId, QueryBounds, and OperationGeneration are
defined in `fava-query` and re-exported (`pub use`) by `fava-observe`,
`fava-subscriptions`, `fava-transport`, and `fava-diagnostics`.`
Reasoning: `fava-query` is the lowest crate every one of the five already
depends on, so this is the only placement that lets `RelayDemand.owner:
ObservationId` exist without inverting the dependency arrow; `fava-observe`
remains the *semantic* owner because it is the only crate that mints values.

```rust
// crates/fava-query/src/identity.rs  (new module, re-exported from lib.rs)

use std::num::NonZeroU64;

/// Identity of one open Observation. Minted only by `fava-observe`.
///
/// Authority: ARCH:1493 (`RelayDemand.owner: ObservationId`),
/// ARCH:2065 ("observation identity and open/close lifecycle").
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

/// Identity of one branch of a composed Query within one Observation.
///
/// Authority: ARCH:1494 (`RelayDemand.branch: QueryBranchId`);
/// GOALS:401 (QUERY-008) "Per-branch and per-relay evidence MUST remain
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
/// Authority: ARCH:1495 (`RelayDemand.bounds: QueryBounds`);
/// GOALS:1049 (RELAY-003) "MUST NOT merge across differences that would change
/// meaning, including incompatible time windows, relay-side limits".
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
```

`Timestamp` is `fava_state::Timestamp` (re-export of `nostr::types::Timestamp`),
already re-exported through `fava_query`. `NonZeroU32` is `core::num::NonZeroU32`.

**Vocabulary consequence (blocking, do not skip).** `ObservationId` and
`QueryBranchId` are new cross-crate nominal types. `VOCAB:354-367` (`Observation`)
lists no identity symbol. Whoever lands §0 also appends
`fava_query::ObservationId`, `fava_query::QueryBranchId`,
`fava_query::QueryBounds`, `fava_query::OperationGeneration` to the
`Observation` and `Query` term `symbols` arrays. `OperationGeneration` belongs
to the `Observation` term (it is read-side operation identity; the write-side
analogue `MaterializationId` is already at `VOCAB:552`).

---

## 1. `fava-transport`

### 1.1 What the implementer owns

The implementer of `Transport` owns every byte, every socket, every clock the
socket is measured against, and every generation number a session ever wears.
It owns a **registry keyed by `RelaySessionKey`** so that `acquire_session` is
a lookup-then-maybe-dial, not a dial (`ARCH:1593` "current and retiring session
lifecycle"; `GOALS:930` "Several writes for one relay SHOULD share
connection/backoff ownership rather than creating independent reconnect
storms"). It owns the refcount on each registry entry and the deterministic
close that fires when the count reaches zero (`ARCH:1628`). It owns reconnect
policy, backoff, jitter, and attempt exhaustion (`ARCH:1588-1589`, `ARCH:1625`),
and it owns the fact that a reconnect mints a new `OperationGeneration` *inside*
the session object the lease holders already hold, so no holder ever swaps an
`Arc` (`GOALS:1086-1089`, RELAY-006). It owns two bounded byte queues per
session (`ARCH:1590`) and therefore owns the only place in Fava where a full
queue converts into `HandoffOutcome::NotHandedOff` instead of an unbounded park.
It owns *nothing* about query meaning, filters, attribution, route policy, or
durable retry (`GOALS:1082`, RELAY-005). It never decides a deadline value: it
enforces the four durations the caller hands it in `OpenRelaySession`.

### 1.2 Literal contract

```rust
// crates/fava-transport/src/lib.rs

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fava_query::OperationGeneration;
use fava_state::{RelaySessionKey, Timestamp};
use thiserror::Error;

// ---------------------------------------------------------------- futures

/// Future yielding an acquired lease on the current session for one key.
pub type RelaySessionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RelaySessionLease, TransportError>> + Send + 'a>>;

/// Future yielding one correlated byte-handoff outcome.
pub type HandoffFuture<'a> = Pin<Box<dyn Future<Output = HandoffOutcome> + Send + 'a>>;

/// Future yielding the outcome of releasing one lease.
pub type ReleaseFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ReleaseOutcome, TransportError>> + Send + 'a>>;

/// Future yielding one inbound item for one consumer.
pub type RelayInboundFuture<'a> =
    Pin<Box<dyn Future<Output = Result<RelayInbound, TransportError>> + Send + 'a>>;

/// Future yielding transport-wide shutdown completion.
pub type TransportShutdownFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + 'a>>;

// ---------------------------------------------------------------- identity

/// Exact authority of one live connection generation.
///
/// Authority: ARCH:1567-1571 (`fn identity(&self) -> RelaySessionIdentity`),
/// ARCH:1610 "Every inbound frame and handoff completion carries exact session
/// generation and relay-access identity."
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionIdentity {
    /// Relay URL and relay-access authority.
    pub key: RelaySessionKey,
    /// Transport-owned connection generation. Advances on every reconnect.
    pub generation: OperationGeneration,
}

/// Caller-supplied correlation for one exact frame handoff.
///
/// Authority: ARCH:1572-1576 (`send(&self, frame, correlation: HandoffCorrelation)`).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HandoffCorrelation(pub u64);

// ---------------------------------------------------------------- request

/// Fava-owned deadlines for one relay session. Never defaulted by transport.
///
/// Authority: GOALS:424 (QUERY-010) "Timeout, disconnect, retry exhaustion,
/// silence, local cancellation, and relay refusal MUST remain distinct";
/// ARCH:1624 "keepalive and dead-session detection".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportDeadlines {
    /// DNS + TCP + TLS + WebSocket handshake must complete within this.
    pub establish: Duration,
    /// One frame must reach the outbound queue *and* the socket within this.
    pub write: Duration,
    /// Maximum silence before the session is declared dead. A keepalive probe
    /// is the implementer's business; the deadline is not.
    pub idle: Duration,
    /// Close handshake must complete within this; afterwards the session is
    /// dropped and reported closed regardless of the peer.
    pub close: Duration,
}

/// Bounded byte queues for one session, in whole frames.
///
/// Authority: ARCH:1590 "bounded inbound and outbound byte queues";
/// GOALS:1437 (OPS-004) "Exceeding a bound MUST produce refusal, backpressure,
/// or exact shortfall."
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportBounds {
    /// Frames buffered per inbound consumer stream before typed loss.
    pub inbound_frames: NonZeroUsize,
    /// Frames buffered for the socket writer before refusal.
    pub outbound_frames: NonZeroUsize,
    /// Maximum encoded size of a single frame, both directions.
    pub max_frame_bytes: NonZeroUsize,
}

/// Complete acquire request for one relay-access identity.
///
/// Authority: ARCH:1560-1565 (`fn open_session(&self, request: OpenRelaySession)`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRelaySession {
    /// Relay URL and access authority to acquire.
    pub key: RelaySessionKey,
    /// Fava-owned deadlines applied to this session.
    pub deadlines: TransportDeadlines,
    /// Fava-owned queue and frame bounds applied to this session.
    pub bounds: TransportBounds,
    /// Reconnect budget. `None` means reconnect until every lease is released.
    pub reconnect_attempts: Option<NonZeroUsize>,
}

// ---------------------------------------------------------------- traits

/// Replaceable owner of relay-session connection resources.
///
/// Authority: ARCH:1562-1566.
pub trait Transport: Send + Sync {
    /// Acquire a lease on the **current** session for `request.key`, dialing a
    /// new one only when no live session exists for that key.
    ///
    /// Acquiring an existing session MUST NOT open a second socket and MUST
    /// NOT change its generation. The returned lease increments the entry's
    /// holder count.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when establishment refuses, times out, or the
    /// transport is shutting down.
    fn acquire_session<'a>(&'a self, request: OpenRelaySession) -> RelaySessionFuture<'a>;

    /// Current holder count for one key, or `None` when no session is registered.
    /// This is the observable proof that acquire-or-reuse happened.
    fn holders(&self, key: &RelaySessionKey) -> Option<NonZeroUsize>;

    /// Stop accepting acquisitions, close every registered session within
    /// `deadline`, and join owned resources.
    ///
    /// # Errors
    ///
    /// [`TransportError::ShutdownIncomplete`] when sessions remained after
    /// `deadline`; the transport is unusable either way.
    fn shutdown<'a>(&'a self, deadline: Duration) -> TransportShutdownFuture<'a>;
}

/// One exact live connection to a relay, shared by every current lease holder.
///
/// Authority: ARCH:1569-1581.
pub trait RelaySession: Send + Sync {
    /// Current identity. The generation changes under the holder on reconnect.
    fn identity(&self) -> RelaySessionIdentity;

    /// Attempt to hand off one complete frame, correlated.
    ///
    /// MUST NOT park indefinitely: the outbound queue is bounded and
    /// `deadlines.write` applies. A full queue is `NotHandedOff`, never a wait.
    fn send<'a>(&'a self, frame: Vec<u8>, correlation: HandoffCorrelation) -> HandoffFuture<'a>;

    /// Obtain an independently-pollable inbound stream for **this consumer**.
    ///
    /// Two calls return two streams; every inbound item is delivered to every
    /// live stream. One consumer cannot remove an item from another's stream.
    ///
    /// Authority: ARCH:1578 verbatim signature.
    fn messages(&self) -> Box<dyn RelayMessageStream>;

    /// Close this session's current generation deterministically, regardless of
    /// remaining leases. Callers hold leases; this is the transport's own
    /// escape hatch and is idempotent.
    fn close<'a>(&'a self) -> ReleaseFuture<'a>;
}

/// One consumer's bounded view of a session's inbound items.
///
/// Authority: ARCH:1578 (`Box<dyn RelayMessageStream>`).
pub trait RelayMessageStream: Send {
    /// Await the next inbound item for this consumer.
    ///
    /// # Errors
    ///
    /// [`TransportError`] for disconnect, idle-deadline expiry, oversize frame,
    /// or bounded inbound loss.
    fn next_inbound(&mut self) -> RelayInboundFuture<'_>;

    /// Detach this consumer. Idempotent; does not affect other consumers.
    fn close(&mut self);
}

// ---------------------------------------------------------------- inbound

/// One item delivered to one inbound consumer.
///
/// `DECIDED:` the stream carries session-lifecycle transitions alongside frames.
/// Reasoning: GOALS:483-489 (QUERY-015) and ARCH:2092 require a generation
/// change to reach the exact affected observations, and the message stream is
/// the only ordered channel between a session and its holders — a side-channel
/// would reintroduce the ordering ambiguity the generation exists to remove.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayInbound {
    /// One complete relay frame under an exact generation.
    Frame {
        /// Session and generation that produced these bytes.
        identity: RelaySessionIdentity,
        /// Raw frame bytes. Decoding belongs to `fava-wire`.
        frame: Vec<u8>,
        /// Local admission time.
        received_at: Timestamp,
    },
    /// The session disconnected; a reconnect may follow.
    Disconnected {
        /// Generation that ended.
        identity: RelaySessionIdentity,
        /// Exact scoped reason.
        reason: TransportFailure,
    },
    /// A new generation is live. Every holder MUST replay its active demand.
    Reconnected {
        /// Generation that ended.
        previous: RelaySessionIdentity,
        /// Generation now current.
        identity: RelaySessionIdentity,
    },
    /// Reconnect budget is exhausted; no further generation will appear.
    ReconnectExhausted {
        /// Last generation attempted.
        identity: RelaySessionIdentity,
        /// Number of attempts actually made.
        attempts: usize,
        /// Exact reason of the final attempt.
        reason: TransportFailure,
    },
    /// This consumer's bounded inbound queue overflowed. Loss is typed, never
    /// silent (GOALS:434, QUERY-011).
    Lost {
        /// Generation during which items were dropped.
        identity: RelaySessionIdentity,
        /// Exact number of items dropped since the last `Lost`.
        dropped: u64,
    },
}

// ---------------------------------------------------------------- handoff

/// Correlated result of attempting to hand one exact frame to a relay session.
///
/// Authority: ARCH:1599-1604 (three variants), ARCH:1610 (must carry identity).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffOutcome {
    /// Bytes definitely did not leave Fava.
    NotHandedOff {
        /// Session and generation the attempt was made against.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
        /// Exact local refusal reason.
        reason: TransportFailure,
    },
    /// The transport accepted the complete frame for the session.
    HandedOff {
        /// Session and generation that accepted the bytes.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
    },
    /// The transport cannot prove whether the relay received the frame.
    Ambiguous {
        /// Session and generation the attempt was made against.
        identity: RelaySessionIdentity,
        /// Caller's correlation, returned verbatim.
        correlation: HandoffCorrelation,
        /// Exact ambiguity reason.
        reason: TransportAmbiguity,
    },
}

impl HandoffOutcome {
    /// Session and generation this completion belongs to.
    #[must_use]
    pub fn identity(&self) -> &RelaySessionIdentity {
        match self {
            Self::NotHandedOff { identity, .. }
            | Self::HandedOff { identity, .. }
            | Self::Ambiguous { identity, .. } => identity,
        }
    }

    /// Caller correlation this completion belongs to.
    #[must_use]
    pub fn correlation(&self) -> HandoffCorrelation {
        match self {
            Self::NotHandedOff { correlation, .. }
            | Self::HandedOff { correlation, .. }
            | Self::Ambiguous { correlation, .. } => *correlation,
        }
    }
}

/// Reasons bytes definitely did not leave Fava.
///
/// Authority: ARCH:1600-1602 names `TransportFailure` as a distinct type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportFailure {
    /// The session generation is closed.
    SessionClosed,
    /// The outbound queue is full at its declared bound.
    OutboundQueueFull {
        /// Declared bound in frames.
        capacity: usize,
    },
    /// The frame exceeds the declared per-frame byte bound.
    FrameTooLarge {
        /// Exact encoded size.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Establishment did not complete within `TransportDeadlines::establish`.
    EstablishTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// No inbound item within `TransportDeadlines::idle`.
    IdleTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// The relay or the network refused or dropped the connection.
    Disconnected {
        /// Bounded verbatim reason (GOALS:1105, RELAY-008).
        detail: BoundedReason,
    },
    /// Fava is shutting down; no new bytes are admitted.
    ShuttingDown,
}

/// Reasons the transport cannot prove whether bytes reached the relay.
///
/// Authority: ARCH:1600-1602 names `TransportAmbiguity` as a distinct type;
/// ARCH:1606-1608 requires the distinction to survive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportAmbiguity {
    /// The socket accepted the bytes and then errored before flush confirmation.
    FlushUnconfirmed {
        /// Bounded verbatim reason.
        detail: BoundedReason,
    },
    /// `TransportDeadlines::write` expired after the bytes entered the socket.
    WriteTimeout {
        /// The deadline that expired.
        after: Duration,
    },
    /// The session disconnected while the frame was in flight.
    DisconnectedInFlight {
        /// Bounded verbatim reason.
        detail: BoundedReason,
    },
}

/// Result of releasing one lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseOutcome {
    /// Other holders remain; the session stays open.
    Retained {
        /// Holder count after this release.
        holders: NonZeroUsize,
    },
    /// This was the last holder; the session was closed deterministically.
    Closed,
}

// ---------------------------------------------------------------- lease

/// Hook the transport registry installs so a lease can decrement its refcount
/// without the contract crate knowing the registry's shape.
pub trait LeaseRelease: Send + Sync {
    /// Decrement the holder count for `identity`. MUST be non-blocking and
    /// MUST NOT await. Closing, if this was the last holder, is scheduled by
    /// the transport, not performed here.
    fn release_now(&self, identity: &RelaySessionIdentity);

    /// Decrement and drive deterministic close when this was the last holder.
    fn release_deterministically<'a>(
        &'a self,
        identity: &'a RelaySessionIdentity,
    ) -> ReleaseFuture<'a>;
}

/// A refcounted hold on one relay session.
///
/// Authority: ARCH:1593 "current and retiring session lifecycle";
/// GOALS:930 shared connection ownership; ARCH:2072 "ownership/refcounts for
/// shared work" (held by `fava-observe`, expressed through this lease).
pub struct RelaySessionLease {
    session: Arc<dyn RelaySession>,
    registry: Arc<dyn LeaseRelease>,
    identity: RelaySessionIdentity,
    released: bool,
}

impl RelaySessionLease {
    /// Construct a lease. Called only by a `Transport` implementation.
    #[must_use]
    pub fn new(
        session: Arc<dyn RelaySession>,
        registry: Arc<dyn LeaseRelease>,
        identity: RelaySessionIdentity,
    ) -> Self {
        Self {
            session,
            registry,
            identity,
            released: false,
        }
    }

    /// The leased session.
    #[must_use]
    pub fn session(&self) -> &Arc<dyn RelaySession> {
        &self.session
    }

    /// Identity at the moment of acquisition. Use `session().identity()` for
    /// the current generation.
    #[must_use]
    pub fn acquired_identity(&self) -> &RelaySessionIdentity {
        &self.identity
    }

    /// Release deterministically, awaiting close when this is the last holder.
    ///
    /// # Errors
    ///
    /// [`TransportError`] when the close handshake fails or times out.
    pub async fn release(mut self) -> Result<ReleaseOutcome, TransportError> {
        self.released = true;
        let registry = Arc::clone(&self.registry);
        let identity = self.identity.clone();
        registry.release_deterministically(&identity).await
    }
}

impl Drop for RelaySessionLease {
    fn drop(&mut self) {
        if !self.released {
            self.registry.release_now(&self.identity);
        }
    }
}

// ---------------------------------------------------------------- bounded text

/// Relay- or OS-supplied text retained under a Fava-owned byte bound.
///
/// Authority: GOALS:1428 (OPS-004, "frame and message sizes"), GOALS:1105
/// (RELAY-008, verbatim evidence). Truncation is recorded, never silent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedReason {
    text: String,
    truncated_bytes: usize,
}

impl BoundedReason {
    /// Maximum retained bytes. `DECIDED: 512.` Reasoning: long enough for every
    /// NIP-01 `CLOSED`/`NOTICE` prefix that carries a machine-readable reason
    /// word, short enough that 256 retained facts per diagnostics category is a
    /// real memory bound (§4.1).
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

    /// Bytes dropped by the bound. Non-zero means the fact is a shortfall.
    #[must_use]
    pub const fn truncated_bytes(&self) -> usize {
        self.truncated_bytes
    }
}

// ---------------------------------------------------------------- error

/// Scoped transport operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TransportError {
    /// A session could not be established before any handoff.
    #[error("relay session open refused: {0:?}")]
    ConnectionRefused(TransportFailure),
    /// A previously open session disconnected.
    #[error("relay session disconnected: {0:?}")]
    Disconnected(TransportFailure),
    /// The session generation is already closed.
    #[error("relay session generation {} is closed", .0.generation.0)]
    Closed(RelaySessionIdentity),
    /// An inbound frame violated a declared bound.
    #[error("inbound frame of {bytes} bytes exceeds the declared bound {maximum}")]
    InboundFrameTooLarge {
        /// Exact received size.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// This consumer lost bounded inbound items.
    #[error("{dropped} inbound items were dropped for this consumer")]
    InboundLost {
        /// Exact number dropped.
        dropped: u64,
    },
    /// The transport refused new work because it is shutting down.
    #[error("transport is shutting down")]
    ShuttingDown,
    /// Shutdown did not complete within its deadline.
    #[error("{remaining} relay sessions remained open after the shutdown deadline")]
    ShutdownIncomplete {
        /// Sessions still registered when the deadline expired.
        remaining: usize,
    },
}
```

### 1.3 Signatures forced by a specific authority line

| Element | Forced by |
|---|---|
| `fn messages(&self) -> Box<dyn RelayMessageStream>` — verbatim | `ARCH:1578` |
| Three-variant `HandoffOutcome` with `TransportFailure` / `TransportAmbiguity` | `ARCH:1599-1604` |
| Every `HandoffOutcome` variant and every `RelayInbound` variant carries `RelaySessionIdentity` | `ARCH:1610` |
| `send(frame, correlation)` two-argument shape | `ARCH:1572-1576` |
| `OpenRelaySession` as the acquire argument (not a bare key) | `ARCH:1560-1565` |
| `RelaySessionIdentity` as a named type with a generation | `ARCH:1567-1571`, `ARCH:1588` |
| Bounded inbound *and* outbound queues; refusal not park | `ARCH:1590` + `GOALS:1437` |
| Reconnect generation minted inside the session, holders unchanged | `GOALS:1086-1089` (RELAY-006) |
| Refcounted acquire-or-reuse | `ARCH:1593` + `GOALS:930` |
| `Transport::shutdown` | `ARCH:1594` "shutdown and resource joining" |

### 1.4 Decisions

- `DECIDED: frames are Vec<u8>, not String.` Reasoning: `ARCH:1573` says
  `frame: Bytes`; `fava-wire` owns text validity (`ARCH:286-355`), and a
  transport that must produce a `String` has already made a decoding decision
  it does not own. Implementers may use `bytes::Bytes` internally; the contract
  stays dependency-free.
- `DECIDED: RelaySession is Send + Sync, not the bare Send of ARCH:1569.`
  Reasoning: leases are shared by construction (`ARCH:1593`), so `Arc<dyn
  RelaySession>` must cross threads; `Send`-only would make the refcount the
  spec requires unrepresentable.
- `DECIDED: reconnect exhaustion is a stream item, not a silent stop.`
  Reasoning: `GOALS:1066` forbids claiming omitted work was completed; an
  exhausted reconnect that produced no item is exactly that claim.
- `DECIDED: Transport::holders is on the public trait.` Reasoning: the audit's
  shared-work falsifier is unwritable otherwise, and `GOALS:1439-1450` (OPS-005)
  makes test observability part of the product.
- `DECIDED: no keepalive/ping method on the trait.` Reasoning: `ARCH:1624`
  assigns keepalive to the *websocket* implementation, not to the neutral
  contract; the contract only names the `idle` deadline it must satisfy.

---

## 2. `fava-subscriptions`

### 2.1 What the implementer owns

The planner owns the entire mapping from *logical* demand to *wire* subscriptions
and nothing else. It allocates every wire `SubscriptionId` (`ARCH:1508` "planner
input identity"), decides grouping and splitting, and proves that grouping did
not change meaning (`GOALS:1045-1051`, RELAY-003). It owns the diff: given the
complete current demand for one relay and the set currently installed on that
relay's session, it decides which subscriptions to open, which to leave
untouched, and which to close — and the close list *is* withdrawal identity
(`ARCH:1511`, `ARCH:1513`). It owns typed in-plan shortfall: a plan that carries
60 of 64 filters is a `SubscriptionPlan` with four `SubscriptionShortfall`
entries, not an `Err` (`ARCH:1512`, `GOALS:1066`). It owns the conformance rules
that define semantic equivalence (`ARCH:1514`) — which is why `validate_plan`
moves here from `crates/fava/src/relay.rs:224-248`. It owns **no** socket, no
route policy, no observation state, no refcount (the planner is told the truth
about demand and answers; `fava-observe` holds the refcount and decides when a
demand leaves the set).

### 2.2 Literal contract

```rust
// crates/fava-subscriptions/src/lib.rs

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;

use fava_query::{ObservationId, QueryBounds, QueryBranchId};
use fava_state::RelaySessionKey;
use fava_transport::BoundedReason;
use fava_wire::SubscriptionId;
use nostr::filter::Filter;
use thiserror::Error;

// ---------------------------------------------------------------- demand

/// Stable identity of one unit of logical relay demand.
///
/// Two observations of the same query produce two distinct `DemandId`s; one
/// observation's two branches also produce two. This is what lets a grouped
/// EOSE settle more than one logical query.
///
/// Authority: GOALS:1043 (RELAY-002) "The planner MUST preserve attribution
/// from every wire request back to the logical queries it serves."
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DemandId {
    /// Observation that needs this demand.
    pub owner: ObservationId,
    /// Branch within that observation.
    pub branch: QueryBranchId,
}

/// One logical read demand assigned to one exact relay session.
///
/// Authority: ARCH:1492-1497, verbatim field set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayDemand {
    /// Observation that owns this demand.
    pub owner: ObservationId,
    /// Branch of that observation's query.
    pub branch: QueryBranchId,
    /// Exact NIP-01 filter requested from the relay.
    pub filter: Filter,
    /// Whole-query bounds that constrain safe merging.
    pub bounds: QueryBounds,
}

impl RelayDemand {
    /// Construct one exact logical relay demand.
    #[must_use]
    pub const fn new(
        owner: ObservationId,
        branch: QueryBranchId,
        filter: Filter,
        bounds: QueryBounds,
    ) -> Self {
        Self {
            owner,
            branch,
            filter,
            bounds,
        }
    }

    /// Logical identity of this demand.
    #[must_use]
    pub const fn id(&self) -> DemandId {
        DemandId {
            owner: self.owner,
            branch: self.branch,
        }
    }
}

// ---------------------------------------------------------------- constraints

/// One relay-declared read limit, or the honest absence of one.
///
/// Authority: GOALS:1068 (RELAY-004) "Missing, stale, malformed, or unsupported
/// claims remain unknown rather than becoming invented defaults."
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclaredLimit {
    /// The relay declared nothing Fava can interpret deterministically.
    #[default]
    Unknown,
    /// The relay declared this exact limit.
    Declared(NonZeroUsize),
}

impl DeclaredLimit {
    /// The declared value, if any. `None` means unknown — never a default.
    #[must_use]
    pub const fn get(self) -> Option<NonZeroUsize> {
        match self {
            Self::Unknown => None,
            Self::Declared(value) => Some(value),
        }
    }
}

/// Read limits one relay declares, per relay session.
///
/// Authority: ARCH:1488 (`constraints: &RelayReadConstraints`);
/// GOALS:1055-1064 (RELAY-004) enumerates exactly these five.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RelayReadConstraints {
    /// Concurrent wire subscriptions this relay accepts.
    pub max_subscriptions: DeclaredLimit,
    /// Maximum encoded bytes of one client message.
    pub max_message_bytes: DeclaredLimit,
    /// Maximum characters in a subscription id.
    pub max_subscription_id_chars: DeclaredLimit,
    /// Maximum `limit` a filter may request.
    pub max_filter_limit: DeclaredLimit,
    /// `limit` the relay applies when a filter declares none. Its presence
    /// forbids merging filters that declare no limit (GOALS:1049).
    pub default_filter_limit: DeclaredLimit,
}

impl RelayReadConstraints {
    /// Constraints for a relay whose NIP-11 document is absent, stale, or
    /// uninterpretable. Every field is `Unknown`, never invented.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            max_subscriptions: DeclaredLimit::Unknown,
            max_message_bytes: DeclaredLimit::Unknown,
            max_subscription_id_chars: DeclaredLimit::Unknown,
            max_filter_limit: DeclaredLimit::Unknown,
            default_filter_limit: DeclaredLimit::Unknown,
        }
    }
}

// ---------------------------------------------------------------- installed

/// What is currently live on this relay session, as the planner's baseline.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InstalledSubscriptions {
    /// Wire subscriptions accepted on the current generation, and the exact
    /// demand each was installed to serve.
    entries: BTreeMap<SubscriptionId, InstalledSubscription>,
}

/// One wire subscription currently live on a relay session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledSubscription {
    /// Filters carried by the installed REQ.
    pub filters: Vec<Filter>,
    /// Logical demand this subscription was installed to serve.
    pub serves: BTreeSet<DemandId>,
}

impl InstalledSubscriptions {
    /// An empty baseline: a fresh session or a fresh generation after reconnect.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Construct from installed entries.
    #[must_use]
    pub fn from_entries(
        entries: impl IntoIterator<Item = (SubscriptionId, InstalledSubscription)>,
    ) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Installed entry for one wire id.
    #[must_use]
    pub fn get(&self, id: &SubscriptionId) -> Option<&InstalledSubscription> {
        self.entries.get(id)
    }

    /// Every installed wire id, ascending.
    pub fn ids(&self) -> impl Iterator<Item = &SubscriptionId> {
        self.entries.keys()
    }

    /// Number of installed wire subscriptions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is installed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ---------------------------------------------------------------- plan

/// Monotonic identity of one desired plan for one relay session.
///
/// Authority: ARCH:1511 "plan diff values"; GOALS:426 (QUERY-010) stale
/// completion rejection.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanRevision(pub u64);

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
        self.entries.get(id).map_or_else(
            || EMPTY.get_or_init(BTreeSet::new),
            |entry| &entry.serves,
        )
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

// ---------------------------------------------------------------- trait

/// Replaceable mapping from logical demand to exact Nostr subscriptions.
///
/// Authority: ARCH:1483-1490.
pub trait SubscriptionPlanner: Send + Sync {
    /// Produce the desired plan for one relay session.
    ///
    /// `demand` is the **complete current** logical demand for `relay` across
    /// every observation. An empty `demand` with a non-empty `installed` is a
    /// legal call whose correct answer is a plan that closes everything.
    ///
    /// # Errors
    ///
    /// [`SubscriptionPlanError`] only for inputs the planner cannot process at
    /// all. Demand the planner *understands* but cannot carry is a
    /// [`SubscriptionShortfall`] inside `Ok`, never an error.
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
        installed: &InstalledSubscriptions,
        revision: PlanRevision,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError>;
}

/// Exact subscription planning refusal. Reserved for malformed input.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SubscriptionPlanError {
    /// Two demands in one call carry the same `DemandId`.
    #[error("duplicate logical demand: observation {:?} branch {:?}", .0.owner, .0.branch)]
    DuplicateDemand(DemandId),
    /// The planner allocated the same wire id twice within one plan.
    #[error("duplicate wire subscription id: {0}")]
    DuplicateSubscription(SubscriptionId),
    /// Exact Nostr REQ encoding failed before any handoff.
    #[error("REQ encoding failed: {0:?}")]
    Encoding(BoundedReason),
}

// ---------------------------------------------------------------- conformance

/// Conformance rules that define semantic equivalence for any planner.
///
/// This function is the whole of the contract's conformance obligation. It
/// replaces the private `validate_plan` at `crates/fava/src/relay.rs:224-248`,
/// which must be deleted, not adapted.
///
/// Authority: ARCH:1514 "the conformance rules that define semantic
/// equivalence"; ARCH:3148 external providers "pass the same conformance kit as
/// the standard provider".
///
/// # Errors
///
/// [`PlanConformanceError`] for the first violated rule, in declaration order.
pub fn validate_plan(
    relay: &RelaySessionKey,
    demand: &[RelayDemand],
    constraints: &RelayReadConstraints,
    installed: &InstalledSubscriptions,
    plan: &SubscriptionPlan,
) -> Result<(), PlanConformanceError> {
    let _ = (relay, demand, constraints, installed, plan);
    unimplemented!("Wave 1 implementer: enforce C1..C11 below")
}

/// A violated planner conformance rule.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PlanConformanceError {
    /// C1: the plan names a different relay than the one it was asked about.
    #[error("plan is scoped to the wrong relay session")]
    WrongRelay,
    /// C2: `open`, `retain`, and `close` are not pairwise disjoint.
    #[error("wire subscription {0} appears in more than one diff bucket")]
    OverlappingBuckets(SubscriptionId),
    /// C3: `open` names an id that is already installed.
    #[error("plan opens already-installed subscription {0}")]
    ReopenedInstalled(SubscriptionId),
    /// C4: `retain` or `close` names an id that is not installed.
    #[error("plan references subscription {0} that is not installed")]
    UnknownInstalled(SubscriptionId),
    /// C5: attribution keys are not exactly `open` ∪ `retain`.
    #[error("attribution does not describe the resulting installed set")]
    AttributionMismatch,
    /// C6: a `PlannedSubscription` carries no filters.
    #[error("planned subscription {0} carries no filter")]
    EmptyFilters(SubscriptionId),
    /// C7: attribution filters for a wire id differ from its planned filters.
    #[error("attribution filters for {0} do not match the planned REQ")]
    FilterAttributionMismatch(SubscriptionId),
    /// C8: some `DemandId` in the input is neither attributed nor a shortfall.
    #[error("demand {:?}/{:?} is neither served nor reported as shortfall", .0.owner, .0.branch)]
    DemandUnaccounted(DemandId),
    /// C9: attribution names a `DemandId` that was not in the input demand.
    #[error("attribution invents demand {:?}/{:?}", .0.owner, .0.branch)]
    DemandInvented(DemandId),
    /// C10: the resulting installed count exceeds a *declared* maximum.
    #[error("plan installs {installed} subscriptions but the relay declared {maximum}")]
    DeclaredSubscriptionsExceeded {
        /// Count after execution.
        installed: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// C11: a wire id is longer than a *declared* id-length maximum.
    #[error("subscription id {id} exceeds the declared length {maximum}")]
    DeclaredIdLengthExceeded {
        /// Offending id.
        id: SubscriptionId,
        /// Declared maximum.
        maximum: usize,
    },
}
```

### 2.3 Conformance rules: what moved, what was deleted

`crates/fava/src/relay.rs:224-248` enforced nine private assumptions. Their
disposition is frozen:

| Old rule | Disposition |
|---|---|
| 1 `plan.relay == expected` | **Kept** as C1 (`ARCH:1518` — routing already chose the relay) |
| 2 `!attribution.is_empty()` | **Deleted.** A close-only plan is legal (`ARCH:1513`) |
| 3 `!messages.is_empty()` | **Deleted.** Same reason; also a pure-`retain` plan is legal |
| 4 `demand.keys() == attribution.keys()` | **Replaced** by C5/C8/C9, which relate attribution to the *resulting installed set* and to input demand, not to a second map |
| 5 no empty demand value | **Replaced** by C6 (a planned subscription must carry a filter) and C8 |
| 6 every message is `Req` | **Deleted.** The plan is `Vec<PlannedSubscription>`, not `Vec<ClientMessage>`; frame construction is the executor's |
| 7 exactly one filter per REQ | **Deleted.** NIP-01 permits many; nothing in `ARCH` or RELAY-002/003/004 requires one |
| 8 `attribution[id] == filters[0]` | **Replaced** by C7, which compares the whole filter vector |
| 9 refusal is `String` | **Deleted.** `PlanConformanceError` is typed (`GOALS:1389`, OPS-001) |
| — | **New:** C2, C3, C4 (diff integrity), C10, C11 (declared-limit integrity) |

### 2.4 Signatures forced by a specific authority line

| Element | Forced by |
|---|---|
| `RelayDemand { owner, branch, filter, bounds }` — exact field set | `ARCH:1492-1497` |
| `constraints: &RelayReadConstraints` parameter | `ARCH:1488` |
| `shortfalls` inside a successful plan | `ARCH:1502`, `ARCH:1512`, `ARCH:1536` |
| `close: Vec<WithdrawnSubscription>` | `ARCH:1513` |
| `retain` / diff shape | `ARCH:1511` "plan diff values" |
| `SubscriptionAttribution` as a named type mapping wire → many `DemandId` | `ARCH:1501` + `GOALS:1043` |
| `DeclaredLimit::Unknown` rather than a numeric default | `GOALS:1068` |
| `validate_plan` living in this crate | `ARCH:1514`, `ARCH:3148` |
| `demand` is the complete set for the relay | `ARCH:1478`, `GOALS:1041` |

### 2.5 Decisions

- `DECIDED: the return type keeps the name SubscriptionPlan while carrying the
  diff, rather than introducing SubscriptionPlanDiff.` Reasoning: `ARCH:1499`
  and `VOCAB:681` name `SubscriptionPlan` as an approved `spec_symbol`; adding a
  second noun would require a vocabulary change for a value the spec already
  names, and `ARCH:1511` puts "plan diff values" inside this crate's *owned
  meaning* rather than in a separate type.
- `DECIDED: plan() takes revision as a parameter instead of the planner minting
  it.` Reasoning: `ARCH:2922` — `fava-observe` owns the desired plan, the
  planner only computes it; a planner that minted revisions would own plan
  identity.
- `DECIDED: SubscriptionPlanError shrinks to three variants;
  TooManySubscriptions and FrameTooLarge move into ShortfallReason.` Reasoning:
  `ARCH:1536` requires "typed shortfall when exact execution does not fit", and
  an error annihilates the 60 subscriptions that did fit.
- `DECIDED: RelayReadConstraints lives in fava-subscriptions, not fava-nip11.`
  Reasoning: it is the planner's contract input (`ARCH:1488`); a NIP-11 service
  produces it, and a service crate depending on a contract crate is the legal
  direction (`ARCH:2984-3016`).
- `DECIDED: no NIP-11 acquisition is in scope for Wave 1.` Reasoning: with
  `RelayReadConstraints::unknown()` the planner is correct-by-absence today;
  `no-nip11-invented-planner-limits` is a Wave 5 defect and must not gate this
  contract.
- `DECIDED: fava-subscriptions depends on fava-transport for BoundedReason
  only.` Reasoning: contract-to-contract edges are legal and one bounded-text
  type is preferable to two; if the implementer prefers zero coupling, move
  `BoundedReason` to `fava-query` — that is the only permitted variation, and it
  must be taken by §1 and §2 together or not at all.

---

## 3. `fava-query`

### 3.1 What the implementer owns

`fava-query` owns the *vocabulary* of evidence, not the facts. It owns the
guarantee that an application can tell "this relay told us it has nothing" from
"we never reached this relay" from "this relay refused us" from "we stopped
asking" (`GOALS:414`, `GOALS:422-426`). It owns the third source role: a live
admitted relay occurrence is a first-class merge input whether or not a cache
retained it (`GOALS:344-350`, QUERY-005). It owns per-branch scoping so
overlapping branches deliver one `EventRecord` while keeping separate EOSE,
error, and auth state (`GOALS:401-403`). It owns the honest report of bounded
loss inside the snapshot rather than through a side channel (`GOALS:434`). It
owns **no** relay session, no plan, no refcount: every relay-scoped value here
is a *report* written by `fava-observe` from facts owned elsewhere.

### 3.2 Literal contract

```rust
// crates/fava-query/src/evidence.rs  (new module, re-exported from lib.rs)

use std::num::NonZeroUsize;

use fava_state::{RelaySessionKey, Timestamp};

use crate::identity::{ObservationId, OperationGeneration, QueryBranchId};
use crate::SourceRevision;

/// Role of one contribution to the universal query merge.
///
/// `LiveRelay` is the third role required by GOALS:344-350 (QUERY-005):
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
    /// The engine is shutting down (distinct from a source failure, GOALS:302).
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
///
/// This type is the whole answer to `query-evidence-cannot-name-relays`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayQueryEvidence {
    /// Relay and access authority.
    pub session: RelaySessionKey,
    /// Transport connection generation these facts belong to.
    pub generation: OperationGeneration,
    /// Desired-plan revision under which this relay's demand was requested.
    /// A completion carrying an older revision is stale (GOALS:426).
    pub plan_revision: u64,
    /// Query branches whose demand this relay currently carries.
    pub branches: Vec<QueryBranchId>,
    /// Current state of this relay's contribution.
    pub state: RelaySourceState,
    /// Observations sharing the wire work behind this relay's demand,
    /// including this one (ARCH:2072, GOALS:294-298).
    pub shared_with: Vec<ObservationId>,
    /// Demand for this relay the current plan does not carry.
    pub shortfall: Option<RelayShortfall>,
    /// Whether this relay entered the query through automatic routing or an
    /// explicit relay set (GOALS:473-481, QUERY-014).
    pub route: RouteOrigin,
}

impl RelayQueryEvidence {
    /// Whether this relay has actually sent EOSE for the exact current request.
    ///
    /// The single predicate GOALS:420-426 (QUERY-010) exists to protect: it is
    /// true only for [`RelaySourceState::StoredEventsComplete`].
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
/// by the same underlying fact (GOALS:422).
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
        /// Verbatim, bounded relay text (GOALS:1105, RELAY-008).
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
    /// No router still contributes this destination (GOALS:479).
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

// ------------------------------------------------------------ plan evidence

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

// ------------------------------------------------------------ shortfall

/// Query-scoped loss or limit that is not attributable to one relay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryShortfall {
    /// Intermediate revisions were superseded before delivery. Bounded loss is
    /// explicit and typed (GOALS:434, QUERY-011).
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
/// Authority: ARCH:700-716 (`QuerySnapshot.evidence`), GOALS:393-403
/// (QUERY-008), GOALS:403-418 (QUERY-009), GOALS:420-428 (QUERY-010).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryEvidence {
    /// Latest scoped local source facts included in this exact result.
    pub sources: Vec<SourceEvidence>,
    /// Latest scoped relay facts for every relay this query has used or
    /// intends to use.
    pub relays: Vec<RelayQueryEvidence>,
    /// The desired plan behind the relay facts, when this query has relay demand.
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
        relay: &'a fava_state::RelayUrl,
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
    /// request. Never a claim about the network (GOALS:403-418, QUERY-009).
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
```

### 3.3 The live-relay source variant

`SourceKind::LiveRelay { session }` is not decorative. The Wave 3 implementer
must be able to construct a `SourceSnapshot { revision, events }` whose `kind`
is `LiveRelay` and feed it into `QueryEvaluator::update` alongside cache and
write-store snapshots. Two consequences are frozen here:

1. `SourceEvent` gains no variant. A live relay occurrence is a
   `SourceEvent::Cached(CachedEvent)` carried by a `LiveRelay`-kinded snapshot.
   `DECIDED:` reasoning — `CachedEvent` already carries the signed event and its
   `RelayEvidence`, and the never-copy-local-writes rule is enforced by
   `CachedEvent`'s constructor; adding a variant would duplicate that guard.
2. A `LiveRelay` source is **ephemeral by contract**: its revision advances only
   while the observation is open, and a newly opened query sees nothing from it
   (`GOALS:348` "A newly opened query later sees only what its configured local
   sources still retain").

### 3.4 Signatures forced by a specific authority line

| Element | Forced by |
|---|---|
| `SourceKind::LiveRelay` | `GOALS:344-350` (QUERY-005) |
| `RelaySourceState` variants kept mutually exclusive | `GOALS:422` (QUERY-010) |
| `stored_events_complete()` true only for an actual EOSE | `GOALS:421-424` |
| `plan_revision` on relay evidence | `GOALS:426` |
| `branches: Vec<QueryBranchId>` on relay evidence | `GOALS:401` (QUERY-008) |
| `shared_with: Vec<ObservationId>` | `ARCH:2072`, `GOALS:294-298` (QUERY-002) |
| `QueryShortfall::CoalescedUpdates` inside the snapshot | `GOALS:434` (QUERY-011) |
| `SourceTerminationCause` | `ARCH:724` merge rule 5 |
| `AuthenticationState` distinct from failure | `GOALS:1091-1104` (RELAY-007) |
| No `synced`/`complete`/percentage accessor anywhere | `GOALS:405-414` (QUERY-009) |

### 3.5 Decisions

- `DECIDED: SourceKind and SourceEvidence lose Copy.` Reasoning: `LiveRelay`
  carries a `RelaySessionKey`; `Copy` is unrecoverable and no caller depends on
  it that Wave 3 does not already rewrite.
- `DECIDED: plan_revision and RouteOrigin::Automatic.revision are u64, not the
  newtypes from fava-subscriptions / fava-routing.` Reasoning: `fava-query` sits
  *below* both contract crates in the dependency order (`ARCH:2984-3016`);
  importing either would invert the arrow. The owner converts.
- `DECIDED: BoundedText is duplicated rather than shared with
  fava_transport::BoundedReason.` Reasoning: same as above — `fava-query` must
  keep zero contract-crate dependencies. The two types are byte-identical in
  behavior and both cap at 512.
- `DECIDED: QueryEvidence exposes accessors, not a builder.` Reasoning: only
  `fava-observe` constructs it; public struct fields plus read accessors keep
  the type inspectable from tests (`GOALS:1439`, OPS-005) without a second API.

---

## 4. `fava-diagnostics`

### 4.1 What the implementer owns

`fava-diagnostics` owns a bounded, current, typed snapshot and **nothing else**
— no policy, no health score, no aggregation that turns facts into a verdict
(`ARCH:2250`, `GOALS:1387`). It owns the bound in **both dimensions**: at most
`capacity` facts per category *and* at most `BoundedText::MAX_BYTES` of
externally-supplied text per fact, so retention is a real number of bytes rather
than `256 × unbounded` (`GOALS:1428-1437`, OPS-004). It owns the ownership graph
of open observations: which observation, bound to which route revision, holding
which logical demand, under which desired plan, sharing which wire work with how
many peers, short of what, and waiting on which provider operation. It owns
publication **from every owner**, not from the facade: the `Diagnostics` handle
is `Send + Sync` and each owner writes its own facts (`ARCH:2254` "Each owner
publishes structured diagnostic facts").

### 4.2 Literal contract

```rust
// crates/fava-diagnostics/src/lib.rs

use std::num::NonZeroUsize;
use std::time::Duration;

use fava_query::{
    BoundedText, ObservationId, OperationGeneration, QueryBranchId, RelaySourceState,
};
use fava_state::RelaySessionKey;
use fava_wire::SubscriptionId;

/// Bounded exact current facts published by Fava owners.
///
/// Authority: ARCH:2269-2275, verbatim five-category shape.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiagnosticsSnapshot {
    /// One entry per relay session Fava currently holds.
    pub relays: Vec<RelayDiagnostic>,
    /// One entry per open observation. This is the ownership graph.
    pub queries: Vec<QueryDiagnostic>,
    /// One entry per write that is not settled.
    pub writes: Vec<WriteDiagnostic>,
    /// One entry per provider currently executing or recently failed.
    pub providers: Vec<ProviderDiagnostic>,
    /// One entry per bound that refused, backpressured, or fell short.
    pub limits: Vec<LimitDiagnostic>,
    /// Facts dropped by the per-category count bound since construction.
    /// A bound that discards must say so (GOALS:1437).
    pub dropped_facts: DroppedFacts,
}

/// Per-category count of facts the bound discarded.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DroppedFacts {
    /// Relay facts dropped.
    pub relays: u64,
    /// Query facts dropped.
    pub queries: u64,
    /// Write facts dropped.
    pub writes: u64,
    /// Provider facts dropped.
    pub providers: u64,
    /// Limit facts dropped.
    pub limits: u64,
}

// ---------------------------------------------------------------- relays

/// Current state of one relay session Fava holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayDiagnostic {
    /// Relay and access authority.
    pub session: RelaySessionKey,
    /// Current transport connection generation.
    pub generation: OperationGeneration,
    /// Current session state.
    pub state: RelaySessionState,
    /// Lease holders on this session — the shared-work refcount.
    pub holders: usize,
    /// Wire subscriptions currently installed on this session.
    pub subscriptions: Vec<WireSubscriptionDiagnostic>,
    /// Reconnect attempts made on this key since the last success.
    pub reconnect_attempts: usize,
}

/// State of one relay session, independent of any query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelaySessionState {
    /// Establishing.
    Connecting,
    /// Live.
    Open,
    /// Dropped; reconnect in progress.
    Reconnecting {
        /// Bounded reason for the drop.
        detail: BoundedText,
    },
    /// Reconnect exhausted.
    Unreachable {
        /// Bounded reason of the final attempt.
        detail: BoundedText,
    },
    /// Closed deterministically.
    Closed,
}

/// One wire subscription installed on one session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireSubscriptionDiagnostic {
    /// Wire id.
    pub id: SubscriptionId,
    /// Observations whose demand it serves — grouped-EOSE fan-out, visible.
    pub serves: Vec<ObservationId>,
    /// Whether an EOSE has arrived for this exact wire id and generation.
    pub stored_events_complete: bool,
    /// Verbatim, bounded CLOSED text if the relay refused it.
    pub closed: Option<BoundedText>,
}

// ---------------------------------------------------------------- queries

/// The complete ownership record for one open observation.
///
/// Authority: ARCH:2254-2262 "open observation and route ownership".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryDiagnostic {
    /// Observation identity.
    pub observation: ObservationId,
    /// Route revision currently bound to this observation, when automatic.
    pub route_revision: Option<u64>,
    /// Relay destinations the bound route revision names.
    pub route_relays: Vec<RelaySessionKey>,
    /// Logical demand this observation currently holds, per relay per branch.
    pub demand: Vec<LogicalDemandDiagnostic>,
    /// Desired-plan revision currently installed for this observation.
    pub plan_revision: Option<u64>,
    /// Wire subscriptions this observation currently relies on.
    pub wire: Vec<ObservationWireBinding>,
    /// Source shortfalls scoped to this observation.
    pub shortfalls: Vec<BoundedText>,
    /// Provider operation this observation is currently waiting on.
    pub pending_operation: Option<ProviderOperation>,
    /// Query revisions superseded before delivery.
    pub coalesced_updates: u64,
}

/// One relay's worth of one observation's logical demand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalDemandDiagnostic {
    /// Relay session this demand is assigned to.
    pub session: RelaySessionKey,
    /// Branch that needs it.
    pub branch: QueryBranchId,
    /// Current state of this relay's contribution to this observation.
    pub state: RelaySourceState,
}

/// Binding from one observation to one shared wire subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationWireBinding {
    /// Relay session.
    pub session: RelaySessionKey,
    /// Wire subscription id.
    pub subscription: SubscriptionId,
    /// Total observations sharing this wire subscription, including this one.
    pub shared_holders: NonZeroUsize,
}

// ---------------------------------------------------------------- writes

/// One write that has not settled.
///
/// Authority: GOALS:1408-1418 (OPS-003).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteDiagnostic {
    /// Receipt identity, rendered by the write-store owner.
    pub receipt: BoundedText,
    /// Single classification of why it is stuck.
    pub classification: WriteStall,
    /// How long it has been in this classification.
    pub stuck_for: Duration,
}

/// The one classification a stalled write carries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriteStall {
    /// No route has been resolved yet.
    Unrouted,
    /// No signer is available for the required author.
    Unsignable,
    /// Routed and signed, awaiting handoff.
    AwaitingDelivery,
    /// Delivery attempts are being retried.
    Retrying {
        /// Attempts made.
        attempts: usize,
    },
    /// Delivery is exhausted and no further attempt is scheduled.
    Undeliverable {
        /// Bounded reason.
        detail: BoundedText,
    },
}

// ---------------------------------------------------------------- providers

/// A provider operation Fava has authorized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderDiagnostic {
    /// Which provider.
    pub provider: ProviderKind,
    /// The operation.
    pub operation: ProviderOperation,
    /// Its current disposition.
    pub state: ProviderOperationState,
}

/// The replaceable providers Fava calls.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProviderKind {
    /// `EventCache`.
    EventCache,
    /// `WriteStore`.
    WriteStore,
    /// `FetchCache`.
    FetchCache,
    /// `QueryEvaluator`.
    QueryEvaluator,
    /// One `Router`.
    Router,
    /// `SubscriptionPlanner`.
    SubscriptionPlanner,
    /// `Transport`.
    Transport,
    /// `Publisher`.
    Publisher,
    /// `DeliveryPolicy`.
    DeliveryPolicy,
    /// `Signer`.
    Signer,
    /// A protocol service (NIP-05, NIP-11).
    Service,
}

/// Identity of one authorized provider operation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderOperation {
    /// Provider instance name supplied at assembly, bounded.
    pub instance: BoundedText,
    /// Generation of this operation slot. Late completions carrying an older
    /// generation are stale.
    pub generation: OperationGeneration,
}

/// Disposition of one provider operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderOperationState {
    /// Running, with the Fava-owned deadline it must beat.
    Running {
        /// Deadline supplied by the owner.
        deadline: Duration,
        /// Elapsed so far.
        elapsed: Duration,
    },
    /// Completed within its deadline.
    Completed,
    /// The deadline expired.
    TimedOut {
        /// The deadline that expired.
        after: Duration,
    },
    /// The provider returned an error.
    Failed {
        /// Bounded reason.
        detail: BoundedText,
    },
    /// The provider panicked and was isolated.
    Panicked {
        /// Bounded panic payload.
        detail: BoundedText,
    },
    /// The owner cancelled it.
    Cancelled,
}

// ---------------------------------------------------------------- limits

/// One bound that refused, backpressured, or fell short.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitDiagnostic {
    /// Which bound.
    pub bound: BoundKind,
    /// What the bound was.
    pub limit: usize,
    /// What was required.
    pub required: usize,
    /// Scope the shortfall is attributable to.
    pub scope: LimitScope,
}

/// The externally-influenced resources Fava bounds (GOALS:1420-1437).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundKind {
    /// Concurrent relay sessions.
    RelaySessions,
    /// Wire subscriptions on one relay.
    WireSubscriptions,
    /// Outbound frame bytes.
    OutboundFrameBytes,
    /// Inbound frame bytes.
    InboundFrameBytes,
    /// Outbound queue depth.
    OutboundQueue,
    /// Inbound queue depth.
    InboundQueue,
    /// Router fan-out.
    RouteFanOut,
    /// Event-cache capacity.
    EventCacheCapacity,
    /// Write-store active work.
    WriteStoreActiveWork,
    /// Observation delivery queue.
    ObservationDelivery,
    /// Diagnostics retention.
    DiagnosticsRetention,
    /// Provider operation concurrency.
    ProviderOperations,
}

/// What a limit shortfall is attributable to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LimitScope {
    /// Engine-wide.
    Engine,
    /// One relay session.
    Relay {
        /// The session.
        session: RelaySessionKey,
    },
    /// One observation.
    Observation {
        /// The observation.
        observation: ObservationId,
    },
    /// One provider operation.
    Provider {
        /// The operation.
        operation: ProviderOperation,
    },
}
```

The publishing surface (`Diagnostics`) keeps `Diagnostics::bounded(NonZeroUsize)`
and gains one typed publish method per category, each taking the owned fact by
value and each callable from any owner. Its exact method list is left to the
implementer with one frozen constraint: **there is no method taking a bare
`String`, and no method taking a `&str` that is not immediately wrapped in
`BoundedText`.** `Diagnostics::default()` stays 256 per category, and
`Fava::builder()` gains `.diagnostics_capacity(NonZeroUsize)`.

### 4.3 Signatures forced by a specific authority line

| Element | Forced by |
|---|---|
| Exactly five categories named `relays`/`queries`/`writes`/`providers`/`limits` | `ARCH:2269-2275` verbatim |
| `QueryDiagnostic.observation` | `ARCH:2254` "open observation and route ownership" |
| `route_revision` + `route_relays` on the query record | `ARCH:2254` |
| `demand` on the query record | `ARCH:2071` |
| `shared_holders` / `RelayDiagnostic.holders` | `ARCH:2072` |
| `pending_operation` with a generation | `ARCH:2262` "signer and auth availability", `ARCH:2933` |
| `WriteStall` as a single classification | `GOALS:1408-1418` (OPS-003) |
| `BoundedText` on every externally-supplied string | `GOALS:1428-1437` (OPS-004) |
| `DroppedFacts` | `GOALS:1437` "MUST NOT silently discard work while claiming success" |
| No health score, no percentage | `GOALS:1387-1400` (OPS-001) |

### 4.4 Decisions

- `DECIDED: DiagnosticsSnapshot carries DroppedFacts, which ARCH:2269 does not
  list.` Reasoning: `ARCH:2277` calls the output "a bounded latest-state
  stream", and `GOALS:1437` forbids silent discard; without this field the count
  bound is itself a silent discard.
- `DECIDED: coalesced_query_updates moves from a global counter to a per-
  observation field.` Reasoning: `GOALS:434` scopes loss to the causal stream
  that lost it; a global counter cannot attribute.
- `DECIDED: the current eleven flat Vec fields are deleted outright.`
  Reasoning: no adapters (Wave 1 rule); every one of them is expressible inside
  the five categories, and keeping both would leave two truths.
- `DECIDED: fava-diagnostics depends on fava-query, fava-state, fava-wire only.`
  Reasoning: it must not depend on `fava-transport` or `fava-subscriptions`, or
  a diagnostics change could force a transport rebuild; `RelaySourceState` and
  `OperationGeneration` come from `fava-query`, and revisions arrive as `u64`.

---

## 5. `fava-runtime`

### 5.1 What the implementer owns

`fava-runtime` owns every task Fava starts and every task Fava must join
(`ARCH:2288`, `ARCH:2298`). It owns the join registry, so `Fava::close()` can
prove that no Fava-started task outlives it. It owns bounded command channels,
so an owner's mailbox has a declared depth and a full mailbox is a typed refusal
rather than a park (`ARCH:2290`, `GOALS:1437`). It owns the deadline wrapped
around **every** provider invocation — `ARCH:2306` "A stalled provider has
bounded influence and cannot block unrelated owner progress or Fava shutdown
indefinitely" — and it owns panic isolation, so an application-supplied provider
that panics becomes a typed completion rather than an aborted owner. It owns
cancellation tokens and their propagation (`ARCH:2297`). It owns **no** meaning:
it never inspects an event kind, chooses a route, evaluates a query, or writes
durable state (`ARCH:2302`).

`fava-runtime` is a **universal owner, not a replaceable provider**. It exposes
concrete types, not a trait to implement. `VOCAB:270` lists it under `Fava`'s
`spec_crates`, not as its own replaceable term.

### 5.2 Literal contract

```rust
// crates/fava-runtime/src/lib.rs

use std::fmt;
use std::future::Future;
use std::num::NonZeroUsize;
use std::time::Duration;

use fava_query::OperationGeneration;
use thiserror::Error;

/// Owner of every task, timer, channel, deadline, and join in one Fava engine.
///
/// Authority: ARCH:2284-2298.
#[derive(Clone)]
pub struct Runtime {
    /* private fields */
}

/// Configuration supplied once at engine construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    /// Default depth for bounded channels created without an explicit depth.
    pub default_channel_depth: NonZeroUsize,
    /// Maximum concurrently tracked spawned tasks. Exceeding it refuses.
    pub max_tasks: NonZeroUsize,
    /// Maximum concurrently running provider operations.
    pub max_provider_operations: NonZeroUsize,
}

impl Runtime {
    /// Construct a runtime on the ambient async executor.
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let _ = config;
        unimplemented!("Wave 2")
    }

    // ------------------------------------------------------------ spawning

    /// Spawn owned work and register it for shutdown join.
    ///
    /// The returned [`TaskHandle`] is the owner's grip; the join registry keeps
    /// its own so shutdown can join work whose handle was dropped.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::TaskLimit`] at `max_tasks`;
    /// [`RuntimeError::ShuttingDown`] after shutdown began.
    pub fn spawn<F>(&self, name: TaskName, future: F) -> Result<TaskHandle<F::Output>, RuntimeError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let _ = (name, future);
        unimplemented!("Wave 2")
    }

    /// Spawn work bound to a cancellation token; cancelling the token drives
    /// the task to completion promptly.
    ///
    /// # Errors
    ///
    /// As [`Runtime::spawn`].
    pub fn spawn_cancellable<F>(
        &self,
        name: TaskName,
        token: CancellationToken,
        future: F,
    ) -> Result<TaskHandle<Option<F::Output>>, RuntimeError>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let _ = (name, token, future);
        unimplemented!("Wave 2")
    }

    // ------------------------------------------------------------ channels

    /// Create a bounded command channel.
    #[must_use]
    pub fn channel<T: Send + 'static>(&self, depth: NonZeroUsize) -> (Sender<T>, Receiver<T>) {
        let _ = depth;
        unimplemented!("Wave 2")
    }

    // ------------------------------------------------------------ providers

    /// Invoke one application-supplied provider call under a Fava-owned
    /// deadline, outside every owner lock, with panic isolation.
    ///
    /// This is the only sanctioned way to await a provider. A bare `.await` on
    /// a `dyn` provider anywhere in a lifecycle owner is a contract violation.
    ///
    /// Authority: ARCH:2306.
    pub async fn call_provider<T, F>(
        &self,
        operation: OperationName,
        generation: OperationGeneration,
        deadline: Duration,
        call: F,
    ) -> ProviderCompletion<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let _ = (operation, generation, deadline, call);
        unimplemented!("Wave 2")
    }

    // ------------------------------------------------------------ time

    /// A cancellation token rooted in this runtime's shutdown token.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        unimplemented!("Wave 2")
    }

    /// Sleep on the runtime's clock. Test builds may drive it deterministically.
    pub async fn sleep(&self, duration: Duration) {
        let _ = duration;
        unimplemented!("Wave 2")
    }

    // ------------------------------------------------------------ shutdown

    /// Refuse new spawns and channels, cancel the root token, and join every
    /// registered task within `deadline`.
    ///
    /// Authority: ARCH:2298 "resource joining and shutdown deadlines";
    /// GOALS:1486-1496 (OPS-009).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::ShutdownIncomplete`] naming the tasks that did not join.
    pub async fn shutdown(&self, deadline: Duration) -> Result<(), RuntimeError> {
        let _ = deadline;
        unimplemented!("Wave 2")
    }

    /// Names of tasks currently registered. The shutdown-join falsifier reads
    /// this.
    #[must_use]
    pub fn outstanding_tasks(&self) -> Vec<TaskName> {
        unimplemented!("Wave 2")
    }
}

// ---------------------------------------------------------------- names

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

// ---------------------------------------------------------------- tasks

/// An owner's grip on one spawned task.
pub struct TaskHandle<T> {
    /* private fields */
}

impl<T> TaskHandle<T> {
    /// Name of the task.
    #[must_use]
    pub fn name(&self) -> TaskName {
        unimplemented!("Wave 2")
    }

    /// Await completion.
    ///
    /// # Errors
    ///
    /// [`TaskFailure`] when the task panicked or was aborted at shutdown.
    pub async fn join(self) -> Result<T, TaskFailure> {
        unimplemented!("Wave 2")
    }

    /// Whether the task has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        unimplemented!("Wave 2")
    }
}

/// Why a task did not produce its value.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TaskFailure {
    /// The task panicked; the payload is bounded.
    #[error("task {name} panicked: {detail}")]
    Panicked {
        /// Task name.
        name: TaskName,
        /// Bounded panic payload.
        detail: String,
    },
    /// The task was aborted because shutdown's deadline expired.
    #[error("task {name} was aborted at shutdown")]
    Aborted {
        /// Task name.
        name: TaskName,
    },
}

// ---------------------------------------------------------------- channels

/// Bounded sender.
pub struct Sender<T> {
    /* private fields */
}

/// Bounded receiver.
pub struct Receiver<T> {
    /* private fields */
}

impl<T> Sender<T> {
    /// Enqueue without waiting. A full channel refuses; it never parks.
    ///
    /// # Errors
    ///
    /// [`SendRefused`] when the channel is full or closed.
    pub fn try_send(&self, value: T) -> Result<(), SendRefused<T>> {
        let _ = value;
        unimplemented!("Wave 2")
    }

    /// Enqueue, waiting for capacity up to `deadline`.
    ///
    /// # Errors
    ///
    /// [`SendRefused`] on deadline expiry or closure.
    pub async fn send_before(&self, value: T, deadline: Duration) -> Result<(), SendRefused<T>> {
        let _ = (value, deadline);
        unimplemented!("Wave 2")
    }

    /// Current queued item count.
    #[must_use]
    pub fn len(&self) -> usize {
        unimplemented!("Wave 2")
    }

    /// Whether the channel is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Receiver<T> {
    /// Await the next item; `None` once every sender is dropped.
    pub async fn recv(&mut self) -> Option<T> {
        unimplemented!("Wave 2")
    }

    /// Take an item without waiting.
    #[must_use]
    pub fn try_recv(&mut self) -> Option<T> {
        unimplemented!("Wave 2")
    }
}

/// A bounded channel refused an item. The item is returned, never dropped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendRefused<T> {
    /// The item that was not enqueued.
    pub value: T,
    /// Why.
    pub reason: SendRefusal,
}

/// Why a bounded channel refused.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SendRefusal {
    /// The channel is at its declared depth.
    #[error("channel is full at its declared depth {depth}")]
    Full {
        /// Declared depth.
        depth: usize,
    },
    /// Every receiver is gone.
    #[error("channel is closed")]
    Closed,
    /// The send deadline expired before capacity appeared.
    #[error("channel send deadline expired")]
    DeadlineExpired,
}

// ---------------------------------------------------------------- providers

/// Typed completion of one deadline-wrapped provider call.
///
/// Authority: ARCH:2300 "The runtime performs the work and returns typed
/// completions."
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCompletion<T> {
    /// The provider returned within its deadline.
    Completed {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: OperationGeneration,
        /// The provider's value.
        value: T,
    },
    /// The deadline expired. The provider may still be running; the runtime
    /// owns detaching it and it can no longer affect the owner.
    TimedOut {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: OperationGeneration,
        /// The deadline that expired.
        after: Duration,
    },
    /// The provider panicked and was isolated.
    Panicked {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: OperationGeneration,
        /// Bounded panic payload.
        detail: String,
    },
    /// The owner's cancellation token fired.
    Cancelled {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: OperationGeneration,
    },
    /// The runtime is shutting down and refused the call.
    Refused {
        /// Operation slot.
        operation: OperationName,
        /// Generation this completion belongs to.
        generation: OperationGeneration,
    },
}

impl<T> ProviderCompletion<T> {
    /// Generation this completion belongs to. An owner MUST compare this
    /// against its current generation and discard stale completions.
    #[must_use]
    pub fn generation(&self) -> OperationGeneration {
        match self {
            Self::Completed { generation, .. }
            | Self::TimedOut { generation, .. }
            | Self::Panicked { generation, .. }
            | Self::Cancelled { generation, .. }
            | Self::Refused { generation, .. } => *generation,
        }
    }

    /// Operation slot this completion belongs to.
    #[must_use]
    pub fn operation(&self) -> OperationName {
        match self {
            Self::Completed { operation, .. }
            | Self::TimedOut { operation, .. }
            | Self::Panicked { operation, .. }
            | Self::Cancelled { operation, .. }
            | Self::Refused { operation, .. } => *operation,
        }
    }

    /// The value, when the provider completed.
    #[must_use]
    pub fn value(self) -> Option<T> {
        match self {
            Self::Completed { value, .. } => Some(value),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- cancellation

/// A cancellation signal owned by a lifecycle owner and propagated by the runtime.
///
/// Authority: ARCH:2297.
#[derive(Clone)]
pub struct CancellationToken {
    /* private fields */
}

impl CancellationToken {
    /// A token that fires when this one fires.
    #[must_use]
    pub fn child(&self) -> Self {
        unimplemented!("Wave 2")
    }

    /// Fire this token and every descendant.
    pub fn cancel(&self) {
        unimplemented!("Wave 2")
    }

    /// Whether this token has fired.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        unimplemented!("Wave 2")
    }

    /// Resolve when this token fires.
    pub async fn cancelled(&self) {
        unimplemented!("Wave 2")
    }
}

// ---------------------------------------------------------------- error

/// Runtime refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeError {
    /// The task registry is at its declared bound.
    #[error("runtime holds {limit} tasks, its declared maximum")]
    TaskLimit {
        /// Declared maximum.
        limit: usize,
    },
    /// The provider-operation bound is reached.
    #[error("runtime holds {limit} provider operations, its declared maximum")]
    ProviderOperationLimit {
        /// Declared maximum.
        limit: usize,
    },
    /// Shutdown began; no new work is admitted.
    #[error("runtime is shutting down")]
    ShuttingDown,
    /// Shutdown's deadline expired with work still registered.
    #[error("{} tasks did not join before the shutdown deadline", .tasks.len())]
    ShutdownIncomplete {
        /// Tasks that did not join.
        tasks: Vec<TaskName>,
    },
}
```

### 5.3 Signatures forced by a specific authority line

| Element | Forced by |
|---|---|
| Join registry and `shutdown(deadline)` | `ARCH:2298`, `ARCH:2893` "runtime joins owned resources" |
| Bounded channels with refusal, not park | `ARCH:2290` + `GOALS:1437` |
| `call_provider` with an owner-supplied deadline | `ARCH:2306` |
| `ProviderCompletion` carrying `OperationGeneration` | `ARCH:2300` + `GOALS:426` |
| `Panicked` as a completion, not an unwind | `ARCH:2296` "provider panic/failure isolation" |
| `CancellationToken` with parent/child propagation | `ARCH:2297` |
| Concrete types, no `Runtime` trait | `ARCH:2300` (universal owner), `VOCAB:270` |

### 5.4 Decisions

- `DECIDED: Runtime is a concrete Clone struct, not a trait.` Reasoning: it is a
  universal owner (`ARCH:2284`), not a replaceable provider; `VOCAB:270` places
  it inside `Fava`'s `spec_crates`, and making it swappable would create a
  twelfth injection point the architecture never names.
- `DECIDED: TimedOut detaches rather than aborting the provider future.`
  Reasoning: `ARCH:2306` bounds a stalled provider's *influence*, not its
  existence; a provider mid-write to its own store must not be torn apart.
  The detached future is registered for shutdown join.
- `DECIDED: TaskName and OperationName are &'static str newtypes.` Reasoning:
  `GOALS:1428` bounds diagnostics; a static name cannot be attacker-supplied and
  needs no `BoundedText`.
- `DECIDED: try_send returns the value inside SendRefused.` Reasoning:
  `GOALS:1437` forbids silently discarding work; the caller must be able to
  report exact shortfall about the specific command it could not enqueue.
- `DECIDED: fava-runtime depends only on fava-query (for
  OperationGeneration), thiserror, and the async executor.` Reasoning: it must
  not depend on any contract crate, or every contract change rebuilds execution.

---

## 6. Frozen cross-cutting rules

1. **No adapters.** No `From` impl, type alias, or shim exists to keep an old
   call site compiling. If `crates/fava/src/relay.rs` breaks, it breaks.
2. **Bounded text everywhere.** No public struct field or enum payload in any of
   the five crates is a bare `String` holding relay-, OS-, or application-
   supplied text. Only `BoundedText` / `BoundedReason` (512 bytes) and
   `&'static str` newtypes are permitted. `TaskFailure` and
   `ProviderCompletion::Panicked` carry `String` because a panic payload is
   Fava-internal; the implementer still truncates at 512 before storing.
3. **Every completion carries a generation.** Transport handoff, transport
   inbound, provider completion, relay evidence. An owner that cannot compare a
   generation cannot reject a stale completion, and `GOALS:426` requires it.
4. **Shortfall is a value, error is a refusal.** Work the owner understood but
   could not carry is a typed shortfall inside `Ok`. `Err` is reserved for input
   the owner cannot process at all.
5. **`unimplemented!()` bodies in this document are placeholders for signature
   review only.** Wave 1 lands the types and the compiling signatures;
   `validate_plan` (§2) must land *implemented* in Wave 1 because Wave 3
   depends on it, and `fava-runtime`'s bodies land in Wave 2.
6. **Vocabulary gate.** Each of the five crates carries new public nominals.
   `docs/internals/vocabulary.toml` must be updated in the *same commit* as the
   crate change, or CI is red for a reason unrelated to the change. New symbols
   attach to existing terms (`Observation`, `Query`, `SubscriptionPlanner`,
   `Transport`, `Diagnostics`, `Fava`); **no new `[[term]]` is authorized by
   this document.**

## 7. Crate dependency edges this document creates

```text
fava-query      -> (unchanged: fava-state, nostr, thiserror)
fava-transport  -> fava-query (OperationGeneration), fava-state, thiserror
fava-subscriptions -> fava-query, fava-state, fava-transport (BoundedReason),
                      fava-wire, nostr, thiserror
fava-diagnostics-> fava-query, fava-state, fava-wire
fava-runtime    -> fava-query, thiserror, tokio
```

Every edge runs contract → contract or contract → domain. None runs
contract → owner. `fava-observe` gains `fava-routing`, `fava-subscriptions`,
`fava-transport`, `fava-ingest`, `fava-diagnostics`, `fava-runtime` in Wave 3;
that is legal (`ARCH:2984-3016`) and out of scope here.

## 8. Falsifier obligations attached to these contracts

Each is written against the signatures above and must compile after Wave 1.

| Contract | Falsifier |
|---|---|
| §1 | `one_physical_session_fans_out_every_inbound_frame_to_every_consumer` — two `messages()` streams both see one pushed frame |
| §1 | `acquiring_a_live_session_does_not_dial` — two acquires on one key, `holders() == 2`, dial count 1 |
| §1 | `stalled_relay_yields_bounded_refusal_not_an_unbounded_park` — `NotHandedOff { reason: OutboundQueueFull }` |
| §1 | `handoff_completion_names_its_own_session_generation` |
| §2 | `a_multi_filter_req_planner_is_accepted` — `PlannedSubscription.filters.len() == 2` passes `validate_plan` |
| §2 | `replanning_retains_unchanged_wire_subscriptions` — second plan has `open.len() == 1`, `retain.len() == 1` |
| §2 | `partial_plan_reports_shortfall_and_still_installs` — `Ok` with non-empty `open` *and* non-empty `shortfalls` |
| §2 | `withdrawal_only_plan_is_conformant` — empty `demand`, non-empty `installed`, `Ok` with only `close` |
| §3 | `empty_with_eose_is_distinguishable_from_unreachable_relay` |
| §3 | `grouped_eose_settles_every_logical_demand` — attribution `serves` fan-out |
| §4 | `diagnostics_attribute_each_relay_session_to_its_observation` |
| §4 | `hostile_relay_text_is_bounded_in_retained_diagnostics` |
| §5 | `stalled_provider_yields_timed_out_completion_and_shutdown_still_joins` |
| §5 | `panicking_provider_becomes_a_typed_completion` |

## 9. Amendments

None. Append here with date, author, and the authority line that forced the
change. Do not edit sections 0-8 in place.
