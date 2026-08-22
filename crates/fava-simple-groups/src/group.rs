use std::collections::BTreeSet;
use std::fmt;

use fava_query::{Query, QueryError};
use fava_state::RelayUrl;
use fava_write::{Event, EventBuildError, EventBuilder, Tag, UnsignedEvent, WriteIntentError};

use crate::GroupRecords;
use crate::bounds::{MAX_GROUP_HOST_INPUT_ITEMS, MAX_GROUP_ID_BYTES, collect_at_most};

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
}

impl fmt::Display for GroupError {
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
        let context =
            Tag::parse(["h", group.id()]).map_err(|error| GroupError::Event(error.to_string()))?;
        EventBuilder::from_parts(
            self.pubkey,
            self.kind,
            self.created_at,
            self.tags.iter().cloned().chain([context]).collect(),
            self.content,
        )
        .build()
        .map_err(Into::into)
    }
}

impl PreparePayload for Event {
    fn prepare_for(self, _group: &Group) -> Result<Self, GroupError> {
        Ok(self)
    }
}
