use nostr::event::{Event, Kind, Tag, UnsignedEvent};
use nostr::key::PublicKey;
use nostr::types::{RelayUrl, Timestamp};
use thiserror::Error;

use crate::routing::refuse_raw_input;
use crate::{MAX_EVENT_BYTES, WriteIntentError, WriteRouting};

const MAX_TAGS: usize = 2_000;

// ── Taggable sealed trait ────────────────────────────────────────────────────

mod sealed {
    pub trait Sealed {}
}

/// Converts a value into the one or more [`Tag`]s it contributes to an event.
///
/// This trait is sealed: only the implementations within `fava-write` are
/// valid. External callers do not need to import it — the compiler resolves
/// the impl automatically when calling [`EventBuilder::tag`].
///
/// | Type | Tags added |
/// |------|-----------|
/// | [`Tag`] | the tag itself |
/// | [`PublicKey`] | `["p", pubkey_hex]` |
/// | `&`[`Event`] | NIP-22: `["e", id, "", pubkey]` + `["p", pubkey]` + `["k", kind]` |
/// | `[S; N]` where `S: Into<String>` | one raw tag via `Tag::parse` |
pub trait Taggable: sealed::Sealed {
    /// Convert this value into the tag(s) it contributes to an event.
    fn into_tags(self) -> Vec<Tag>;
}

// Tag → single raw tag (existing behaviour, backwards-compatible)
impl sealed::Sealed for Tag {}
impl Taggable for Tag {
    fn into_tags(self) -> Vec<Tag> {
        vec![self]
    }
}

// PublicKey → ["p", pubkey_hex]
impl sealed::Sealed for PublicKey {}
impl Taggable for PublicKey {
    fn into_tags(self) -> Vec<Tag> {
        vec![Tag::parse(["p", &self.to_hex()]).expect("p tag from public key")]
    }
}

// &Event → NIP-22: ["e", id, "", pubkey] + ["p", pubkey] + ["k", kind]
impl sealed::Sealed for &Event {}
impl Taggable for &Event {
    fn into_tags(self) -> Vec<Tag> {
        let pubkey_hex = self.pubkey.to_hex();
        let e_tag = Tag::parse(["e", &self.id.to_hex(), "", &pubkey_hex])
            .expect("e tag from event");
        let p_tag =
            Tag::parse(["p", &pubkey_hex]).expect("p tag from event author");
        let k_tag = Tag::parse(["k", &self.kind.to_string()])
            .expect("k tag from event kind");
        vec![e_tag, p_tag, k_tag]
    }
}

// [S; N] where S: Into<String> → Tag::parse(arr)
impl<S: Into<String>, const N: usize> sealed::Sealed for [S; N] {}
impl<S: Into<String>, const N: usize> Taggable for [S; N] {
    fn into_tags(self) -> Vec<Tag> {
        if N == 0 {
            return vec![];
        }
        vec![Tag::parse(self).expect("raw tag array must not be empty")]
    }
}

// ── EventBuilder ─────────────────────────────────────────────────────────────

/// Incrementally assembles one complete unsigned Nostr event.
///
/// Every field of a Nostr event feeds the same deterministic event id, so
/// filling fields in through separate, order-dependent mutations is a common
/// source of subtly wrong ids. `EventBuilder` starts from just an author and
/// a kind, accepts timestamp, content, and tags through owned builder calls
/// in any order, and only computes the id and checks the declared tag and
/// byte bounds once, in [`EventBuilder::build`]. Reach for it when assembling
/// an event from application-level fields; when every field is already known
/// up front, such as re-encoding a previously decoded event, construct it
/// directly with [`EventBuilder::from_parts`] instead.
///
/// # Examples
///
/// ```
/// use fava_write::{EventBuilder, Kind, PublicKey};
///
/// let author = PublicKey::from_hex(
///     "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
/// )
/// .expect("valid hex public key");
///
/// let event = EventBuilder::new(author, Kind::TextNote)
///     .content("gm")
///     .build()
///     .expect("event stays within declared bounds");
///
/// assert_eq!(event.pubkey, author);
/// assert_eq!(event.kind, Kind::TextNote);
/// assert_eq!(event.content, "gm");
/// assert!(event.id.is_some());
/// ```
pub struct EventBuilder {
    author: PublicKey,
    kind: Kind,
    created_at: Timestamp,
    content: String,
    tags: Vec<Tag>,
    routing: WriteRouting,
    raw_routing_inputs: usize,
}

impl EventBuilder {
    /// Begin one event body without interpreting its kind.
    #[must_use]
    pub fn new(author: PublicKey, kind: Kind) -> Self {
        Self::from_parts(author, kind, Timestamp::now(), Vec::new(), String::new())
    }

    /// Begin one event from exact raw Nostr parts without interpreting any field.
    ///
    /// # Arguments
    ///
    /// * `author` - the event's signing public key
    /// * `kind` - the event kind
    /// * `created_at` - the exact event timestamp, taken as-is
    /// * `tags` - the exact event tags, taken in the supplied order
    /// * `content` - the opaque event content
    #[must_use]
    pub fn from_parts(
        author: PublicKey,
        kind: Kind,
        created_at: Timestamp,
        tags: Vec<Tag>,
        content: String,
    ) -> Self {
        Self {
            author,
            kind,
            created_at,
            content,
            tags,
            routing: WriteRouting::Automatic,
            raw_routing_inputs: 0,
        }
    }

