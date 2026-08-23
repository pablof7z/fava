//! Replaceable relay-session transport contracts.
//!
//! The implementer of [`Transport`] owns every byte, every socket, every clock
//! the socket is measured against, and every generation number a session ever
//! wears. It owns a registry keyed by `RelaySessionKey`, so [`Transport::acquire_session`]
//! is a lookup-then-maybe-dial rather than a dial (`ARCH:1593`, `GOALS:936`).
//! It owns the refcount on each entry and the deterministic close that fires
//! when the count reaches zero (`ARCH:1628`). It owns reconnect policy,
//! backoff, jitter, and attempt exhaustion (`ARCH:1588-1589`, `ARCH:1625`),
//! and the fact that a reconnect mints a new generation *inside* the session
//! object the lease holders already hold (`GOALS:1093-1095`, RELAY-006).
//!
//! It owns nothing about query meaning, filters, attribution, route policy, or
//! durable retry (`GOALS:1089`, RELAY-005), and it never decides a deadline
//! value: it enforces the four durations the caller hands it in
//! [`OpenRelaySession`].

mod bounded;
mod error;
mod handoff;
mod inbound;
mod lease;
mod request;
mod session;

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::time::Duration;

pub use bounded::BoundedReason;
pub use error::TransportError;
pub use fava_query::OperationGeneration;
use fava_state::RelaySessionKey;
pub use handoff::{HandoffOutcome, ReleaseOutcome, TransportAmbiguity, TransportFailure};
pub use inbound::RelayInbound;
pub use lease::{LeaseRelease, RelaySessionLease};
pub use request::{OpenRelaySession, TransportBounds, TransportDeadlines};
pub use session::{HandoffCorrelation, RelayMessageStream, RelaySession, RelaySessionIdentity};

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
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_>;

    /// Current holder count for one key, or `None` when no session is
    /// registered. This is the observable proof that acquire-or-reuse happened.
    fn holders(&self, key: &RelaySessionKey) -> Option<NonZeroUsize>;

    /// Stop accepting acquisitions, close every registered session within
    /// `deadline`, and join owned resources.
    ///
    /// # Errors
    ///
    /// [`TransportError::ShutdownIncomplete`] when sessions remained after
    /// `deadline`; the transport is unusable either way.
    fn shutdown(&self, deadline: Duration) -> TransportShutdownFuture<'_>;
}
