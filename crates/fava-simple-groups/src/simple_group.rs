use std::collections::BTreeSet;
use std::fmt;

use fava_query::{Query, QueryError, QuerySnapshot};
use fava_state::RelayUrl;
use fava_write::{Event, EventBuildError, EventBuilder, Tag, UnsignedEvent, WriteIntentError};

use crate::SimpleGroupRecords;
use crate::bounds::{
    MAX_SIMPLE_GROUP_CONTEXT_INPUT_ITEMS, MAX_SIMPLE_GROUP_HOST_INPUT_ITEMS,
    MAX_SIMPLE_GROUP_ID_BYTES, collect_at_most,
};

/// One opaque NIP-29 simple group id over an application-selected host set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleGroup {
    hosts: Vec<RelayUrl>,
    id: String,
}

/// Typed refusal while constructing or transforming a simple group value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimpleGroupError {
    /// A host value is not a valid relay URL.
    InvalidHost(String),
    /// A simple group requires at least one host relay.
    EmptyHosts,
    /// The host set repeats one exact relay identity.
    DuplicateHost {
        /// Repeated exact relay identity.
        relay: RelayUrl,
    },
    /// Host input exceeds the shared explicit-route bound.
    TooManyHosts {
        /// Total host inputs observed before refusal.
        actual: usize,
        /// Maximum supported distinct host count.
        maximum: usize,
    },
    /// An opaque simple group id may not be empty.
    EmptyId,
    /// The opaque simple group id exceeds its byte bound.
    SimpleGroupIdTooLong {
        /// Actual id length in bytes.
        bytes: usize,
        /// Maximum supported id length in bytes.
        maximum: usize,
    },
    /// A simple group query was refused before work opened.
    Query(String),
    /// Event preparation was refused before custody.
    Event(String),
    /// A contextual `h` tag is missing its value.
    EmptySimpleGroupContext,
    /// A signed event has no contextual `h` tag.
    MissingSimpleGroupContext,
    /// More than one contextual `h` tag was supplied.
    DuplicateSimpleGroupContext,
    /// The supplied contextual `h` value names another simple group.
    ConflictingSimpleGroupContext,
    /// A contextual `h` value exceeds the opaque id byte bound.
    SimpleGroupContextTooLong {
        /// Actual context length in bytes.
        bytes: usize,
        /// Maximum supported context length in bytes.
        maximum: usize,
    },
    /// Event tag input exceeds the preparation bound.
    TooManyContextTags {
        /// Total tags observed before refusal.
        actual: usize,
        /// Maximum supported tag count.
        maximum: usize,
    },
    /// A parser received an event kind other than its exact protocol kind.
    WrongRecordKind {
        /// Required event kind.
        expected: u16,
        /// Supplied event kind.
        actual: u16,
    },
    /// Relay-authored records must carry a signature.
    UnsignedRecord,
    /// A signed record's id does not match its body.
    InvalidRecordId,
    /// A signed record's signature does not verify against its id and author.
    InvalidRecordSignature,
    /// An addressable simple group record has no `d` row.
    MissingRecordId,
    /// An addressable simple group record has an empty `d` value.
    EmptyRecordId,
    /// An addressable simple group record repeats its `d` row.
    DuplicateRecordId,
    /// An addressable simple group record carries contradictory `d` values.
    ConflictingRecordId,
    /// Record tag input exceeds the parser's structural bound.
    TooManyRecordTags {
        /// Tags observed before refusal.
        actual: usize,
        /// Maximum accepted tag count.
        maximum: usize,
    },
    /// A record exceeds the parser's aggregate byte bound.
    RecordTooLarge {
        /// Bytes observed before refusal.
        bytes: usize,
        /// Maximum accepted aggregate bytes.
        maximum: usize,
    },
    /// A single tag carries too many values.
    TooManyRecordTagValues {
        /// Source tag index.
        tag_index: usize,
        /// Values observed before refusal.
        actual: usize,
        /// Maximum accepted value count.
        maximum: usize,
    },
    /// A tag value exceeds the parser's string bound.
    RecordValueTooLong {
        /// Source tag index.
        tag_index: usize,
        /// Source value index within the tag.
        value_index: usize,
        /// Value length in bytes.
        bytes: usize,
        /// Maximum accepted string bytes.
        maximum: usize,
    },
    /// A singleton record field was repeated and cannot be selected safely.
    AmbiguousRecordField(&'static str),
    /// One recognized source row is malformed without retaining hostile input.
    MalformedRecordRow {
        /// Source tag index.
        tag_index: usize,
        /// Stable bounded reason.
        reason: &'static str,
    },
    /// One recognized source row repeats an already accepted exact target.
    DuplicateRecordRow {
        /// Source tag index.
        tag_index: usize,
    },
    /// Discovery input or snapshot size exceeds its pure-operation bound.
    TooManyDiscoveryItems {
        /// Total items observed before refusal.
        actual: usize,
        /// Maximum accepted item count.
        maximum: usize,
    },
}

