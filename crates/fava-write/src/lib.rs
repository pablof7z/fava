//! Event values and publication evidence shared by write owners and queries.

use fava_relay::RelayAccess;
use fava_state::{EventCoordinate, event_coordinate};
pub use nostr::event::{Event, EventId, Kind, Tag, UnsignedEvent};
pub use nostr::key::PublicKey;
use nostr::types::RelayUrl;
pub use nostr::types::Timestamp;
use serde::{Deserialize, Serialize};
use std::num::{NonZeroU64, TryFromIntError};
use thiserror::Error;

mod attempt_map;
mod builder;
mod delivery_map;
mod edit;
mod edit_application;
mod receipt;
mod relay_session_serde;
mod routing;
mod session_set;

pub use builder::{AuthoredEventBuilder, EventBuildError, EventBuilder};
pub use edit::EventEdit;
pub use edit_application::{EditApplier, EditApplierSink, RevisionId};
pub use receipt::{
    LocalWriteEvent, PublicationEvidence, Receipt, ReceiptOutcome, RelayDeliveryOutcome,
    SignatureState,
};
pub use routing::WriteRouting;

pub(crate) const MAX_EVENT_BYTES: usize = 131_072;

/// Stable identity of one accepted write.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WriteId(NonZeroU64);

impl WriteId {
    /// Reconstruct a nonzero identity value.
    ///
    /// Construction does not allocate durable write custody. A write store
    /// mints the identity only when it commits the accepted write.
    #[must_use]
    pub const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for WriteId {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::try_from(value).map(Self)
    }
}

/// Stable, reattachable identity of one write receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ReceiptId(NonZeroU64);

/// Which of three admission paths a write takes: an unsigned body, a durable
/// protocol-owned edit, or an already-signed event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WritePayload {
    /// Complete unsigned event body.
    Event(UnsignedEvent),
    /// Persistable protocol-owned change awaiting or surviving revision.
    Edit {
        /// Durable protocol-owned change.
        edit: EventEdit,
        /// Author resolved exactly once before custody.
        author: PublicKey,
    },
    /// Verified complete signed event.
    Presigned(Event),
}

/// Application request to accept responsibility for publishing one event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WriteIntent {
    payload: WritePayload,
    routing: WriteRouting,
    /// The relay authority this write is accepted under. Separate from the
    /// event's author: a write may go over one account's authenticated session
    /// and be signed by another.
    access: RelayAccess,
}

impl WriteIntent {
    /// The relay authority this write is accepted under.
    #[must_use]
    pub const fn access(&self) -> &RelayAccess {
        &self.access
    }

    /// Accept this write under one account's relay authority.
    ///
    /// Separate from the event's author, which the payload already carries.
    #[must_use]
    pub fn under(mut self, access: RelayAccess) -> Self {
        self.access = access;
        self
    }

    /// Validate one unsigned event and its route before custody.
    ///
    /// # Arguments
    ///
    /// * `event` - the unsigned event; its id is computed if absent
    /// * `routing` - the relay-routing mode to publish it under
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for an invalid, expired, or unroutable event.
    pub fn event(
        mut event: UnsignedEvent,
        routing: WriteRouting,
    ) -> Result<Self, WriteIntentError> {
        event.ensure_id();
        event
            .verify_id()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
        routing.validate()?;
        validate_event_size(&event)?;
        if event
            .tags
            .expiration()
            .is_some_and(|expiry| expiry <= Timestamp::now())
        {
            return Err(WriteIntentError::Expired);
        }
        Ok(Self {
            payload: WritePayload::Event(event),
            routing,
            access: RelayAccess::Public,
        })
    }

    /// Validate one signed event and its route before custody.
    ///
    /// # Arguments
    ///
    /// * `event` - the already-signed event to verify
    /// * `routing` - the relay-routing mode to publish it under
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for an invalid, expired, or unroutable event.
    pub fn presigned(event: Event, routing: WriteRouting) -> Result<Self, WriteIntentError> {
        event
            .verify()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
        routing.validate()?;
        validate_event_size(&event)?;
        if event
            .tags
            .expiration()
            .is_some_and(|expiry| expiry <= Timestamp::now())
        {
            return Err(WriteIntentError::Expired);
        }
        Ok(Self {
            payload: WritePayload::Presigned(event),
            routing,
            access: RelayAccess::Public,
        })
    }

    /// Which of the three admission paths this intent takes.
    #[must_use]
    pub const fn payload(&self) -> &WritePayload {
        &self.payload
    }

    /// Selected relay-routing mode.
    #[must_use]
    pub const fn routing(&self) -> &WriteRouting {
        &self.routing
    }

    /// Exact event author, including the author resolved for an edit.
    #[must_use]
    pub fn author(&self) -> PublicKey {
        match &self.payload {
            WritePayload::Event(event) => event.pubkey,
            WritePayload::Edit { author, .. } => *author,
            WritePayload::Presigned(event) => event.pubkey,
        }
    }

    /// Consume the intent into its exact parts.
    #[must_use]
    pub fn into_parts(self) -> (WritePayload, WriteRouting, RelayAccess) {
        (self.payload, self.routing, self.access)
    }
}

