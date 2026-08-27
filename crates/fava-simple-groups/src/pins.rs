//! Kind-39005 group pins: [`SimpleGroupPins`] and its decoder.

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
    /// Each `e` and `a` tag whose first value is present is returned as a cloned
    /// [`Tag`]. Malformed, repeated, and unrelated tags are ignored; a missing
    /// first value becomes a local [`SimpleGroupDecodeError`] without erasing
    /// valid siblings.
    ///
    /// # Examples
    ///
    /// ```
    /// use fava_simple_groups::SimpleGroupPins;
    /// use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// use nostr::key::Keys;
    ///
    /// let keys = Keys::generate();
    /// let event_id = "a".repeat(64); // placeholder hex event id
    /// let event = NostrEventBuilder::new(Kind::from_u16(39_005), "")
    ///     .tags([
    ///         Tag::parse(["d", "photos"])?,
    ///         Tag::parse(["e", &event_id])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let pins = SimpleGroupPins::from_event(&EventValue::Signed(event))?;
    /// assert_eq!(pins.id(), "photos");
    /// assert_eq!(pins.pins().len(), 1);
    /// let pin_tag = pins.pins()[0].as_ref().expect("valid pin");
    /// assert_eq!(pin_tag.as_slice().first().map(String::as_str), Some("e"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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

/// Validates the tag has a first value, then clones it unchanged.
fn parse_pin(tag: &Tag, tag_index: usize) -> Result<Tag, SimpleGroupDecodeError> {
    required_value(tag.as_slice(), tag_index, 1)?;
    Ok(tag.clone())
}
