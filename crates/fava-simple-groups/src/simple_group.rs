use std::collections::BTreeSet;
use std::fmt;

use fava_query::{Query, QueryError, SingleLetterTag};
use fava_write::{EventBuildError, EventBuilder, Tag, UnsignedEvent};
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
    /// let group = SimpleGroup::from_relays("photos", vec![first, second])?;
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
    /// let _ = SimpleGroup::from_relays("photos", std::iter::repeat(relay));
    /// ```
    ///
    /// The superseded head-plus-tail signature is not retained.
    ///
    /// ```compile_fail
    /// use fava_simple_groups::SimpleGroup;
    /// use nostr::types::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example").unwrap();
    /// let _ = SimpleGroup::from_relays("photos", relay.clone(), vec![relay]);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupConstructionError::EmptyId`] when `id` is exactly
    /// empty, or [`SimpleGroupConstructionError::EmptyRelays`] when `relays`
    /// is empty.
    pub fn from_relays(
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
    /// let group = SimpleGroup::from_relays("photos", vec![relay])?;
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
    /// let group = SimpleGroup::from_relays("photos", vec![relay])?;
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

    /// Add one matching `h` tag to an unsigned event when none is already present.
    ///
    /// Existing tags are preserved unchanged. If an exact `h = <id>` tag is
    /// already present the event is returned unmodified. Malformed, repeated,
    /// extended, and unrelated tags are not affected.
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
    /// let group = SimpleGroup::from_relays("photos", vec![relay])?;
    /// let keys = Keys::generate();
    ///
    /// let draft = EventBuilder::new(keys.public_key(), Kind::from_u16(9))
    ///     .created_at(Timestamp::from(1))
    ///     .content("hello")
    ///     .build()?;
    ///
    /// let prepared = group.prepare(draft)?;
    /// let has_h = prepared.tags.iter().any(|t| {
    ///     let s = t.as_slice();
    ///     s.first().map(String::as_str) == Some("h")
    ///         && s.get(1).map(String::as_str) == Some("photos")
    /// });
    /// assert!(has_h);
    ///
    /// // Calling prepare again on an already-tagged event leaves it unchanged.
    /// let tag_count = prepared.tags.len();
    /// let again = group.prepare(prepared)?;
    /// assert_eq!(again.tags.len(), tag_count);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the generic builder's [`EventBuildError`] when rebuilding is refused.
    pub fn prepare(&self, event: UnsignedEvent) -> Result<UnsignedEvent, EventBuildError> {
        if event.tags.iter().any(|tag| matching_h(tag, self.id())) {
            return Ok(event);
        }
        let mut tags = event.tags.to_vec();
        tags.push(
            Tag::parse(["h", self.id()])
                .map_err(|error| EventBuildError::Encoding(error.to_string()))?,
        );
        EventBuilder::from_parts(
            event.pubkey,
            event.kind,
            event.created_at,
            tags,
            event.content,
        )
        .build()
    }
}

fn group_tag() -> SingleLetterTag {
    SingleLetterTag::from_char('h').expect("h is a valid single-letter tag")
}

fn identifier_tag() -> SingleLetterTag {
    SingleLetterTag::from_char('d').expect("d is a valid single-letter tag")
}

fn matching_h(tag: &Tag, id: &str) -> bool {
    let values = tag.as_slice();
    values.first().map(String::as_str) == Some("h") && values.get(1).map(String::as_str) == Some(id)
}
