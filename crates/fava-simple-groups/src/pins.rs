use fava_state::EventCoordinate;
use fava_write::{EventId, EventValue, PublicKey};
use nostr::nips::nip01::Coordinate;

use crate::records::{SimpleGroupDecodeError, required_value, state_event};

/// Semantic kind-39005 pin entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupPins {
    id: String,
    author: PublicKey,
    pins: Vec<Result<EventCoordinate, SimpleGroupDecodeError>>,
}

impl SimpleGroupPins {
    /// Decode one kind-39005 event, retaining `e`- and `a`-tag-local failures.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupDecodeError`] for the wrong kind or a missing first `d` value.
    pub fn from_event(event: &EventValue) -> Result<Self, SimpleGroupDecodeError> {
        let (id, author, tags) = state_event(event, 39_005)?;
        let pins = tags
            .iter()
            .enumerate()
            .filter_map(
                |(tag_index, tag)| match tag.as_slice().first().map(String::as_str) {
                    Some("e") => Some(parse_event(tag.as_slice(), tag_index)),
                    Some("a") => Some(parse_coordinate(tag.as_slice(), tag_index)),
                    _ => None,
                },
            )
            .collect();
        Ok(Self {
            id: id.to_owned(),
            author,
            pins,
        })
    }

    /// Borrow the first `d` tag's first value.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the event author.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Return interleaved `e` and `a` pins in source order with local failures.
    pub fn pins(&self) -> &[Result<EventCoordinate, SimpleGroupDecodeError>] {
        &self.pins
    }
}

fn parse_event(
    values: &[String],
    tag_index: usize,
) -> Result<EventCoordinate, SimpleGroupDecodeError> {
    let raw = required_value(values, tag_index, 1)?;
    EventId::from_hex(raw)
        .map(EventCoordinate::Event)
        .map_err(|_| SimpleGroupDecodeError::InvalidEventId {
            tag_index,
            value_index: 1,
        })
}

fn parse_coordinate(
    values: &[String],
    tag_index: usize,
) -> Result<EventCoordinate, SimpleGroupDecodeError> {
    let invalid = || SimpleGroupDecodeError::InvalidEventCoordinate {
        tag_index,
        value_index: 1,
    };
    let raw = required_value(values, tag_index, 1)?;
    let coordinate = Coordinate::from_kpi_format(raw).map_err(|_| invalid())?;
    if !coordinate.kind.is_addressable() {
        return Err(invalid());
    }
    Ok(EventCoordinate::Replaceable {
        author: coordinate.public_key,
        kind: coordinate.kind,
        identifier: Some(coordinate.identifier),
    })
}
