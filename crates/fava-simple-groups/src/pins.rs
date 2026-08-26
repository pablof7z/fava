use fava_write::{EventValue, PublicKey, Tag};

use crate::records::{SimpleGroupDecodeError, required_value, state_event};

/// Semantic kind-39005 pin entries from one event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroupPins {
    id: String,
    author: PublicKey,
    pins: Vec<Result<Tag, SimpleGroupDecodeError>>,
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
            .filter(|(_, tag)| {
                matches!(tag.as_slice().first().map(String::as_str), Some("e" | "a"))
            })
            .map(|(tag_index, tag)| parse_pin(tag, tag_index))
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
    pub fn pins(&self) -> &[Result<Tag, SimpleGroupDecodeError>] {
        &self.pins
    }
}

fn parse_pin(tag: &Tag, tag_index: usize) -> Result<Tag, SimpleGroupDecodeError> {
    required_value(tag.as_slice(), tag_index, 1)?;
    Ok(tag.clone())
}
