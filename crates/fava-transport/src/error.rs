//! Scoped transport operation failures.

use thiserror::Error;

use crate::{RelaySessionIdentity, TransportFailure};

/// Scoped transport operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TransportError {
    /// A session could not be established before any handoff.
    #[error("relay session open refused: {0:?}")]
    ConnectionRefused(TransportFailure),
    /// A previously open session disconnected.
    #[error("relay session disconnected: {0:?}")]
    Disconnected(TransportFailure),
    /// The connection is already closed.
    #[error("relay connection {} is closed", .0.connection.get())]
    Closed(RelaySessionIdentity),
    /// The transport cannot mint another unique connection.
    #[error("relay connection space is exhausted")]
    GenerationExhausted,
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
