//! Scoped observation refusals and terminal facts.

use fava_query::{QueryEvaluationError, QuerySourceError, SourceKind};
use thiserror::Error;

/// Query-open refusal before a usable handle exists.
///
/// The value is large because it names the exact source role that refused, and
/// a live-relay role carries the relay session identity. Collapsing it would
/// erase the distinction `GOALS:302` requires.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// One named local source could not establish its initial boundary.
    #[error("{role:?} failed to open: {error}")]
    SourceOpen {
        /// Query-source role.
        role: SourceKind,
        /// Scoped provider refusal.
        error: QuerySourceError,
    },
    /// Initial local evaluation failed.
    #[error(transparent)]
    Evaluation(#[from] QueryEvaluationError),
    /// Relay demand could not be bound to one exact route plan.
    #[error("relay query refused: {0}")]
    Relay(String),
    /// The engine is shutting down and admits no new observation.
    #[error("engine is shutting down")]
    EngineClosed,
}

/// Terminal observation fact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("live query observation closed")]
pub struct ObservationClosed;
