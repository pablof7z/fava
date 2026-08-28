//! Kind-10009 Simple Group List reads: [`SavedGroupList`], [`SavedSimpleGroup`], and their decoder.

use std::fmt;

use fava_write::{EventValue, Kind, PublicKey};

/// One valid semantic `group` entry from a kind-10009 Simple Group List event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedSimpleGroup {
    id: String,
    relay: String,
    display_name: Option<String>,
}

impl SavedSimpleGroup {
    /// Borrow the exact group id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Borrow the exact inert relay string.
    #[must_use]
    pub fn relay(&self) -> &str {
        &self.relay
    }

    /// Return the optional exact display name.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }
}

/// One kind-10009 event decoded into public `group` and `r` entries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedGroupList {
    author: PublicKey,
    simple_groups: Vec<Result<SavedSimpleGroup, SavedGroupListDecodeError>>,
    relays: Vec<Result<String, SavedGroupListDecodeError>>,
}

impl SavedGroupList {
    /// Decode one kind-10009 event without establishing trust or provenance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava_simple_groups::SavedGroupList;
    /// # use fava_write::{EventValue, Kind, Tag, Timestamp};
    /// # use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
    /// # use nostr::key::Keys;
    /// let keys = Keys::generate();
    /// let event = NostrEventBuilder::new(Kind::from_u16(10_009), "")
    ///     .tags([
    ///         Tag::parse(["group", "photos", "wss://relay.example", "Photography"])?,
    ///         Tag::parse(["r", "wss://relay.example"])?,
    ///     ])
    ///     .custom_created_at(Timestamp::from(1))
    ///     .finalize(&keys)?;
    ///
    /// let list = SavedGroupList::from_event(&EventValue::Signed(event))?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SavedGroupListDecodeError::WrongEventKind`] for another kind.
    pub fn from_event(event: &EventValue) -> Result<Self, SavedGroupListDecodeError> {
        let expected = Kind::from_u16(10_009);
        let actual = event.kind();
        if actual != expected {
            return Err(SavedGroupListDecodeError::WrongEventKind { expected, actual });
        }
        let mut simple_groups = Vec::new();
        let mut relays = Vec::new();
        for (tag_index, tag) in event.tags().iter().enumerate() {
            let values = tag.as_slice();
            match values.first().map(String::as_str) {
                Some("group") => simple_groups.push(parse_group(values, tag_index)),
                Some("r") => relays.push(parse_relay(values, tag_index)),
                _ => {}
            }
        }
        Ok(Self {
            author: event.author(),
            simple_groups,
            relays,
        })
    }

    /// Return the event author.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Return all `group` entries and local failures in relative source order.
    pub fn simple_groups(&self) -> &[Result<SavedSimpleGroup, SavedGroupListDecodeError>] {
        &self.simple_groups
    }

    /// Return all `r` entries and local failures in relative source order.
    pub fn relays(&self) -> &[Result<String, SavedGroupListDecodeError>] {
        &self.relays
    }
}

/// A source-positioned semantic failure while decoding a Simple Group List event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SavedGroupListDecodeError {
    /// The decoder received a non-10009 event.
    WrongEventKind {
        /// Required kind.
        expected: Kind,
        /// Supplied kind.
        actual: Kind,
    },
    /// A recognized tag lacks a required position.
    MissingTagValue {
        /// Zero-based event tag index.
        tag_index: usize,
        /// Zero-based index in `Tag::as_slice()`.
        value_index: usize,
    },
}

impl fmt::Display for SavedGroupListDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongEventKind { expected, actual } => {
                write!(formatter, "wrong event kind {actual}; expected {expected}")
            }
            Self::MissingTagValue {
                tag_index,
                value_index,
            } => write!(
                formatter,
                "tag {tag_index} has no value at position {value_index}"
            ),
        }
    }
}

impl std::error::Error for SavedGroupListDecodeError {}

/// Parses a `group` tag into id, relay, and optional display name.
fn parse_group(
    values: &[String],
    tag_index: usize,
) -> Result<SavedSimpleGroup, SavedGroupListDecodeError> {
    let id = required(values, tag_index, 1)?.to_owned();
    let relay = required(values, tag_index, 2)?.to_owned();
    Ok(SavedSimpleGroup {
        id,
        relay,
        display_name: values.get(3).cloned(),
    })
}

/// Requires and returns an `r` tag's first value.
fn parse_relay(values: &[String], tag_index: usize) -> Result<String, SavedGroupListDecodeError> {
    required(values, tag_index, 1).map(str::to_owned)
}

/// Returns `values[value_index]`, or a `MissingTagValue` error positioned at `tag_index`.
fn required(
    values: &[String],
    tag_index: usize,
    value_index: usize,
) -> Result<&str, SavedGroupListDecodeError> {
    values
        .get(value_index)
        .map(String::as_str)
        .ok_or(SavedGroupListDecodeError::MissingTagValue {
            tag_index,
            value_index,
        })
}
