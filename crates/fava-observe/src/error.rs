//! Scoped observation refusals and terminal facts.

use fava_query::{
    OperationGenerationExhausted, QueryEvaluationError, QuerySourceError, SourceKind,
};
use fava_subscriptions::PlanRevisionExhausted;
use thiserror::Error;

/// Query-open refusal before a usable handle exists.
#[derive(Debug, Error)]
pub enum ObserveError {
    /// One named local source could not establish its initial boundary.
    ///
    /// `role` names the exact source that refused, and a live-relay role
    /// carries the relay session identity; both fields are boxed because a
    /// live-relay role and refusal are large, but collapsing the distinction
    /// itself would erase what `GOALS:302` requires.
    #[error("{role:?} failed to open: {error}")]
    SourceOpen {
        /// Query-source role.
        role: Box<SourceKind>,
        /// Scoped provider refusal.
        error: Box<QuerySourceError>,
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
    /// The owner cannot mint another provider-operation generation.
    #[error(transparent)]
    OperationGenerationExhausted(#[from] OperationGenerationExhausted),
    /// The owner cannot mint another desired-plan revision.
    #[error(transparent)]
    PlanRevisionExhausted(#[from] PlanRevisionExhausted),
}

/// Terminal observation fact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("live query observation closed")]
pub struct ObservationClosed;
