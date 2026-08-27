//! Shared decode error type and tag-reading helpers used by every
//! kind-3900x state-event decoder in this crate.

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
    /// A `LiveKit` participant key is not exact lowercase hex.
    InvalidLivekitParticipantPublicKey {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based value index.
        value_index: usize,
    },
}

impl fmt::Display for SimpleGroupDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongEventKind { expected, actual } => {
                write!(formatter, "wrong event kind {actual}; expected {expected}")
            }
            Self::MissingIdentifierTag => formatter.write_str("event has no d identifier tag"),
            Self::MissingTagValue {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has no value at position {value_index}"
            ),
            Self::InvalidLivekitParticipantPublicKey {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has an invalid LiveKit participant key at position {value_index}"
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
