use std::collections::BTreeSet;

use fava_query::{Query, QueryError, SingleLetterTag};
use fava_state::RelayUrl;
use fava_write::{EventBuildError, EventBuilder, Tag, UnsignedEvent};

use crate::SimpleGroupStateEventKind;

/// An opaque simple-group id plus normalized non-empty application-selected relays.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroup {
    id: String,
    relays: Vec<RelayUrl>,
}

impl SimpleGroup {
    /// Construct from one required relay and a finite owned remainder.
    ///
    /// Later duplicate relay identities collapse while first-occurrence order
    /// is preserved. The concrete tail makes construction finite without a
    /// domain limit; query and write operations apply their own resource caps
    /// when this value is lowered.
    ///
    /// ```
    /// use fava_simple_groups::SimpleGroup;
    /// use fava_state::RelayUrl;
    ///
    /// let first = RelayUrl::parse("wss://a.example")?;
    /// let second = RelayUrl::parse("wss://b.example")?;
    /// let group = SimpleGroup::from_relays("photos", first, vec![second]);
    /// assert_eq!(group.relays().count(), 2);
    /// # Ok::<(), nostr::error::Error>(())
    /// ```
    ///
    /// Empty construction is impossible because the first relay is required.
    ///
    /// ```compile_fail
    /// use fava_simple_groups::SimpleGroup;
    /// use fava_state::RelayUrl;
    ///
    /// let no_relays: Vec<RelayUrl> = Vec::new();
    /// let _ = SimpleGroup::from_relays("photos", no_relays);
    /// ```
    ///
    /// Arbitrary iterators are not accepted as the finite tail.
    ///
    /// ```compile_fail
    /// use fava_simple_groups::SimpleGroup;
    /// use fava_state::RelayUrl;
    ///
    /// let relay = RelayUrl::parse("wss://relay.example").unwrap();
    /// let _ = SimpleGroup::from_relays("photos", relay.clone(), std::iter::repeat(relay));
    /// ```
    #[must_use]
    pub fn from_relays(id: impl Into<String>, first: RelayUrl, rest: Vec<RelayUrl>) -> Self {
        let mut seen = BTreeSet::new();
        let mut relays = Vec::with_capacity(rest.len().saturating_add(1));
        for relay in std::iter::once(first).chain(rest) {
            if seen.insert(relay.clone()) {
                relays.push(relay);
            }
        }
        Self {
            id: id.into(),
            relays,
        }
    }

    /// Borrow the opaque id exactly as supplied.
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
    /// # Errors
    ///
    /// Returns the owning [`QueryError`] when bounded tag or relay composition is refused.
    pub fn events(&self, selection: Query) -> Result<Query, QueryError> {
        selection
            .intersect_tag_values(group_tag(), [self.id()])?
            .from_relays(self.relays())
    }

    /// Build an exact-`d`, relay-authoritative query for selected NIP-29 state-event kinds.
    ///
    /// # Errors
    ///
    /// Returns the owning [`QueryError`] when bounded kind, tag, or relay composition is refused.
    pub fn state_events<I>(&self, kinds: I) -> Result<Query, QueryError>
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
