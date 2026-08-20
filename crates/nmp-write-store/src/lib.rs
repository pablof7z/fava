//! Neutral contract for accepted local event materializations.

use nmp_query::QuerySource;
use nmp_write::{EventValue, InvalidEventValue, LocalWriteEvent, ReceiptId, WriteId};
use thiserror::Error;

/// Result returned only after a local event contribution is committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedWrite {
    /// Stable write identity.
    pub write_id: WriteId,
    /// Stable reattachable receipt identity.
    pub receipt_id: ReceiptId,
    /// Committed current local event.
    pub current: LocalWriteEvent,
}

/// Write-store provider contract used by the first local-source slice.
pub trait WriteStore: QuerySource + Send + Sync {
    /// Atomically accept one already-materialized local event.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the event is invalid or the complete
    /// acceptance mutation cannot commit.
    fn accept_materialized(&self, event: EventValue) -> Result<AcceptedWrite, WriteStoreError>;

    /// Cancel one accepted local contribution before publication work exists.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the cancellation cannot commit.
    fn cancel(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError>;

    /// Read one current local contribution by receipt.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the provider cannot read current state.
    fn receipt_event(
        &self,
        receipt_id: ReceiptId,
    ) -> Result<Option<LocalWriteEvent>, WriteStoreError>;

    /// Number of current local contributions.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the provider cannot read current state.
    fn len(&self) -> Result<usize, WriteStoreError>;

    /// Whether the store currently exposes no local events.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the provider cannot read current state.
    fn is_empty(&self) -> Result<bool, WriteStoreError> {
        self.len().map(|len| len == 0)
    }
}

/// Scoped write-store operation failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WriteStoreError {
    /// Provider has closed.
    #[error("write store is closed")]
    Closed,
    /// Event body cannot become accepted local state.
    #[error(transparent)]
    InvalidEvent(#[from] InvalidEventValue),
    /// Provider refused an operation before mutation.
    #[error("write store refused operation: {0}")]
    Refused(String),
}