/// Refusal while validating or applying a write intent.
///
/// Intent validation can return this typed value directly before durable
/// custody. Appliers can return it during initial or post-custody
/// revision, but current publication converts either result to
/// `PublicationError::Routing(error.to_string())`; this typed value does not
/// survive that boundary. Issue 0025 owns structured lifecycle attribution.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum WriteIntentError {
    /// Explicit publication requires at least one relay.
    #[error("explicit publication requires at least one relay")]
    EmptyExplicitRelays,
    /// Explicit relay fan-out exceeds the declared bound.
    #[error("explicit relay count exceeds bound: {actual} > {maximum}")]
    TooManyExplicitRelays {
        /// Actual relay count.
        actual: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Raw explicit relay input exceeds the pre-normalization work bound.
    #[error("raw explicit relay input exceeds bound: {actual} > {maximum}")]
    TooManyRawExplicitRelays {
        /// Raw input count before duplicate normalization.
        actual: usize,
        /// Declared raw-input maximum.
        maximum: usize,
    },
    /// A deserialized or reconstructed explicit route contains a duplicate.
    #[error("explicit publication route repeats relay identity {relay}")]
    DuplicateExplicitRelay {
        /// Repeated exact relay identity.
        relay: RelayUrl,
    },
    /// Event id or signature is invalid.
    #[error("event verification failed: {0}")]
    InvalidEvent(String),
    /// Complete event is already expired.
    #[error("event is already expired")]
    Expired,
    /// Event contains too many tags.
    #[error("event tags exceed bound: {actual} > {maximum}")]
    TooManyTags {
        /// Actual tag count.
        actual: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Event exceeds the declared byte bound.
    #[error("event bytes exceed bound: {bytes} > {maximum}")]
    TooLarge {
        /// Actual serialized bytes.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Exact serialization failed.
    #[error("event encoding failed: {0}")]
    Encoding(String),
    /// Event-only construction would discard local explicit routing.
    #[error("event-only construction cannot discard explicit publication routing")]
    ExplicitRoutingAttached,
    /// Builder and facade both selected an explicit publication route.
    #[error("builder and facade cannot both select an explicit publication route")]
    ConflictingExplicitRoutes,
}

impl From<EventBuildError> for WriteIntentError {
    fn from(error: EventBuildError) -> Self {
        match error {
            EventBuildError::TooManyTags { actual, maximum } => {
                Self::TooManyTags { actual, maximum }
            }
            EventBuildError::TooLarge { bytes, maximum } => Self::TooLarge { bytes, maximum },
            EventBuildError::Encoding(reason) => Self::Encoding(reason),
            EventBuildError::ExplicitRoutingAttached => Self::ExplicitRoutingAttached,
        }
    }
}

fn validate_event_size(event: &impl Serialize) -> Result<(), WriteIntentError> {
    let bytes = serde_json::to_vec(event)
        .map_err(|error| WriteIntentError::Encoding(error.to_string()))?
        .len();
    if bytes > MAX_EVENT_BYTES {
        Err(WriteIntentError::TooLarge {
            bytes,
            maximum: MAX_EVENT_BYTES,
        })
    } else {
        Ok(())
    }
}

impl ReceiptId {
    /// Reconstruct a nonzero reattachable receipt identity.
    ///
    /// Construction permits durable import and lookup. It does not allocate a
    /// receipt or authorize a write-store mutation.
    #[must_use]
    pub const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for ReceiptId {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::try_from(value).map(Self)
    }
}

/// Whether the write store currently holds a signed event or an unsigned body
/// still waiting for one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EventValue {
    /// Applied event awaiting a valid signature.
    Unsigned(UnsignedEvent),
    /// Exact signed event.
    Signed(Event),
}

impl EventValue {
    /// Stable event id when the unsigned body has been completely built.
    #[must_use]
    pub fn id(&self) -> Option<EventId> {
        match self {
            Self::Unsigned(event) => event.id,
            Self::Signed(event) => Some(event.id),
        }
    }

    /// Event author.
    #[must_use]
    pub fn author(&self) -> PublicKey {
        match self {
            Self::Unsigned(event) => event.pubkey,
            Self::Signed(event) => event.pubkey,
        }
    }

    /// Event kind.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Self::Unsigned(event) => event.kind,
            Self::Signed(event) => event.kind,
        }
    }

    /// Event creation timestamp.
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        match self {
            Self::Unsigned(event) => event.created_at,
            Self::Signed(event) => event.created_at,
        }
    }

    /// Event tags.
    #[must_use]
    pub fn tags(&self) -> &[Tag] {
        match self {
            Self::Unsigned(event) => event.tags.as_slice(),
            Self::Signed(event) => event.tags.as_slice(),
        }
    }

    /// Identity of this immutable event or its replaceable-event coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEventValue::MissingId`] when an unsigned body was not
    /// finalized with its deterministic event id.
    pub fn coordinate(&self) -> Result<EventCoordinate, InvalidEventValue> {
        let id = self.id().ok_or(InvalidEventValue::MissingId)?;
        Ok(event_coordinate(
            id,
            self.author(),
            self.kind(),
            self.tags(),
        ))
    }
}

/// Refusal for an event body that cannot enter accepted local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InvalidEventValue {
    /// An unsigned body was not finalized with its deterministic id.
    #[error("unsigned event has no computed event id")]
    MissingId,
}