impl fmt::Display for SimpleGroupError {
    #[allow(clippy::too_many_lines)] // One closed error owner keeps every typed refusal attributable.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHost(error) => write!(formatter, "invalid simple group host: {error}"),
            Self::EmptyHosts => formatter.write_str("a simple group requires at least one host"),
            Self::DuplicateHost { relay } => {
                write!(
                    formatter,
                    "simple group host set repeats relay identity {relay}"
                )
            }
            Self::TooManyHosts { actual, maximum } => {
                write!(
                    formatter,
                    "simple group host count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::EmptyId => formatter.write_str("a simple group id may not be empty"),
            Self::SimpleGroupIdTooLong { bytes, maximum } => {
                write!(
                    formatter,
                    "simple group id bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::Query(error) => write!(formatter, "simple group query refused: {error}"),
            Self::Event(error) => write!(formatter, "simple group event refused: {error}"),
            Self::EmptySimpleGroupContext => {
                formatter.write_str("simple group context has no value")
            }
            Self::MissingSimpleGroupContext => {
                formatter.write_str("signed event has no simple group context")
            }
            Self::DuplicateSimpleGroupContext => {
                formatter.write_str("simple group context is duplicated")
            }
            Self::ConflictingSimpleGroupContext => {
                formatter.write_str("simple group context names another simple group")
            }
            Self::SimpleGroupContextTooLong { bytes, maximum } => {
                write!(
                    formatter,
                    "simple group context bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::TooManyContextTags { actual, maximum } => {
                write!(
                    formatter,
                    "simple group event tag count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::WrongRecordKind { expected, actual } => {
                write!(
                    formatter,
                    "wrong simple group record kind: {actual}, expected {expected}"
                )
            }
            Self::UnsignedRecord => formatter.write_str("simple group records must be signed"),
            Self::InvalidRecordId => formatter.write_str("simple group record id is invalid"),
            Self::InvalidRecordSignature => {
                formatter.write_str("simple group record signature is invalid")
            }
            Self::MissingRecordId => formatter.write_str("simple group record has no d row"),
            Self::EmptyRecordId => formatter.write_str("simple group record d value is empty"),
            Self::DuplicateRecordId => {
                formatter.write_str("simple group record d row is duplicated")
            }
            Self::ConflictingRecordId => {
                formatter.write_str("simple group record d rows are contradictory")
            }
            Self::TooManyRecordTags { actual, maximum } => {
                write!(
                    formatter,
                    "simple group record tag count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::RecordTooLarge { bytes, maximum } => {
                write!(
                    formatter,
                    "simple group record bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::TooManyRecordTagValues {
                tag_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "simple group record tag {tag_index} value count exceeds bound: {actual} > {maximum}"
            ),
            Self::RecordValueTooLong {
                tag_index,
                value_index,
                bytes,
                maximum,
            } => write!(
                formatter,
                "simple group record tag {tag_index} value {value_index} bytes exceed bound: {bytes} > {maximum}"
            ),
            Self::AmbiguousRecordField(field) => {
                write!(formatter, "simple group record field {field} is ambiguous")
            }
            Self::MalformedRecordRow { tag_index, reason } => {
                write!(
                    formatter,
                    "simple group record row {tag_index} is malformed: {reason}"
                )
            }
            Self::DuplicateRecordRow { tag_index } => {
                write!(
                    formatter,
                    "simple group record row {tag_index} repeats an accepted target"
                )
            }
            Self::TooManyDiscoveryItems { actual, maximum } => write!(
                formatter,
                "simple-group input or snapshot item count exceeds bound: {actual} > {maximum}"
            ),
        }
    }
}

impl std::error::Error for SimpleGroupError {}

impl From<QueryError> for SimpleGroupError {
    fn from(error: QueryError) -> Self {
        Self::Query(error.to_string())
    }
}

impl From<EventBuildError> for SimpleGroupError {
    fn from(error: EventBuildError) -> Self {
        Self::Event(error.to_string())
    }
}

impl From<WriteIntentError> for SimpleGroupError {
    fn from(error: WriteIntentError) -> Self {
        match error {
            WriteIntentError::EmptyExplicitRelays => Self::EmptyHosts,
            WriteIntentError::TooManyExplicitRelays { actual, maximum } => {
                Self::TooManyHosts { actual, maximum }
            }
            // The third relay-route refusal. Typing its two siblings and
            // letting this one fall through to `Event` reports a bad host set
            // as a malformed event.
            WriteIntentError::DuplicateExplicitRelay { relay } => Self::DuplicateHost { relay },
            other => Self::Event(other.to_string()),
        }
    }
}

impl SimpleGroup {
    /// Construct one simple group over one or several host relays through the same bounded input.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when a host is invalid or the host set is empty or oversized.
    #[allow(private_bounds)]
    pub fn on<I>(hosts: I, id: impl Into<String>) -> Result<Self, SimpleGroupError>
    where
        I: IntoIterator,
        I::Item: IntoRelayUrl,
    {
        let id = id.into();
        if id.is_empty() {
            return Err(SimpleGroupError::EmptyId);
        }
        if id.len() > MAX_SIMPLE_GROUP_ID_BYTES {
            return Err(SimpleGroupError::SimpleGroupIdTooLong {
                bytes: id.len(),
                maximum: MAX_SIMPLE_GROUP_ID_BYTES,
            });
        }
        let inputs = collect_at_most(hosts, MAX_SIMPLE_GROUP_HOST_INPUT_ITEMS).map_err(
            |actual| SimpleGroupError::TooManyHosts {
                actual,
                maximum: MAX_SIMPLE_GROUP_HOST_INPUT_ITEMS,
            },
        )?;
        let parsed = inputs
            .into_iter()
            .map(IntoRelayUrl::into_relay_url)
            .collect::<Result<Vec<_>, _>>()?;
        if parsed.is_empty() {
            return Err(SimpleGroupError::EmptyHosts);
        }
        let mut seen = BTreeSet::new();
        let hosts = parsed
            .into_iter()
            .filter(|host| seen.insert(host.clone()))
            .collect();
        Ok(Self { hosts, id })
    }

    /// Return the opaque simple group id exactly as supplied.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Iterate over the complete normalized host route.
    pub fn hosts(&self) -> impl Iterator<Item = RelayUrl> + '_ {
        self.hosts.iter().cloned()
    }

    /// Prepare an unsigned or signed event without opening publication work.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the event cannot carry this exact simple group context.
    #[allow(private_bounds)]
    pub fn prepare<P>(&self, payload: P) -> Result<P, SimpleGroupError>
    where
        P: PreparePayload,
    {
        payload.prepare_for(self)
    }

    /// Add simple group content selection to an ordinary query.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when exact host acquisition cannot be represented.
    pub fn events(&self, selection: Query) -> Result<Query, SimpleGroupError> {
        crate::query::content(self, selection)
    }

    /// Construct an ordinary query for relay-authored simple group records.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when exact host authority cannot be represented.
    pub fn records(&self, records: SimpleGroupRecords) -> Result<Query, SimpleGroupError> {
        crate::query::records(self, records)
    }

    /// Project one immutable ordinary query snapshot into this simple group's exact host views.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the snapshot exceeds the projection bound.
    pub fn project(
        &self,
        snapshot: &QuerySnapshot,
    ) -> Result<crate::SimpleGroupSnapshot, SimpleGroupError> {
        crate::SimpleGroupSnapshot::project(self, snapshot)
    }
}

trait IntoRelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, SimpleGroupError>;
}

impl IntoRelayUrl for RelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, SimpleGroupError> {
        Ok(self)
    }
}

impl IntoRelayUrl for &RelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, SimpleGroupError> {
        Ok(self.clone())
    }
}

impl IntoRelayUrl for &str {
    fn into_relay_url(self) -> Result<RelayUrl, SimpleGroupError> {
        RelayUrl::parse(self).map_err(|error| SimpleGroupError::InvalidHost(error.to_string()))
    }
}

impl IntoRelayUrl for String {
    fn into_relay_url(self) -> Result<RelayUrl, SimpleGroupError> {
        RelayUrl::parse(&self).map_err(|error| SimpleGroupError::InvalidHost(error.to_string()))
    }
}

trait PreparePayload: Sized {
    fn prepare_for(self, simple_group: &SimpleGroup) -> Result<Self, SimpleGroupError>;
}

impl PreparePayload for UnsignedEvent {
    fn prepare_for(self, simple_group: &SimpleGroup) -> Result<Self, SimpleGroupError> {
        validate_context_input_bound(self.tags.iter())?;
        let context = Tag::parse(["h", simple_group.id()])
            .map_err(|error| SimpleGroupError::Event(error.to_string()))?;
        let mut matching_contexts = 0usize;
        let mut tags = Vec::with_capacity(self.tags.len().saturating_add(1));
        for tag in self.tags.iter() {
            if is_simple_group_context(tag) {
                validate_simple_group_context(tag, simple_group.id())?;
                matching_contexts += 1;
                if matching_contexts == 1 {
                    tags.push(tag.clone());
                }
            } else {
                tags.push(tag.clone());
            }
        }
        if matching_contexts == 1 {
            return Ok(self);
        }
        if matching_contexts == 0 {
            tags.push(context);
        }
        EventBuilder::from_parts(self.pubkey, self.kind, self.created_at, tags, self.content)
            .build()
            .map_err(Into::into)
    }
}

impl PreparePayload for Event {
    fn prepare_for(self, simple_group: &SimpleGroup) -> Result<Self, SimpleGroupError> {
        self.verify()
            .map_err(|error| SimpleGroupError::Event(error.to_string()))?;
        validate_context_input_bound(self.tags.iter())?;
        let mut contexts = 0usize;
        for tag in self.tags.iter().filter(|tag| is_simple_group_context(tag)) {
            contexts += 1;
            if contexts > 1 {
                return Err(SimpleGroupError::DuplicateSimpleGroupContext);
            }
            validate_simple_group_context(tag, simple_group.id())?;
        }
        if contexts == 0 {
            return Err(SimpleGroupError::MissingSimpleGroupContext);
        }
        Ok(self)
    }
}

fn validate_context_input_bound<'a>(
    tags: impl IntoIterator<Item = &'a Tag>,
) -> Result<(), SimpleGroupError> {
    let actual = tags
        .into_iter()
        .take(MAX_SIMPLE_GROUP_CONTEXT_INPUT_ITEMS.saturating_add(1))
        .count();
    if actual > MAX_SIMPLE_GROUP_CONTEXT_INPUT_ITEMS {
        Err(SimpleGroupError::TooManyContextTags {
            actual,
            maximum: MAX_SIMPLE_GROUP_CONTEXT_INPUT_ITEMS,
        })
    } else {
        Ok(())
    }
}

fn is_simple_group_context(tag: &Tag) -> bool {
    tag.as_slice().first().map(String::as_str) == Some("h")
}

fn validate_simple_group_context(tag: &Tag, simple_group_id: &str) -> Result<(), SimpleGroupError> {
    let values = tag.as_slice();
    let value = values
        .get(1)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(SimpleGroupError::EmptySimpleGroupContext)?;
    if value.len() > MAX_SIMPLE_GROUP_ID_BYTES {
        return Err(SimpleGroupError::SimpleGroupContextTooLong {
            bytes: value.len(),
            maximum: MAX_SIMPLE_GROUP_ID_BYTES,
        });
    }
    if values.len() != 2 || value != simple_group_id {
        return Err(SimpleGroupError::ConflictingSimpleGroupContext);
    }
    Ok(())
}
