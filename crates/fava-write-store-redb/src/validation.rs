use std::collections::{BTreeMap, BTreeSet};

use fava_state::{RelayAccess, RelaySessionKey};
use fava_write::{
    EventValue, MaterializationId, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome,
    SignatureState, WriteIntent, WriteRouting,
};
use fava_write_store::{
    WriteStoreError, destination_evidence_capacity, validate_delivery_outcome,
    validate_receipt_text,
};

use crate::SemanticCustody;

pub(super) fn reconstructed(
    next_identity: u64,
    receipts: &BTreeMap<ReceiptId, Receipt>,
    semantics: &BTreeMap<ReceiptId, SemanticCustody>,
) -> Result<(), WriteStoreError> {
    if next_identity == 0 {
        return incoherent("durable next identity is zero");
    }
    for (receipt_id, receipt) in receipts {
        validate_receipt(*receipt_id, receipt, semantics.get(receipt_id))?;
        if receipt_id.as_u64() >= next_identity {
            return incoherent("durable next identity does not exceed every receipt row");
        }
    }
    if semantics.keys().any(|id| !receipts.contains_key(id)) {
        return incoherent("durable semantic custody has no receipt");
    }
    Ok(())
}

fn validate_receipt(
    row_id: ReceiptId,
    receipt: &Receipt,
    semantic: Option<&SemanticCustody>,
) -> Result<(), WriteStoreError> {
    let publication = &receipt.current.publication;
    if receipt.receipt_id != row_id
        || receipt.write_id.as_u64() != row_id.as_u64()
        || publication.receipt_id != receipt.receipt_id
        || publication.write_id != receipt.write_id
    {
        return incoherent("durable receipt and publication identities disagree");
    }
    if receipt.current.event.id() != Some(receipt.current.id()) {
        return incoherent("durable current event identity disagrees with its body");
    }
    validate_event_and_signature(receipt)?;
    validate_routing(receipt)?;
    validate_bounds_and_text(receipt)?;
    validate_outcome(receipt)?;
    validate_materializations(receipt, semantic)
}

fn validate_routing(receipt: &Receipt) -> Result<(), WriteStoreError> {
    let WriteRouting::Explicit(relays) = &receipt.routing else {
        return Ok(());
    };
    if relays.is_empty() {
        return incoherent("durable explicit route is empty");
    }
    let mut seen = BTreeSet::new();
    if relays.iter().any(|relay| !seen.insert(relay)) {
        return incoherent("durable explicit route repeats a relay identity");
    }
    let expected: BTreeSet<_> = relays
        .iter()
        .cloned()
        .map(|relay| RelaySessionKey::new(relay, RelayAccess::public()))
        .collect();
    let retained: BTreeSet<_> = receipt
        .current
        .publication
        .destinations
        .keys()
        .cloned()
        .collect();
    if retained != expected || receipt.desired_destinations != expected {
        return incoherent("durable explicit route disagrees with its destination lanes");
    }
    Ok(())
}

fn validate_event_and_signature(receipt: &Receipt) -> Result<(), WriteStoreError> {
    match (
        &receipt.current.event,
        &receipt.current.publication.signature,
    ) {
        (EventValue::Unsigned(event), SignatureState::Unsigned) => event
            .verify_id()
            .map_err(|error| WriteStoreError::Refused(error.to_string())),
        (EventValue::Unsigned(event), SignatureState::Refused(reason)) => {
            event
                .verify_id()
                .map_err(|error| WriteStoreError::Refused(error.to_string()))?;
            validate_receipt_text(reason)
        }
        (EventValue::Signed(event), SignatureState::Signed) => event
            .verify()
            .map_err(|error| WriteStoreError::Refused(error.to_string())),
        (EventValue::Unsigned(_), SignatureState::Signed)
        | (EventValue::Signed(_), SignatureState::Unsigned | SignatureState::Refused(_)) => {
            incoherent("durable signature state disagrees with current event")
        }
    }
}

fn validate_bounds_and_text(receipt: &Receipt) -> Result<(), WriteStoreError> {
    let publication = &receipt.current.publication;
    let capacity = destination_evidence_capacity();
    if publication.destinations.len() > capacity
        || publication.retired_materializations.len() > capacity
        || receipt.desired_destinations.len() > capacity
        || receipt.attempts.len() > capacity
        || receipt.route_shortfalls.len() > capacity
        || matches!(&receipt.routing, WriteRouting::Explicit(relays) if relays.len() > capacity)
    {
        return incoherent("durable destination or evidence count exceeds bound");
    }
    if !receipt
        .desired_destinations
        .iter()
        .all(|session| publication.destinations.contains_key(session))
        || receipt.attempts.iter().any(|(session, attempt)| {
            *attempt == 0 || !publication.destinations.contains_key(session)
        })
    {
        return incoherent("durable destination ownership is incoherent");
    }
    for outcome in publication.destinations.values() {
        validate_delivery_outcome(outcome)?;
    }
    for shortfall in &receipt.route_shortfalls {
        validate_receipt_text(shortfall)?;
    }
    if let Some(failure) = &publication.materialization_failure {
        validate_receipt_text(failure)?;
    }
    Ok(())
}

