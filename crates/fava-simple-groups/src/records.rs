use std::fmt;

use fava_write::{EventValue, Kind, PublicKey, Tag};

/// A source-positioned semantic failure while decoding a NIP-29 state event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimpleGroupDecodeError {
    /// The selected decoder does not own the supplied kind.
    WrongEventKind {
        /// Required kind.
        expected: Kind,
        /// Supplied kind.
        actual: Kind,
    },
    /// No `d` tag exists.
    MissingIdentifierTag,
    /// A recognized tag lacks a required position.
    MissingTagValue {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based index in `Tag::as_slice()`.
        value_index: usize,
    },
    /// An administrator or member key is invalid.
    InvalidPublicKey {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
    /// A `LiveKit` participant key is not exact lowercase hex.
    InvalidLivekitParticipantPublicKey {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
    /// A `supported_kinds` value is not a decimal `u16`.
    InvalidKind {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
    /// An `e` pin value is not an event id.
    InvalidEventId {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
    /// An `a` pin value is not an addressable event coordinate.
    InvalidEventCoordinate {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
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
    validate_structure_parts(&event.content, event.tags.as_slice())
}

pub(crate) fn validate_value_structure(event: &EventValue) -> Result<(), SimpleGroupError> {
    let content = match event {
        EventValue::Unsigned(event) => event.content.as_str(),
        EventValue::Signed(event) => event.content.as_str(),
    };
    validate_structure_parts(content, event.tags())
}

fn validate_structure_parts(content: &str, tags: &[Tag]) -> Result<(), SimpleGroupError> {
    if tags.len() > MAX_RECORD_TAGS {
        return Err(SimpleGroupError::TooManyRecordTags {
            actual: tags.len().min(MAX_RECORD_TAGS.saturating_add(1)),
            maximum: MAX_RECORD_TAGS,
        });
    }

    let mut bytes = 0usize;
    add_bytes(&mut bytes, content.len())?;
    for (tag_index, tag) in tags.iter().enumerate() {
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
            Self::MissingIdentifierTag => formatter.write_str("event has no d identifier tag"),
            Self::MissingTagValue {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has no value at position {value_index}"
            ),
            Self::InvalidPublicKey {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid public key at position {value_index}"
            ),
            Self::InvalidLivekitParticipantPublicKey {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid LiveKit participant key at position {value_index}"
            ),
            Self::InvalidKind {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid kind at position {value_index}"
            ),
            Self::InvalidEventId {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid event id at position {value_index}"
            ),
            Self::InvalidEventCoordinate {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid event coordinate at position {value_index}"
            ),
        }
    }
}

impl std::error::Error for SimpleGroupDecodeError {}

pub(crate) fn state_event(
    event: &EventValue,
    expected: u16,
) -> Result<(&str, PublicKey, &[Tag]), SimpleGroupDecodeError> {
    let expected = Kind::from_u16(expected);
    let actual = event.kind();
    if actual != expected {
        return Err(SimpleGroupDecodeError::WrongEventKind { expected, actual });
    }
    let tags = event.tags();
    let Some((tag_index, tag)) = tags
        .iter()
        .enumerate()
        .find(|(_, tag)| tag.as_slice().first().map(String::as_str) == Some("d"))
    else {
        return Err(SimpleGroupDecodeError::MissingIdentifierTag);
    };
    let id = tag.as_slice().get(1).map(String::as_str).ok_or(
        SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index: 1,
        },
    )?;
    Ok((id, event.author(), tags))
}

pub(crate) fn required_value(
    values: &[String],
    tag_index: usize,
    value_index: usize,
) -> Result<&str, SimpleGroupDecodeError> {
    values
        .get(value_index)
        .map(String::as_str)
        .ok_or(SimpleGroupDecodeError::MissingTagValue {
            tag_index,
            value_index,
        })
}
