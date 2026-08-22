use fava_write::{Event, EventValue, PublicKey, Tag};

use crate::GroupError;

pub(crate) const MAX_RECORD_TAGS: usize = 2_000;
pub(crate) const MAX_RECORD_BYTES: usize = 131_072;
pub(crate) const MAX_RECORD_TAG_VALUES: usize = 256;
pub(crate) const MAX_RECORD_VALUE_BYTES: usize = 4_096;

pub(crate) struct RecordBoundary<'a> {
    pub(crate) event: &'a Event,
    pub(crate) id: String,
}

impl RecordBoundary<'_> {
    pub(crate) fn author(&self) -> PublicKey {
        self.event.pubkey
    }

    pub(crate) fn tags(&self) -> &[Tag] {
        self.event.tags.as_slice()
    }
}

pub(crate) fn record_boundary(
    event: &EventValue,
    expected_kind: u16,
) -> Result<RecordBoundary<'_>, GroupError> {
    if event.kind().as_u16() != expected_kind {
        return Err(GroupError::WrongRecordKind {
            expected: expected_kind,
            actual: event.kind().as_u16(),
        });
    }
    let EventValue::Signed(event) = event else {
        return Err(GroupError::UnsignedRecord);
    };
    if !event.verify_id() {
        return Err(GroupError::InvalidRecordId);
    }
    if !event.verify_signature() {
        return Err(GroupError::InvalidRecordSignature);
    }

    let mut id = None;
    for tag in event.tags.iter() {
        let values = tag.as_slice();
        if values.first().map(String::as_str) != Some("d") {
            continue;
        }
        let Some(value) = values.get(1) else {
            return Err(GroupError::EmptyRecordId);
        };
        if value.is_empty() {
            return Err(GroupError::EmptyRecordId);
        }
        match id.as_deref() {
            None => id = Some(value.clone()),
            Some(existing) if existing == value => return Err(GroupError::DuplicateRecordId),
            Some(_) => return Err(GroupError::ConflictingRecordId),
        }
    }

    Ok(RecordBoundary {
        event,
        id: id.ok_or(GroupError::MissingRecordId)?,
    })
}
