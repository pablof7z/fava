//! Neutral contract for accepted local event materializations.

use fava_query::QuerySource;
use fava_relay::RelaySessionKey;
use fava_routing::RoutePlan;
use fava_write::{
    EventId, EventValue, InvalidEventValue, LocalWriteEvent, MaterializationId, PublicKey, Receipt,
    ReceiptId, RelayDeliveryOutcome, ReplaceableEventEdit, Timestamp, UnsignedEvent, WriteId,
    WriteIntent, WriteIntentError, WriteRouting,
};
use thiserror::Error;
use tokio::sync::broadcast;

mod receipt;

pub use receipt::{
    apply_route_to_receipt, destination_evidence_capacity, validate_current_materialization,
    validate_delivery_outcome, validate_receipt_text,
};

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
    /// Exact active semantic-write admission capacity of this provider.
    ///
    /// Providers that do not yet support semantic custody report zero.
    fn active_capacity(&self) -> usize {
        0
    }

    /// Reserve one active semantic-write slot before invoking external providers.
    ///
    /// The primitive identity is store-local, bound to the edit's exact
    /// author/kind/identifier coordinate, and has no meaning after it is
    /// released or consumed by matching
    /// [`WriteStore::accept_reserved_materialized_edit`].
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] without a reservation when the coordinate is
    /// already reserved or when active custody plus reserved inactive
    /// coordinates has reached [`WriteStore::active_capacity`]. An
    /// already-active coordinate may hold one reservation against its operation
    /// and must still pass composition bounds.
    fn reserve_active(
        &self,
        _edit: &ReplaceableEventEdit,
        _author: PublicKey,
    ) -> Result<u64, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support active reservations".to_owned(),
        ))
    }

    /// Release one unused store-local active reservation.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the primitive identity is not current.
    fn release_active(&self, _reservation: u64) -> Result<(), WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support active reservations".to_owned(),
        ))
    }

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

    /// Atomically accept one edit and its already-validated materialization.
    /// A distinct same-coordinate edit may append to the exact active unsigned
    /// operation and returns that operation's stable write and receipt identity.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when semantic custody is unsupported or the
    /// complete edit, receipt, materialization, and query-source commit refuses.
    fn accept_materialized_edit(
        &self,
        _intent: WriteIntent,
        _event: UnsignedEvent,
        _source: Option<&EventValue>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support replaceable-event edits".to_owned(),
        ))
    }

    /// Atomically consume one active reservation while accepting an edit.
    ///
    /// Success and post-coordinate-validation refusal consume the reservation,
    /// so a provider failure cannot leak pre-custody capacity. A different
    /// coordinate refuses without consuming another coordinate's reservation.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the reservation is stale, belongs to a
    /// different coordinate, or the complete acceptance mutation refuses.
    fn accept_reserved_materialized_edit(
        &self,
        _reservation: u64,
        _intent: WriteIntent,
        _event: UnsignedEvent,
        _source: Option<&EventValue>,
        _initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support reserved edit acceptance".to_owned(),
        ))
    }

    /// Atomically replace the exact current semantic materialization after
    /// proving the caller applied the complete durable edit sequence.
    ///
    /// Repeating an already-committed exact update is idempotent. Every other
    /// stale generation, write, source, body, or terminal update refuses.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] without mutation when any currentness or
    /// boundedness check fails.
    #[allow(clippy::too_many_arguments)]
    fn install_materialization(
        &self,
        _write_id: WriteId,
        _receipt_id: ReceiptId,
        _expected: MaterializationId,
        _expected_source: Option<EventId>,
        _applied_edits: &[ReplaceableEventEdit],
        _event: UnsignedEvent,
        _source: Option<&EventValue>,
        _initial_route: Option<&RoutePlan>,
    ) -> Result<Receipt, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support materialization replacement".to_owned(),
        ))
    }

    /// Record one bounded post-accept materialization failure against exact
    /// current write, generation, and selected source identity.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] without mutation for stale, terminal,
    /// unqualified, unsupported, or otherwise invalid completion facts.
    #[allow(clippy::too_many_arguments)]
    fn record_materialization_failure(
        &self,
        _write_id: WriteId,
        _receipt_id: ReceiptId,
        _expected: MaterializationId,
        _expected_source: Option<EventId>,
        _source: Option<&EventValue>,
        _reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support materialization failure evidence".to_owned(),
        ))
    }

    /// Recover live semantic custody in stable receipt order.
    ///
    /// Each tuple carries the current receipt, durable ordered edit sequence, accepted author,
    /// current selected source id/timestamp, and last failed source id. No
    /// separate recovery noun exists.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when coherent current custody cannot be read.
    #[allow(clippy::type_complexity)] // Existing values deliberately avoid a recovery wrapper.
    fn recover_materialized_edits(
        &self,
    ) -> Result<
        Vec<(
            Receipt,
            Vec<ReplaceableEventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        Ok(Vec::new())
    }

    /// Read one exact live semantic custody record by receipt identity.
    ///
    /// The ordered edit sequence is bounded by retained materialization
    /// evidence and is empty only for an incoherent provider implementation.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when current custody cannot be read.
    #[allow(clippy::type_complexity)]
    fn materialized_edits(
        &self,
        _receipt_id: ReceiptId,
        _expected: MaterializationId,
    ) -> Result<
        Option<(
            Vec<ReplaceableEventEdit>,
            PublicKey,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        Ok(None)
    }

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
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        event: fava_write::Event,
    ) -> Result<Receipt, WriteStoreError>;

    /// Durably authorize signer invocation for one exact current unsigned generation.
    ///
    /// A semantic coordinate reservation that committed first leaves typed
    /// retryable evidence instead. Once authorization commits, later semantic
    /// admission is retained as at most one bounded successor and cannot
    /// supersede the authorized generation before invocation.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale, signed, terminal, or failed mutation.
    fn authorize_signing(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
    ) -> Result<Receipt, WriteStoreError>;

    /// Commit an exact retryable pre-effect signing failure.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale, signed, terminal, or failed mutation.
    fn record_signer_retryable(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError>;

    /// Whether one exact authorized generation has a bounded durable successor.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when the supplied identity is stale or unreadable.
    fn signing_successor(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
    ) -> Result<bool, WriteStoreError>;

    /// Commit an exact signing refusal for the current unsigned event.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale, terminal, or failed mutation.
    fn record_signer_refusal(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        reason: String,
    ) -> Result<Receipt, WriteStoreError>;

    /// Atomically apply one complete current automatic route plan.
    ///
    /// New destinations become pending lanes. Withdrawn destinations retire
    /// only when no handoff may have occurred; exact historical facts remain.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] for stale revisions, explicit receipts, or
    /// a failed atomic mutation.
    fn apply_route(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        plan: &RoutePlan,
    ) -> Result<Receipt, WriteStoreError>;

    /// Durably authorize one exact attempt before any transport effect.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] unless the receipt and destination are current.
    fn begin_attempt(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
    ) -> Result<Receipt, WriteStoreError>;

    /// Commit one exact destination result after an authorized attempt.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] unless the attempt remains current.
    #[allow(clippy::too_many_arguments)]
    fn record_outcome(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        attempt: u32,
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
