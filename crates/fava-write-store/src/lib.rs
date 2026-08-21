//! Neutral contract for accepted local event materializations.

use fava_query::QuerySource;
use fava_routing::{CoverageState, RoutePlan};
use fava_state::RelaySessionKey;
use fava_write::{
    Event, EventId, EventValue, InvalidEventValue, LocalWriteEvent, MaterializationId, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, ReplaceableEventEdit, UnsignedEvent, WriteId,
    WriteIntent, WriteIntentError, WriteRouting,
};
use thiserror::Error;
use tokio::sync::broadcast;

const MAX_RECEIPT_TEXT_BYTES: usize = 4_096;
const DESTINATION_EVIDENCE_CAPACITY: usize = 256;

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

    /// Atomically accept one edit and its already-validated first materialization.
    ///
    /// # Errors
    ///
    /// Returns [`WriteStoreError`] when semantic custody is unsupported or the
    /// complete edit, receipt, materialization, and query-source commit refuses.
    fn accept_materialized_edit(
        &self,
        _intent: WriteIntent,
        _event: UnsignedEvent,
        _source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support replaceable-event edits".to_owned(),
        ))
    }

    /// Atomically replace the exact current semantic materialization.
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
        _event: UnsignedEvent,
        _source: Option<&Event>,
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
        _source: Option<&Event>,
        _reason: String,
    ) -> Result<Receipt, WriteStoreError> {
        Err(WriteStoreError::Refused(
            "write store does not support materialization failure evidence".to_owned(),
        ))
    }

    /// Recover live semantic custody in stable receipt order.
    ///
    /// Each tuple carries the current receipt, durable edit, current selected
    /// source id, and last failed source id. No separate recovery noun exists.
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
            ReplaceableEventEdit,
            Option<EventId>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        Ok(Vec::new())
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

/// Shared bound for current destinations and retained publication evidence.
///
/// Publication queues and write-store providers consume this function rather
/// than repeating the underlying number.
#[must_use]
pub const fn destination_evidence_capacity() -> usize {
    DESTINATION_EVIDENCE_CAPACITY
}

/// Validate exact current write, materialization, and event identity.
///
/// Provider mutations call this while holding their own atomic state boundary.
///
/// # Errors
///
/// Returns [`WriteStoreError`] when a delayed completion no longer names the
/// current non-terminal materialization.
pub fn validate_current_materialization(
    receipt: &Receipt,
    write_id: WriteId,
    materialization_id: MaterializationId,
    event_id: EventId,
) -> Result<(), WriteStoreError> {
    if receipt.is_terminal()
        || receipt.write_id != write_id
        || receipt.current.publication.materialization_id != materialization_id
        || receipt.current.id() != event_id
    {
        Err(WriteStoreError::Refused(
            "write materialization is not current".to_owned(),
        ))
    } else {
        Ok(())
    }
}

/// Apply one newer complete route plan to a mutable receipt.
///
/// Provider implementations call this inside their own atomic mutation.
///
/// # Errors
///
/// Returns [`WriteStoreError`] when the plan is stale, too large, or belongs
/// to an explicit-route receipt.
pub fn apply_route_to_receipt(
    receipt: &mut Receipt,
    plan: &RoutePlan,
) -> Result<(), WriteStoreError> {
    if !matches!(receipt.routing, WriteRouting::Automatic) {
        return Err(WriteStoreError::Refused(
            "automatic route cannot mutate an explicit receipt".to_owned(),
        ));
    }
    if plan.revision <= receipt.route_revision {
        return Err(WriteStoreError::Refused(format!(
            "route revision is not newer: {} <= {}",
            plan.revision, receipt.route_revision
        )));
    }
    if plan.destinations.len() > destination_evidence_capacity() {
        return Err(WriteStoreError::Refused(format!(
            "route destination fan-out exceeds bound: {} > {}",
            plan.destinations.len(),
            destination_evidence_capacity()
        )));
    }

    let desired: std::collections::BTreeSet<_> = plan.destinations.keys().cloned().collect();
    let mut shortfalls = plan.shortfalls.clone();
    shortfalls.extend(
        plan.coverage
            .iter()
            .filter(|(_, state)| matches!(state, CoverageState::SettledAbsent))
            .map(|(target, _)| format!("no relay destination for {target:?}")),
    );
    if shortfalls.len() > destination_evidence_capacity() {
        return Err(WriteStoreError::Refused(format!(
            "route shortfall count exceeds bound: {} > {}",
            shortfalls.len(),
            destination_evidence_capacity()
        )));
    }
    for shortfall in &shortfalls {
        validate_receipt_text(shortfall)?;
    }

    let removed: Vec<_> = receipt
        .desired_destinations
        .difference(&desired)
        .cloned()
        .collect();
    for session in removed {
        match receipt.current.publication.destinations.get(&session) {
            Some(RelayDeliveryOutcome::Pending) => {
                receipt.current.publication.destinations.remove(&session);
                receipt.attempts.remove(&session);
            }
            Some(RelayDeliveryOutcome::Retryable { .. }) => {
                receipt
                    .current
                    .publication
                    .destinations
                    .insert(session, RelayDeliveryOutcome::CancelledBeforeHandoff);
            }
            Some(
                RelayDeliveryOutcome::Attempting
                | RelayDeliveryOutcome::Acknowledged { .. }
                | RelayDeliveryOutcome::Rejected { .. }
                | RelayDeliveryOutcome::GivenUp { .. }
                | RelayDeliveryOutcome::Unknown { .. }
                | RelayDeliveryOutcome::CancelledBeforeHandoff,
            )
            | None => {}
        }
    }
    for session in &desired {
        receipt
            .current
            .publication
            .destinations
            .entry(session.clone())
            .or_insert(RelayDeliveryOutcome::Pending);
    }

    receipt.route_revision = plan.revision;
    receipt.route_settled = plan.settled;
    receipt.route_shortfalls = shortfalls;
    receipt.desired_destinations = desired;
    settle_route(receipt);
    Ok(())
}

fn settle_route(receipt: &mut Receipt) {
    if !receipt.route_settled
        || receipt
            .destinations()
            .values()
            .any(|outcome| !outcome.is_terminal())
    {
        receipt.outcome = ReceiptOutcome::Open;
    } else if receipt.desired_destinations.is_empty() {
        receipt.outcome = ReceiptOutcome::NoDestination;
    } else {
        receipt.outcome = ReceiptOutcome::Complete;
    }
}
