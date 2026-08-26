use std::collections::BTreeSet;
use std::fmt;

use fava_query::{Query, QueryError, SingleLetterTag};
use fava_write::{EventBuilder, Tag, WriteIntentError};
use nostr::types::RelayUrl;

use crate::SimpleGroupStateEventKind;

/// An opaque simple-group id plus normalized non-empty application-selected relays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroup {
    id: String,
    relays: Vec<RelayUrl>,
}

/// A caller-attributable refusal to construct a [`SimpleGroup`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimpleGroupConstructionError {
    /// The supplied group id is exactly empty.
    EmptyId,
    /// The supplied relay vector is empty.
    EmptyRelays,
}

impl fmt::Display for SimpleGroupConstructionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => formatter.write_str("simple group id must not be empty"),
            Self::EmptyRelays => formatter.write_str("simple group relays must not be empty"),
        }
    }
}

impl std::error::Error for SimpleGroupConstructionError {}

impl SimpleGroup {
    /// Construct from a non-empty id and finite non-empty owned relay vector.
    ///
    /// Later duplicate relay identities collapse while first-occurrence order
    /// is preserved. The concrete tail makes construction finite without a
    /// domain limit; query and write operations apply their own resource caps
    /// when this value is lowered.
    ///
    /// ```
    /// use fava_simple_groups::SimpleGroup;
    /// use nostr::types::RelayUrl;
    ///
    /// let first = RelayUrl::parse("wss://a.example")?;
    /// let second = RelayUrl::parse("wss://b.example")?;
    /// let group = SimpleGroup::new("photos", vec![first, second])?;
    /// assert_eq!(group.relays().count(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// Arbitrary iterators are not accepted as the finite relay input.
    ///
    /// ```compile_fail
    /// use fava_simple_groups::SimpleGroup;
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example").unwrap();
    /// let _ = SimpleGroup::new("photos", std::iter::repeat(relay));
    /// ```
    ///
    /// The superseded head-plus-tail signature is not retained.
    ///
    /// ```compile_fail
    /// use fava_simple_groups::SimpleGroup;
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example").unwrap();
    /// let _ = SimpleGroup::new("photos", relay.clone(), vec![relay]);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupConstructionError::EmptyId`] when `id` is exactly
    /// empty, or [`SimpleGroupConstructionError::EmptyRelays`] when `relays`
    /// is empty.
    pub fn new(
        id: impl Into<String>,
        relays: Vec<RelayUrl>,
    ) -> Result<Self, SimpleGroupConstructionError> {
        let id = id.into();
        if id.is_empty() {
            return Err(SimpleGroupConstructionError::EmptyId);
        }
        if relays.is_empty() {
            return Err(SimpleGroupConstructionError::EmptyRelays);
        }
        let mut seen = BTreeSet::new();
        let mut normalized_relays = Vec::with_capacity(relays.len());
        for relay in relays {
            if seen.insert(relay.clone()) {
                normalized_relays.push(relay);
            }
        }
        Ok(Self {
            id,
            relays: normalized_relays,
        })
    }

