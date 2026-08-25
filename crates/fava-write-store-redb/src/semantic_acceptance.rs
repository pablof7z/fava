//! Exact semantic acceptance validation shared by durable admission and completion paths.

use fava_state::{EventCoordinate, event_coordinate, event_is_newer};
use fava_write::{
    EventId, EventValue, MaterializationId, PublicKey, Receipt, ReplaceableEventEdit, Timestamp,
    UnsignedEvent, WriteId, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStoreError;

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
    let event_id = event
        .id
        .ok_or_else(|| WriteStoreError::Refused("materialization has no stable id".to_owned()))?;
    if selected.is_some_and(|(source_id, source_time)| {
        !event_is_newer((event.created_at, event_id), (source_time, source_id))
    }) {
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

pub(super) fn edit_coordinate(edit: &ReplaceableEventEdit, author: PublicKey) -> EventCoordinate {
    EventCoordinate::Replaceable {
        author,
        kind: edit.kind(),
        identifier: edit.identifier().map(str::to_owned),
    }
}

pub(super) fn attributed_failure(
    materialization_id: MaterializationId,
    source: Option<EventId>,
    reason: String,
) -> String {
    let source = source.map_or_else(|| "empty state".to_owned(), |id| id.to_string());
    let prefix = format!(
        "materialization {} from source {source} failed",
        materialization_id.as_u64()
    );
    let attributed = format!("{prefix}: {reason}");
    drop(reason);
    if fava_write_store::validate_receipt_text(&attributed).is_ok() {
        attributed
    } else {
        prefix
    }
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

pub(super) fn require_qualified_source(
    current: Option<(EventId, Timestamp)>,
    candidate: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    let qualified = match (current, candidate) {
        (None, Some(_)) | (Some(_), None) => true,
        (Some((current_id, current_time)), Some((candidate_id, candidate_time))) => {
            event_is_newer((candidate_time, candidate_id), (current_time, current_id))
        }
        (None, None) => false,
    };
    if qualified {
        Ok(())
    } else {
        Err(WriteStoreError::Refused(
            "source event is equal, older, or already consumed".to_owned(),
        ))
    }
}

pub(super) fn require_failure_source(
    current: Option<(EventId, Timestamp)>,
    failed: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    if current == failed {
        Ok(())
    } else {
        require_qualified_source(current, failed)
    }
}
