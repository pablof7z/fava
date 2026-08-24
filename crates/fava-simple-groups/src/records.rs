use fava_write::{Event, EventValue, PublicKey, Tag};

use crate::SimpleGroupError;

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
) -> Result<RecordBoundary<'_>, SimpleGroupError> {
    if event.kind().as_u16() != expected_kind {
        return Err(SimpleGroupError::WrongRecordKind {
            expected: expected_kind,
            actual: event.kind().as_u16(),
        });
    }
    let EventValue::Signed(event) = event else {
        return Err(SimpleGroupError::UnsignedRecord);
    };
    validate_structure(event)?;
    if !event.verify_id() {
        return Err(SimpleGroupError::InvalidRecordId);
    }
    if !event.verify_signature() {
        return Err(SimpleGroupError::InvalidRecordSignature);
    }

    let mut id = None;
    for (tag_index, tag) in event.tags.iter().enumerate() {
        let values = tag.as_slice();
        if values.first().map(String::as_str) != Some("d") {
            continue;
        }
        let Some(value) = values.get(1) else {
            return Err(SimpleGroupError::EmptyRecordId);
        };
        if value.is_empty() {
            return Err(SimpleGroupError::EmptyRecordId);
        }
        if values.len() != 2 {
            return Err(SimpleGroupError::MalformedRecordRow {
                tag_index,
                reason: "d row must contain exactly one value",
            });
        }
        match id.as_deref() {
            None => id = Some(value.clone()),
            Some(existing) if existing == value => return Err(SimpleGroupError::DuplicateRecordId),
            Some(_) => return Err(SimpleGroupError::ConflictingRecordId),
        }
    }

    Ok(RecordBoundary {
        event,
        id: id.ok_or(SimpleGroupError::MissingRecordId)?,
    })
}

pub(crate) fn saved_boundary(event: &EventValue) -> Result<&Event, SimpleGroupError> {
    if event.kind().as_u16() != 10_009 {
        return Err(SimpleGroupError::WrongRecordKind {
            expected: 10_009,
            actual: event.kind().as_u16(),
        });
    }
    let EventValue::Signed(event) = event else {
        return Err(SimpleGroupError::UnsignedRecord);
    };
    validate_structure(event)?;
    if !event.verify_id() {
        return Err(SimpleGroupError::InvalidRecordId);
    }
    if !event.verify_signature() {
        return Err(SimpleGroupError::InvalidRecordSignature);
    }
    Ok(event)
}

pub(crate) fn validate_structure(event: &Event) -> Result<(), SimpleGroupError> {
    if event.tags.len() > MAX_RECORD_TAGS {
        return Err(SimpleGroupError::TooManyRecordTags {
            actual: event.tags.len().min(MAX_RECORD_TAGS.saturating_add(1)),
            maximum: MAX_RECORD_TAGS,
        });
    }

    let mut bytes = 0usize;
    add_bytes(&mut bytes, event.content.len())?;
    for (tag_index, tag) in event.tags.iter().enumerate() {
        let values = tag.as_slice();
        if values.len() > MAX_RECORD_TAG_VALUES {
            return Err(SimpleGroupError::TooManyRecordTagValues {
                tag_index,
                actual: values.len().min(MAX_RECORD_TAG_VALUES.saturating_add(1)),
                maximum: MAX_RECORD_TAG_VALUES,
            });
        }
        for (value_index, value) in values.iter().enumerate() {
            if value.len() > MAX_RECORD_VALUE_BYTES {
                return Err(SimpleGroupError::RecordValueTooLong {
                    tag_index,
                    value_index,
                    bytes: value.len(),
                    maximum: MAX_RECORD_VALUE_BYTES,
                });
            }
            add_bytes(&mut bytes, value.len())?;
            add_bytes(&mut bytes, 1)?;
        }
    }
    Ok(())
}

fn add_bytes(bytes: &mut usize, amount: usize) -> Result<(), SimpleGroupError> {
    *bytes = bytes
        .checked_add(amount)
        .ok_or(SimpleGroupError::RecordTooLarge {
            bytes: MAX_RECORD_BYTES.saturating_add(1),
            maximum: MAX_RECORD_BYTES,
        })?;
    if *bytes > MAX_RECORD_BYTES {
        return Err(SimpleGroupError::RecordTooLarge {
            bytes: (*bytes).min(MAX_RECORD_BYTES.saturating_add(1)),
            maximum: MAX_RECORD_BYTES,
        });
    }
    Ok(())
}