fn validate_outcome(receipt: &Receipt) -> Result<(), WriteStoreError> {
    let all_terminal = receipt
        .destinations()
        .values()
        .all(RelayDeliveryOutcome::is_terminal);
    let settled = receipt.route_settled && all_terminal;
    let coherent = match receipt.outcome {
        ReceiptOutcome::Open => !settled,
        ReceiptOutcome::Cancelled => receipt
            .destinations()
            .values()
            .all(|outcome| matches!(outcome, RelayDeliveryOutcome::CancelledBeforeHandoff)),
        ReceiptOutcome::Complete => settled && !receipt.desired_destinations.is_empty(),
        ReceiptOutcome::NoDestination => settled && receipt.desired_destinations.is_empty(),
    };
    if coherent {
        Ok(())
    } else {
        incoherent("durable receipt outcome disagrees with route evidence")
    }
}

fn validate_materializations(
    receipt: &Receipt,
    semantic: Option<&SemanticCustody>,
) -> Result<(), WriteStoreError> {
    let publication = &receipt.current.publication;
    let current_id = publication.materialization_id.as_u64();
    if current_id == 0
        || usize::try_from(current_id - 1).ok() != Some(publication.retired_materializations.len())
    {
        return incoherent("durable materialization generations are not contiguous");
    }
    let mut event_ids = BTreeSet::new();
    event_ids.insert(receipt.current.id());
    for (index, (id, event_id, _, failure)) in
        publication.retired_materializations.iter().enumerate()
    {
        let expected = u64::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .map(MaterializationId::from_u64);
        if Some(*id) != expected || !event_ids.insert(*event_id) {
            return incoherent("durable retired materialization identity is incoherent");
        }
        if let Some(failure) = failure {
            validate_receipt_text(failure)?;
            let prefix = format!("materialization {} from source ", id.as_u64());
            if !failure.starts_with(&prefix) {
                return incoherent("durable retired failure attribution is incoherent");
            }
        }
    }
    match semantic {
        Some(semantic) => validate_semantic(receipt, semantic),
        None if publication.materialization_source.is_some()
            || publication.materialization_failure.is_some()
            || current_id != 1 =>
        {
            incoherent("durable publication evidence has no semantic custody")
        }
        None => Ok(()),
    }
}

fn validate_semantic(
    receipt: &Receipt,
    (edits, author, current_source, failed_source): &SemanticCustody,
) -> Result<(), WriteStoreError> {
    if edits.is_empty()
        || edits.len() > receipt.current.publication.retired_materializations.len() + 1
    {
        return incoherent("durable semantic edit sequence and generations disagree");
    }
    for edit in edits {
        WriteIntent::edit_as(edit.clone(), *author, receipt.routing.clone())?;
    }
    let edit = edits.last().expect("non-empty edit sequence validated");
    if receipt.current.event.author() != *author
        || receipt
            .current
            .event
            .coordinate()
            .map_err(|error| WriteStoreError::Refused(error.to_string()))?
            != crate::semantic::edit_coordinate(edit, *author)
        || receipt.current.publication.materialization_source != current_source.map(|(id, _)| id)
        || current_source.is_some_and(|(_, time)| time >= receipt.current.event.created_at())
    {
        return incoherent("durable semantic custody is incoherent");
    }
    let failure = receipt
        .current
        .publication
        .materialization_failure
        .as_deref();
    if failure.is_none() && failed_source.is_some() {
        return incoherent("durable failed source and failure evidence disagree");
    }
    if let Some(reason) = failure {
        let source = failed_source.map_or_else(|| "empty state".to_owned(), |id| id.to_string());
        let prefix = format!(
            "materialization {} from source {source} failed",
            receipt.current.publication.materialization_id.as_u64()
        );
        if reason != prefix && !reason.starts_with(&format!("{prefix}: ")) {
            return incoherent("durable failed-source attribution is incoherent");
        }
    }
    Ok(())
}

fn incoherent<T>(reason: &str) -> Result<T, WriteStoreError> {
    Err(WriteStoreError::Refused(reason.to_owned()))
}
