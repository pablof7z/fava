use std::collections::BTreeSet;
use std::fmt;

use fava_query::{Query, QueryError, QuerySnapshot};
use fava_state::RelayUrl;
use fava_write::{Event, EventBuildError, EventBuilder, Tag, UnsignedEvent, WriteIntentError};

use crate::GroupRecords;
use crate::bounds::{
    MAX_GROUP_CONTEXT_INPUT_ITEMS, MAX_GROUP_HOST_INPUT_ITEMS, MAX_GROUP_ID_BYTES, collect_at_most,
};

/// One opaque NIP-29 group id over an application-selected host set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group {
    hosts: Vec<RelayUrl>,
    id: String,
}

/// Typed refusal while constructing or transforming a group value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GroupError {
    /// A host value is not a valid relay URL.
    InvalidHost(String),
    /// A group requires at least one host relay.
    EmptyHosts,
    /// Host input exceeds the shared explicit-route bound.
    TooManyHosts {
        /// Total host inputs observed before refusal.
        actual: usize,
        /// Maximum supported distinct host count.
        maximum: usize,
    },
    /// An opaque group id may not be empty.
    EmptyId,
    /// The opaque group id exceeds its byte bound.
    GroupIdTooLong {
        /// Actual id length in bytes.
        bytes: usize,
        /// Maximum supported id length in bytes.
        maximum: usize,
    },
    /// A group query was refused before work opened.
    Query(String),
    /// Event preparation was refused before custody.
    Event(String),
    /// A contextual `h` tag is missing its value.
    EmptyGroupContext,
    /// A signed event has no contextual `h` tag.
    MissingGroupContext,
    /// More than one contextual `h` tag was supplied.
    DuplicateGroupContext,
    /// The supplied contextual `h` value names another group.
    ConflictingGroupContext,
    /// A contextual `h` value exceeds the opaque id byte bound.
    GroupContextTooLong {
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
    /// An addressable group record has no `d` row.
    MissingRecordId,
    /// An addressable group record has an empty `d` value.
    EmptyRecordId,
    /// An addressable group record repeats its `d` row.
    DuplicateRecordId,
    /// An addressable group record carries contradictory `d` values.
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

impl fmt::Display for GroupError {
    #[allow(clippy::too_many_lines)] // One closed error owner keeps every typed refusal attributable.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHost(error) => write!(formatter, "invalid group host: {error}"),
            Self::EmptyHosts => formatter.write_str("a group requires at least one host"),
            Self::TooManyHosts { actual, maximum } => {
                write!(
                    formatter,
                    "group host count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::EmptyId => formatter.write_str("a group id may not be empty"),
            Self::GroupIdTooLong { bytes, maximum } => {
                write!(
                    formatter,
                    "group id bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::Query(error) => write!(formatter, "group query refused: {error}"),
            Self::Event(error) => write!(formatter, "group event refused: {error}"),
            Self::EmptyGroupContext => formatter.write_str("group context has no value"),
            Self::MissingGroupContext => formatter.write_str("signed event has no group context"),
            Self::DuplicateGroupContext => formatter.write_str("group context is duplicated"),
            Self::ConflictingGroupContext => {
                formatter.write_str("group context names another group")
            }
            Self::GroupContextTooLong { bytes, maximum } => {
                write!(
                    formatter,
                    "group context bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::TooManyContextTags { actual, maximum } => {
                write!(
                    formatter,
                    "group event tag count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::WrongRecordKind { expected, actual } => {
                write!(
                    formatter,
                    "wrong group record kind: {actual}, expected {expected}"
                )
            }
            Self::UnsignedRecord => formatter.write_str("group records must be signed"),
            Self::InvalidRecordId => formatter.write_str("group record id is invalid"),
            Self::InvalidRecordSignature => {
                formatter.write_str("group record signature is invalid")
            }
            Self::MissingRecordId => formatter.write_str("group record has no d row"),
            Self::EmptyRecordId => formatter.write_str("group record d value is empty"),
            Self::DuplicateRecordId => formatter.write_str("group record d row is duplicated"),
            Self::ConflictingRecordId => {
                formatter.write_str("group record d rows are contradictory")
            }
            Self::TooManyRecordTags { actual, maximum } => {
                write!(
                    formatter,
                    "group record tag count exceeds bound: {actual} > {maximum}"
                )
            }
            Self::RecordTooLarge { bytes, maximum } => {
                write!(
                    formatter,
                    "group record bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::TooManyRecordTagValues {
                tag_index,
                actual,
                maximum,
            } => write!(
                formatter,
                "group record tag {tag_index} value count exceeds bound: {actual} > {maximum}"
            ),
            Self::RecordValueTooLong {
                tag_index,
                value_index,
                bytes,
                maximum,
            } => write!(
                formatter,
                "group record tag {tag_index} value {value_index} bytes exceed bound: {bytes} > {maximum}"
            ),
            Self::AmbiguousRecordField(field) => {
                write!(formatter, "group record field {field} is ambiguous")
            }
            Self::MalformedRecordRow { tag_index, reason } => {
                write!(
                    formatter,
                    "group record row {tag_index} is malformed: {reason}"
                )
            }
            Self::DuplicateRecordRow { tag_index } => {
                write!(
                    formatter,
                    "group record row {tag_index} repeats an accepted target"
                )
            }
            Self::TooManyDiscoveryItems { actual, maximum } => write!(
                formatter,
                "simple-group input or snapshot item count exceeds bound: {actual} > {maximum}"
            ),
        }
    }
}

impl std::error::Error for GroupError {}

impl From<QueryError> for GroupError {
    fn from(error: QueryError) -> Self {
        Self::Query(error.to_string())
    }
}

impl From<EventBuildError> for GroupError {
    fn from(error: EventBuildError) -> Self {
        Self::Event(error.to_string())
    }
}

impl From<WriteIntentError> for GroupError {
    fn from(error: WriteIntentError) -> Self {
        match error {
            WriteIntentError::EmptyExplicitRelays => Self::EmptyHosts,
            WriteIntentError::TooManyExplicitRelays { actual, maximum } => {
                Self::TooManyHosts { actual, maximum }
            }
            other => Self::Event(other.to_string()),
        }
    }
}

impl Group {
    /// Construct one group over one or several host relays through the same bounded input.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when a host is invalid or the host set is empty or oversized.
    #[allow(private_bounds)]
    pub fn on<I>(hosts: I, id: impl Into<String>) -> Result<Self, GroupError>
    where
        I: IntoIterator,
        I::Item: IntoRelayUrl,
    {
        let id = id.into();
        if id.is_empty() {
            return Err(GroupError::EmptyId);
        }
        if id.len() > MAX_GROUP_ID_BYTES {
            return Err(GroupError::GroupIdTooLong {
                bytes: id.len(),
                maximum: MAX_GROUP_ID_BYTES,
            });
        }
        let inputs = collect_at_most(hosts, MAX_GROUP_HOST_INPUT_ITEMS).map_err(|actual| {
            GroupError::TooManyHosts {
                actual,
                maximum: MAX_GROUP_HOST_INPUT_ITEMS,
            }
        })?;
        let parsed = inputs
            .into_iter()
            .map(IntoRelayUrl::into_relay_url)
            .collect::<Result<Vec<_>, _>>()?;
        if parsed.is_empty() {
            return Err(GroupError::EmptyHosts);
        }
        let mut seen = BTreeSet::new();
        let hosts = parsed
            .into_iter()
            .filter(|host| seen.insert(host.clone()))
            .collect();
        Ok(Self { hosts, id })
    }

    /// Return the opaque group id exactly as supplied.
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
    /// Returns [`GroupError`] when the event cannot carry this exact group context.
    #[allow(private_bounds)]
    pub fn prepare<P>(&self, payload: P) -> Result<P, GroupError>
    where
        P: PreparePayload,
    {
        payload.prepare_for(self)
    }

    /// Add group content selection to an ordinary query.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when exact host acquisition cannot be represented.
    pub fn events(&self, selection: Query) -> Result<Query, GroupError> {
        crate::query::content(self, selection)
    }

    /// Construct an ordinary query for relay-authored group records.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when exact host authority cannot be represented.
    pub fn records(&self, records: GroupRecords) -> Result<Query, GroupError> {
        crate::query::records(self, records)
    }

    /// Project one immutable ordinary query snapshot into this group's exact host views.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the snapshot exceeds the projection bound.
    pub fn project(&self, snapshot: &QuerySnapshot) -> Result<crate::GroupSnapshot, GroupError> {
        crate::GroupSnapshot::project(self, snapshot)
    }
}

trait IntoRelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, GroupError>;
}

impl IntoRelayUrl for RelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, GroupError> {
        Ok(self)
    }
}

impl IntoRelayUrl for &RelayUrl {
    fn into_relay_url(self) -> Result<RelayUrl, GroupError> {
        Ok(self.clone())
    }
}

impl IntoRelayUrl for &str {
    fn into_relay_url(self) -> Result<RelayUrl, GroupError> {
        RelayUrl::parse(self).map_err(|error| GroupError::InvalidHost(error.to_string()))
    }
}

impl IntoRelayUrl for String {
    fn into_relay_url(self) -> Result<RelayUrl, GroupError> {
        RelayUrl::parse(&self).map_err(|error| GroupError::InvalidHost(error.to_string()))
    }
}

trait PreparePayload: Sized {
    fn prepare_for(self, group: &Group) -> Result<Self, GroupError>;
}

impl PreparePayload for UnsignedEvent {
    fn prepare_for(self, group: &Group) -> Result<Self, GroupError> {
        validate_context_input_bound(self.tags.iter())?;
        let context =
            Tag::parse(["h", group.id()]).map_err(|error| GroupError::Event(error.to_string()))?;
        let mut matching_contexts = 0usize;
        let mut tags = Vec::with_capacity(self.tags.len().saturating_add(1));
        for tag in self.tags.iter() {
            if is_group_context(tag) {
                validate_group_context(tag, group.id())?;
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
    fn prepare_for(self, group: &Group) -> Result<Self, GroupError> {
        self.verify()
            .map_err(|error| GroupError::Event(error.to_string()))?;
        validate_context_input_bound(self.tags.iter())?;
        let mut contexts = 0usize;
        for tag in self.tags.iter().filter(|tag| is_group_context(tag)) {
            contexts += 1;
            if contexts > 1 {
                return Err(GroupError::DuplicateGroupContext);
            }
            validate_group_context(tag, group.id())?;
        }
        if contexts == 0 {
            return Err(GroupError::MissingGroupContext);
        }
        Ok(self)
    }
}

fn validate_context_input_bound<'a>(
    tags: impl IntoIterator<Item = &'a Tag>,
) -> Result<(), GroupError> {
    let actual = tags
        .into_iter()
        .take(MAX_GROUP_CONTEXT_INPUT_ITEMS.saturating_add(1))
        .count();
    if actual > MAX_GROUP_CONTEXT_INPUT_ITEMS {
        Err(GroupError::TooManyContextTags {
            actual,
            maximum: MAX_GROUP_CONTEXT_INPUT_ITEMS,
        })
    } else {
        Ok(())
    }
}

fn is_group_context(tag: &Tag) -> bool {
    tag.as_slice().first().map(String::as_str) == Some("h")
}

fn validate_group_context(tag: &Tag, group_id: &str) -> Result<(), GroupError> {
    let values = tag.as_slice();
    let value = values
        .get(1)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(GroupError::EmptyGroupContext)?;
    if value.len() > MAX_GROUP_ID_BYTES {
        return Err(GroupError::GroupContextTooLong {
            bytes: value.len(),
            maximum: MAX_GROUP_ID_BYTES,
        });
    }
    if values.len() != 2 || value != group_id {
        return Err(GroupError::ConflictingGroupContext);
    }
    Ok(())
}