    /// Set the exact event timestamp.
    #[must_use]
    pub const fn created_at(mut self, created_at: Timestamp) -> Self {
        self.created_at = created_at;
        self
    }

    /// Set opaque event content.
    #[must_use]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Append already-validated Nostr tags in their exact input order.
    #[must_use]
    pub fn tags(mut self, tags: impl IntoIterator<Item = Tag>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Append the tag(s) determined by the target type.
    ///
    /// Accepts any type that implements [`Taggable`]:
    ///
    /// - [`Tag`] — appends the tag directly (existing behaviour)
    /// - [`PublicKey`] — appends `["p", pubkey_hex]`
    /// - `&`[`Event`] — NIP-22: appends `["e", id, "", pubkey]` +
    ///   `["p", pubkey]` + `["k", kind]`
    /// - `[S; N] where S: Into<String>` — parses as one raw tag
    #[must_use]
    pub fn tag(self, target: impl Taggable) -> Self {
        self.tags(target.into_tags())
    }

    /// Append a NIP-10 style marked event reference.
    ///
    /// Produces `["e", id, "", marker]` + `["p", author_pubkey]`.
    ///
    /// Use when the event being built needs a threaded reply marker (`"reply"`,
    /// `"root"`, `"mention"`) in the NIP-10 deprecated style. Unlike
    /// `.tag(&event)`, this does **not** add a `k` tag and places the marker
    /// at position 3 of the `e` tag rather than the author pubkey.
    ///
    /// NIP-10 markers and NIP-22 pubkey hints occupy the same `e`-tag position
    /// and must not be combined for the same referenced event.
    #[must_use]
    pub fn tag_event_marked(self, event: &Event, marker: &str) -> Self {
        let pubkey_hex = event.pubkey.to_hex();
        let e_tag = Tag::parse(["e", &event.id.to_hex(), "", marker])
            .expect("e tag with marker from event");
        let p_tag =
            Tag::parse(["p", &pubkey_hex]).expect("p tag from marked event author");
        self.tags([e_tag, p_tag])
    }

    /// Borrow every exact event tag in insertion order.
    #[must_use]
    pub fn event_tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Add relays to the builder's local explicit publication route.
    ///
    /// The route is not serialized or signed. Duplicate relay identities
    /// collapse in first-occurrence order under the write owner's bound.
    ///
    /// # Errors
    ///
    /// Returns the owning [`WriteIntentError`] when route accumulation is
    /// empty, cumulative raw input exceeds 1,024 occurrences, or the normalized
    /// route exceeds 256 distinct destinations.
    pub fn to_relays(mut self, relays: impl Into<Vec<RelayUrl>>) -> Result<Self, WriteIntentError> {
        let relays = relays.into();
        let raw_routing_inputs = self.raw_routing_inputs.checked_add(relays.len()).ok_or(
            WriteIntentError::TooManyRawExplicitRelays {
                actual: usize::MAX,
                maximum: crate::routing::MAX_RAW_EXPLICIT_RELAYS,
            },
        )?;
        refuse_raw_input(raw_routing_inputs)?;
        self.routing = self.routing.append(relays)?;
        self.raw_routing_inputs = raw_routing_inputs;
        Ok(self)
    }

    /// Produce the exact unsigned body and deterministic event id.
    ///
    /// # Errors
    ///
    /// Returns [`EventBuildError`] when event structure exceeds declared bounds.
    pub fn build(self) -> Result<UnsignedEvent, EventBuildError> {
        if matches!(self.routing, WriteRouting::Explicit(_)) {
            return Err(EventBuildError::ExplicitRoutingAttached);
        }
        self.build_event()
    }

    /// Consume the exact unsigned event and its neutral publication route.
    ///
    /// # Errors
    ///
    /// Returns [`EventBuildError`] when event structure exceeds declared bounds.
    pub fn into_event_and_routing(self) -> Result<(UnsignedEvent, WriteRouting), EventBuildError> {
        let routing = self.routing.clone();
        Ok((self.build_event()?, routing))
    }

    fn build_event(self) -> Result<UnsignedEvent, EventBuildError> {
        if self.tags.len() > MAX_TAGS {
            return Err(EventBuildError::TooManyTags {
                actual: self.tags.len(),
                maximum: MAX_TAGS,
            });
        }
        let mut event = UnsignedEvent::new(
            self.author,
            self.created_at,
            self.kind,
            self.tags,
            self.content,
        );
        event.ensure_id();
        let bytes = serde_json::to_vec(&event)
            .map_err(|error| EventBuildError::Encoding(error.to_string()))?
            .len();
        if bytes > MAX_EVENT_BYTES {
            return Err(EventBuildError::TooLarge {
                bytes,
                maximum: MAX_EVENT_BYTES,
            });
        }
        Ok(event)
    }
}

/// Checked event-construction refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EventBuildError {
    /// Event contains too many tags.
    #[error("event tags exceed bound: {actual} > {maximum}")]
    TooManyTags {
        /// Actual tag count.
        actual: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Serialized event exceeds the declared byte bound.
    #[error("event bytes exceed bound: {bytes} > {maximum}")]
    TooLarge {
        /// Actual serialized bytes.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Exact event serialization failed.
    #[error("event encoding failed: {0}")]
    Encoding(String),
    /// Event-only construction would discard local explicit routing.
    #[error("event-only construction cannot discard explicit publication routing")]
    ExplicitRoutingAttached,
}
