use fava_routing::{CoverageState, RoutePlan};
use fava_write::{
    EventId, MaterializationId, Receipt, ReceiptOutcome, RelayDeliveryOutcome, WriteId,
    WriteRouting,
};

use crate::WriteStoreError;

const MAX_RECEIPT_TEXT_BYTES: usize = 4_096;
/// Shared cap on live relay destinations and on retained superseded
/// materializations.
const DESTINATION_EVIDENCE_CAPACITY: usize = 256;

/// Refuse provider text that exceeds durable receipt bounds.
///
/// # Errors
///
/// Returns [`WriteStoreError`] with actual and maximum byte counts.
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

/// Refuse text-bearing delivery outcomes that exceed receipt bounds.
///
/// # Errors
///
/// Returns [`WriteStoreError`] when outcome text exceeds the receipt bound.
pub fn validate_delivery_outcome(outcome: &RelayDeliveryOutcome) -> Result<(), WriteStoreError> {
    match outcome {
        RelayDeliveryOutcome::Retryable { reason }
        | RelayDeliveryOutcome::AuthenticationDenied { reason }
        | RelayDeliveryOutcome::GivenUp { reason }
        | RelayDeliveryOutcome::Unknown { reason } => validate_receipt_text(reason),
        RelayDeliveryOutcome::Acknowledged { message }
        | RelayDeliveryOutcome::Rejected { message } => validate_receipt_text(message),
        RelayDeliveryOutcome::Pending
        | RelayDeliveryOutcome::Attempting
        | RelayDeliveryOutcome::CancelledBeforeHandoff => Ok(()),
    }
}

/// Shared cap on live relay destinations and on retained superseded
/// materializations.
#[must_use]
pub const fn destination_evidence_capacity() -> usize {
    DESTINATION_EVIDENCE_CAPACITY
}

/// Validate exact current write, materialization, and event identity.
///
/// # Errors
///
/// Returns [`WriteStoreError`] when the named materialization is not current.
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

/// Apply one newer complete route plan or accept its exact persisted replay.
///
/// # Errors
///
/// Returns [`WriteStoreError`] for stale, same-revision mismatched, oversized,
/// or incompatible plans.
pub fn apply_route_to_receipt(
    receipt: &mut Receipt,
    plan: &RoutePlan,
) -> Result<(), WriteStoreError> {
    if !matches!(receipt.routing, WriteRouting::Automatic) {
        return Err(WriteStoreError::Refused(
            "automatic route cannot mutate an explicit receipt".to_owned(),
        ));
    }
    if plan.revision < receipt.route_revision {
        return Err(WriteStoreError::Refused(format!(
            "route revision is stale: {} < {}",
            plan.revision, receipt.route_revision
        )));
    }
    if plan.revision == receipt.route_revision {
        if plan.revision == 0 {
            return Err(WriteStoreError::Refused(
                "route revision zero has no persisted effect to replay".to_owned(),
            ));
        }
        return if persisted_route_effect_matches(receipt, plan) {
            Ok(())
        } else {
            Err(WriteStoreError::Refused(format!(
                "route revision {} has a different persisted effect",
                plan.revision
            )))
        };
    }
    apply_newer_route_to_receipt(receipt, plan)
}

fn persisted_route_effect_matches(receipt: &Receipt, plan: &RoutePlan) -> bool {
    let mut candidate = receipt.clone();
    candidate.outcome = ReceiptOutcome::Open;
    candidate.route_revision = 0;
    candidate.route_settled = false;
    candidate.route_shortfalls.clear();
    candidate.desired_destinations.clear();
    candidate.attempts.clear();
    candidate.current.publication.destinations.clear();
    apply_newer_route_to_receipt(&mut candidate, plan).is_ok()
        && candidate.outcome == receipt.outcome
        && candidate.route_revision == receipt.route_revision
        && candidate.route_settled == receipt.route_settled
        && candidate.route_shortfalls == receipt.route_shortfalls
        && candidate.desired_destinations == receipt.desired_destinations
        && candidate.attempts == receipt.attempts
        && candidate.current.publication.destinations == receipt.current.publication.destinations
}

fn apply_newer_route_to_receipt(
    receipt: &mut Receipt,
    plan: &RoutePlan,
) -> Result<(), WriteStoreError> {
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
                | RelayDeliveryOutcome::AuthenticationDenied { .. }
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
    receipt.route_settled = plan.settled();
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
