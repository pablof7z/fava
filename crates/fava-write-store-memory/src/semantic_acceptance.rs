//! Exact semantic acceptance validation for volatile custody.

use fava_routing::RoutePlan;
use fava_state::{EventCoordinate, event_coordinate};
use fava_write::{
    EventId, EventValue, MaterializationId, PublicKey, Receipt, ReceiptOutcome,
    ReplaceableEventEdit, Timestamp, UnsignedEvent, WriteId, WriteIntent, WriteRouting,
};
use fava_write_store::{WriteStoreError, apply_route_to_receipt};

use super::state::edit_coordinate;

pub(super) fn route_matches(receipt: &Receipt, plan: &RoutePlan) -> bool {
    let mut candidate = receipt.clone();
    candidate.outcome = ReceiptOutcome::Open;
    candidate.route_revision = 0;
    candidate.route_settled = false;
    candidate.route_shortfalls.clear();
    candidate.desired_destinations.clear();
    candidate.attempts.clear();
    candidate.current.publication.destinations.clear();
    apply_route_to_receipt(&mut candidate, plan).is_ok()
        && candidate.outcome == receipt.outcome
        && candidate.route_revision == receipt.route_revision
        && candidate.route_settled == receipt.route_settled
        && candidate.route_shortfalls == receipt.route_shortfalls
        && candidate.desired_destinations == receipt.desired_destinations
        && candidate.attempts == receipt.attempts
        && candidate.current.publication.destinations == receipt.current.publication.destinations
}

pub(super) fn validate_materialization(
    edit: &ReplaceableEventEdit,
    author: PublicKey,
    event: &UnsignedEvent,
    source: Option<&EventValue>,
    routing: &WriteRouting,
) -> Result<Option<(EventId, Timestamp)>, WriteStoreError> {
    WriteIntent::event(event.clone(), routing.clone())?;
    if event.pubkey != author
        || event_coordinate_of_unsigned(event)? != edit_coordinate(edit, author)
    {
        return Err(WriteStoreError::Refused(
            "materialization actor or coordinate does not match edit".to_owned(),
        ));
    }
    let selected = validate_source(edit, author, source)?;
    let Some((_, source_time)) = selected else {
        return Ok(None);
    };
    if source_time >= event.created_at {
        return Err(WriteStoreError::Refused(
            "materialization is not newer than its selected source".to_owned(),
        ));
    }
    Ok(selected)
}

pub(super) fn validate_source(
    edit: &ReplaceableEventEdit,
    author: PublicKey,
    source: Option<&EventValue>,
) -> Result<Option<(EventId, Timestamp)>, WriteStoreError> {
    let Some(source) = source else {
        return Ok(None);
    };
    match source {
        EventValue::Signed(event) => event
            .verify()
            .map_err(|error| WriteStoreError::Refused(error.to_string()))?,
        EventValue::Unsigned(event) => event
            .verify_id()
            .map_err(|error| WriteStoreError::Refused(error.to_string()))?,
    }
    if event_coordinate(
        source.id().ok_or_else(|| {
            WriteStoreError::Refused("materialization source has no event id".to_owned())
        })?,
        source.author(),
        source.kind(),
        source.tags(),
    ) != edit_coordinate(edit, author)
    {
        return Err(WriteStoreError::Refused(
            "materialization source does not match edit coordinate".to_owned(),
        ));
    }
    Ok(Some((
        source.id().ok_or_else(|| {
            WriteStoreError::Refused("materialization source has no event id".to_owned())
        })?,
        source.created_at(),
    )))
}

fn event_coordinate_of_unsigned(event: &UnsignedEvent) -> Result<EventCoordinate, WriteStoreError> {
    let id = event
        .id
        .ok_or_else(|| WriteStoreError::Refused("materialization has no event id".to_owned()))?;
    Ok(event_coordinate(
        id,
        event.pubkey,
        event.kind,
        event.tags.as_slice(),
    ))
}

pub(super) fn require_current(
    receipt: &Receipt,
    write_id: WriteId,
    expected: MaterializationId,
    expected_source: Option<EventId>,
    current_source: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    if receipt.is_terminal()
        || receipt.write_id != write_id
        || receipt.current.publication.materialization_id != expected
        || current_source.map(|(id, _)| id) != expected_source
    {
        return Err(WriteStoreError::Refused(
            "semantic materialization is not current".to_owned(),
        ));
    }
    Ok(())
}
