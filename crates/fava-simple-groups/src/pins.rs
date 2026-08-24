use fava_state::EventCoordinate;
use fava_write::{EventId, EventValue, Kind, PublicKey};

use crate::SimpleGroupError;
use crate::records::record_boundary;

/// One typed event or addressable-event pin target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PinnedItem {
    /// Immutable event id target.
    Event(EventId),
    /// Addressable event coordinate target.
    Address(EventCoordinate),
}

/// Ordered pin rows from one signed kind-39005 record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupPins {
    id: String,
    author: PublicKey,
    items: Vec<Result<PinnedItem, SimpleGroupError>>,
}

impl SimpleGroupPins {
    /// Parse one signed kind-39005 record.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the signed record boundary is invalid.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupError> {
        let boundary = record_boundary(event, 39_005)?;
        let author = boundary.author();
        let items = boundary
            .tags()
            .iter()
            .enumerate()
            .filter_map(|(tag_index, tag)| {
                let values = tag.as_slice();
                match values.first().map(String::as_str) {
                    Some("e") => Some(parse_event(tag_index, values)),
                    Some("a") => Some(parse_address(tag_index, values)),
                    _ => None,
                }
            })
            .collect();
        Ok(Self {
            id: boundary.id,
            author,
            items,
        })
    }

    /// Exact simple group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Relay author that signed this record.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Source-ordered pin targets and row-local failures.
    pub fn items(&self) -> &[Result<PinnedItem, SimpleGroupError>] {
        &self.items
    }
}

fn parse_event(tag_index: usize, values: &[String]) -> Result<PinnedItem, SimpleGroupError> {
    if values.len() != 2 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "event pin must contain exactly one id",
        });
    }
    EventId::from_hex(&values[1])
        .map(PinnedItem::Event)
        .map_err(|_| SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "event pin id is invalid",
        })
}

fn parse_address(tag_index: usize, values: &[String]) -> Result<PinnedItem, SimpleGroupError> {
    if values.len() != 2 {
        return Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "address pin must contain exactly one coordinate",
        });
    }
    let mut parts = values[1].splitn(3, ':');
    let kind = parts.next().and_then(|value| value.parse::<u16>().ok());
    let author = parts
        .next()
        .and_then(|value| PublicKey::from_hex(value).ok());
    let identifier = parts.next();
    match (kind, author, identifier) {
        (Some(kind @ 30_000..=39_999), Some(author), Some(identifier)) => {
            Ok(PinnedItem::Address(EventCoordinate::Replaceable {
                author,
                kind: Kind::from_u16(kind),
                identifier: Some(identifier.to_owned()),
            }))
        }
        _ => Err(SimpleGroupError::MalformedRecordRow {
            tag_index,
            reason: "address pin coordinate is invalid",
        }),
    }
}