    /// Borrow the non-empty opaque id exactly as supplied.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Iterate over cloned relays in normalized first-occurrence order.
    pub fn relays(&self) -> impl Iterator<Item = RelayUrl> + '_ {
        self.relays.iter().cloned()
    }

    /// Compose this group's `h` value and relay acquisition into an ordinary query.
    ///
    /// Query-owned exact intersection narrows any existing lowercase `h` axis.
    /// A disjoint axis remains present-empty and therefore matches nothing.
    /// This crate neither validates nor translates query failures.
    ///
    /// # Examples
    ///
    /// ```
    /// use fava_simple_groups::SimpleGroup;
    /// use fava_query::{Query, SingleLetterTag};
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let group = SimpleGroup::new("photos", vec![relay])?;
    ///
    /// // An open query is narrowed to this group's h axis and relays.
    /// let query = group.events(Query::events())?;
    /// let h = SingleLetterTag::from_char('h').expect("h is a valid tag");
    /// assert!(
    ///     query.selection().tag_values.get(&h)
    ///         .map_or(false, |v| v.contains("photos")),
    /// );
    ///
    /// // A disjoint h axis produces a match-nothing query rather than being rewritten.
    /// let disjoint = Query::events().tag_values(h, ["other"])?;
    /// let nothing = group.events(disjoint)?;
    /// assert_eq!(
    ///     nothing.selection().tag_values.get(&h).map(|v| v.is_empty()),
    ///     Some(true),
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the owning [`QueryError`] when bounded tag or relay composition is refused.
    pub fn events(&self, selection: Query) -> Result<Query, QueryError> {
        selection
            .intersect_tag_values(group_tag(), [self.id()])?
            .from_relays(self.relays())
    }

    /// Build an exact-`d`, relay-authoritative query for selected NIP-29 meta-event kinds.
    ///
    /// # Examples
    ///
    /// ```
    /// use fava_simple_groups::{SimpleGroup, SimpleGroupStateEventKind};
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let group = SimpleGroup::new("photos", vec![relay])?;
    ///
    /// // Query all six NIP-29 state kinds for this group.
    /// let all_state = group.meta_events(SimpleGroupStateEventKind::ALL)?;
    ///
    /// // Query only metadata and members.
    /// let subset = group.meta_events([
    ///     SimpleGroupStateEventKind::Metadata,
    ///     SimpleGroupStateEventKind::Members,
    /// ])?;
    ///
    /// // Empty kind set produces a query that matches nothing.
    /// let empty = group.meta_events([])?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the owning [`QueryError`] when bounded kind, tag, or relay composition is refused.
    pub fn meta_events<I>(&self, kinds: I) -> Result<Query, QueryError>
    where
        I: IntoIterator<Item = SimpleGroupStateEventKind>,
    {
        let query = Query::events().kinds(kinds.into_iter().map(Into::into))?;
        query
            .tag_values(identifier_tag(), [self.id()])?
            .only_from_relays(self.relays())
    }
}

/// Fluent NIP-29 group composition for the concrete generic event builder.
pub trait SimpleGroupEventBuilder {
    /// Add this group's exact `h` context and local relay contribution.
    ///
    /// The returned value remains [`EventBuilder`]. Relays are local
    /// publication intent, not Nostr event data. The generic route owner
    /// handles normalization and boundedness.
    ///
    /// Existing tags are preserved unchanged. If an exact `h = <id>` tag is
    /// already present no duplicate tag is added. Malformed, repeated,
    /// extended, and unrelated tags are not affected. Calling this method for
    /// several groups accumulates their distinct exact `h` tags and the union
    /// of their relays in first-occurrence order.
    ///
    /// # Examples
    ///
    /// ```
    /// use fava_simple_groups::SimpleGroup;
    /// use fava_write::{EventBuilder, Kind, Timestamp};
    /// use nostr::key::Keys;
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example")?;
    /// let group = SimpleGroup::new("photos", vec![relay])?;
    /// let keys = Keys::generate();
    ///
    /// let builder = EventBuilder::new(keys.public_key(), Kind::from_u16(9))
    ///     .created_at(Timestamp::from(1))
    ///     .content("hello")
    ///     .simple_group(&group)?;
    ///
    /// let (event, routing) = builder.into_event_and_routing()?;
    /// let has_h = event.tags.iter().any(|t| {
    ///     let s = t.as_slice();
    ///     s.first().map(String::as_str) == Some("h")
    ///         && s.get(1).map(String::as_str) == Some("photos")
    /// });
    /// assert!(has_h);
    /// assert!(matches!(routing, fava_write::WriteRouting::Explicit(_)));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] directly when generic route accumulation
    /// is refused.
    fn simple_group(self, group: &SimpleGroup) -> Result<EventBuilder, WriteIntentError>;
}

impl SimpleGroupEventBuilder for EventBuilder {
    fn simple_group(self, group: &SimpleGroup) -> Result<EventBuilder, WriteIntentError> {
        let tag = Tag::parse(["h", group.id()])
            .expect("a non-empty opaque simple-group id forms a valid h tag");
        let builder = self.to_relays(group.relays())?;
        if builder
            .event_tags()
            .iter()
            .any(|existing| exact_group_tag(existing, group.id()))
        {
            return Ok(builder);
        }
        Ok(builder.tag(tag))
    }
}

fn group_tag() -> SingleLetterTag {
    SingleLetterTag::from_char('h').expect("h is a valid single-letter tag")
}

fn identifier_tag() -> SingleLetterTag {
    SingleLetterTag::from_char('d').expect("d is a valid single-letter tag")
}

fn exact_group_tag(tag: &Tag, id: &str) -> bool {
    let values = tag.as_slice();
    values.len() == 2
        && values.first().map(String::as_str) == Some("h")
        && values.get(1).map(String::as_str) == Some(id)
}
