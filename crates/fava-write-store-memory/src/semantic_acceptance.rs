//! Exact semantic acceptance validation for volatile custody.

use fava_state::{EventCoordinate, event_coordinate, event_is_newer};
use fava_write::{
    EventEdit, EventId, EventValue, PublicKey, Receipt, RevisionId, Timestamp, UnsignedEvent,
    WriteId, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStoreError;

use super::state::edit_coordinate;

pub(super) fn validate_revision(
    edit: &EventEdit,
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
            "revision actor or coordinate does not match edit".to_owned(),
        ));
    }
    let selected = validate_source(edit, author, source)?;
    let Some((source_id, source_time)) = selected else {
        return Ok(None);
    };
    let event_id = event
        .id
        .ok_or_else(|| WriteStoreError::Refused("revision has no stable id".to_owned()))?;
    if !event_is_newer((event.created_at, event_id), (source_time, source_id)) {
        return Err(WriteStoreError::Refused(
            "revision is not newer than its selected source".to_owned(),
        ));
    }
    Ok(selected)
}

pub(super) fn validate_source(
    edit: &EventEdit,
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
            WriteStoreError::Refused("revision source has no event id".to_owned())
        })?,
        source.author(),
        source.kind(),
        source.tags(),
    ) != edit_coordinate(edit, author)
    {
        return Err(WriteStoreError::Refused(
            "revision source does not match edit coordinate".to_owned(),
        ));
    }
    Ok(Some((
        source.id().ok_or_else(|| {
            WriteStoreError::Refused("revision source has no event id".to_owned())
        })?,
        source.created_at(),
    )))
}

fn event_coordinate_of_unsigned(event: &UnsignedEvent) -> Result<EventCoordinate, WriteStoreError> {
    let id = event
        .id
        .ok_or_else(|| WriteStoreError::Refused("revision has no event id".to_owned()))?;
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
    expected: RevisionId,
    expected_source: Option<EventId>,
    current_source: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    if receipt.is_terminal()
        || receipt.write_id != write_id
        || receipt.current.publication.revision_id != expected
        || current_source.map(|(id, _)| id) != expected_source
    {
        return Err(WriteStoreError::Refused(
            "semantic revision is not current".to_owned(),
        ));
    }
    Ok(())
}
