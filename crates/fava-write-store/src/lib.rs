//! Neutral contract for accepted local event materializations.

use fava_query::QuerySource;
use fava_state::RelaySessionKey;
use fava_write::{
    EventValue, InvalidEventValue, LocalWriteEvent, Receipt, ReceiptId, RelayDeliveryOutcome,
    WriteId, WriteIntent, WriteIntentError, WriteRouting,
};
use thiserror::Error;
use tokio::sync::broadcast;

const MAX_RECEIPT_TEXT_BYTES: usize = 4_096;

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
    /// Subscribe to committed receipt changes after this call.
    ///
    /// `Some(receipt)` is one committed current receipt; `None` is removal for
    /// the paired id. The bounded receiver reports lag explicitly.
    fn receipt_changes(&self) -> broadcast::Receiver<(ReceiptId, Option<Receipt>)>;

    /// Atomically accept one publication obligation and its current event.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the event is invalid or the complete
    /// acceptance mutation cannot commit.
    fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, WriteStoreError>;

    /// Atomically accept one current event using automatic routing.
    ///
    /// This is useful for deterministic local-source profiles. Publication-capable
    /// applications ordinarily submit a checked [`WriteIntent`] directly.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when validation or the complete commit fails.
    fn accept_materialized(&self, event: EventValue) -> Result<AcceptedWrite, WriteStoreError> {
        let intent = match event {
            EventValue::Unsigned(event) => WriteIntent::event(event, WriteRouting::Automatic),
            EventValue::Signed(event) => WriteIntent::presigned(event, WriteRouting::Automatic),
        }?;
        self.accept(intent)
    }

    /// Install a verified signature for the exact current unsigned event.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale, invalid, terminal, or failed mutation.
    fn install_signed(
        &self,
        receipt_id: ReceiptId,
        event: fava_write::Event,
    ) -> Result<Receipt, WriteStoreError>;

    /// Commit an exact signing refusal for the current unsigned event.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale, terminal, or failed mutation.
    fn record_signer_refusal(
        &self,
        receipt_id: ReceiptId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError>;

    /// Durably authorize one exact attempt before any transport effect.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] unless the receipt and destination are current.
    fn begin_attempt(
        &self,
        receipt_id: ReceiptId,
        session: &RelaySessionKey,
    ) -> Result<Receipt, WriteStoreError>;

    /// Commit one exact destination result after an authorized attempt.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] unless the attempt remains current.
    fn record_outcome(
        &self,
        receipt_id: ReceiptId,
        session: &RelaySessionKey,
        outcome: RelayDeliveryOutcome,
    ) -> Result<Receipt, WriteStoreError>;

    /// Cancel one accepted local contribution before publication work exists.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the cancellation cannot commit.
    fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError>;

    /// Read one current local contribution by receipt.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the provider cannot read current state.
    fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, WriteStoreError>;

    /// Recover every currently open obligation in stable identity order.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when recovery cannot read coherent state.
    fn recover_open(&self) -> Result<Vec<Receipt>, WriteStoreError>;

    /// Remove one retained terminal receipt independently of cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for active work or failed mutation.
    fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, WriteStoreError>;

    /// Read one current query-visible local contribution by receipt.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the provider cannot read current state.
    fn receipt_event(
        &self,
        receipt_id: ReceiptId,
    ) -> Result<Option<LocalWriteEvent>, WriteStoreError> {
        Ok(self
            .receipt(receipt_id)?
            .filter(|receipt| !matches!(receipt.outcome, fava_write::ReceiptOutcome::Cancelled))
            .map(|receipt| receipt.current))
    }

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
    /// Intent was invalid before durable mutation.
    #[error(transparent)]
    InvalidIntent(#[from] WriteIntentError),
    /// Provider refused an operation before mutation.
    #[error("write store refused operation: {0}")]
    Refused(String),
}

/// Refuse provider text that would exceed durable receipt bounds.
///
/// # Errors
///
/// Returns [`WriteStoreError`] with the actual and maximum byte counts.
pub fn validate_receipt_text(value: &str) -> Result<(), WriteStoreError> {
    if value.len() <= MAX_RECEIPT_TEXT_BYTES {
        Ok(())
    } else {
        Err(WriteStoreError::Refused(format!(
            "receipt text exceeds bound: {} > {MAX_RECEIPT_TEXT_BYTES}",
            value.len()
        )))
    }
}

/// Refuse any text-bearing delivery outcome that exceeds receipt bounds.
///
/// # Errors
///
/// Returns [`WriteStoreError`] with the actual and maximum byte counts.
pub fn validate_delivery_outcome(outcome: &RelayDeliveryOutcome) -> Result<(), WriteStoreError> {
    match outcome {
        RelayDeliveryOutcome::Retryable { reason }
        | RelayDeliveryOutcome::GivenUp { reason }
        | RelayDeliveryOutcome::Unknown { reason } => validate_receipt_text(reason),
        RelayDeliveryOutcome::Acknowledged { message }
        | RelayDeliveryOutcome::Rejected { message } => validate_receipt_text(message),
        RelayDeliveryOutcome::Pending
        | RelayDeliveryOutcome::Attempting
        | RelayDeliveryOutcome::CancelledBeforeHandoff => Ok(()),
    }
}
