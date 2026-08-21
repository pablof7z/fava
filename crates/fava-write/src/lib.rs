//! Event values and publication evidence shared by write owners and queries.

use std::collections::BTreeSet;

use fava_state::{EventCoordinate, RelayAccess, RelayUrl, event_coordinate};
pub use nostr::event::{Event, EventId, Kind, Tag, UnsignedEvent};
pub use nostr::key::PublicKey;
pub use nostr::types::Timestamp;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod attempt_map;
mod builder;
mod delivery;
mod delivery_map;
mod edit;
mod materialization;
mod session_set;

pub use builder::{EventBuildError, EventBuilder};
pub use delivery::{
    LocalWriteEvent, PublicationEvidence, Receipt, ReceiptOutcome, RelayDeliveryOutcome,
};
pub use edit::ReplaceableEventEdit;
pub use materialization::{MaterializationId, ReplaceableEventMaterializer};

pub(crate) const MAX_EVENT_BYTES: usize = 131_072;
const MAX_EXPLICIT_RELAYS: usize = 256;

/// Stable identity of one accepted write.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct WriteId(u64);

impl WriteId {
    /// Construct an id allocated by a write store.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Stable, reattachable identity of one write receipt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ReceiptId(u64);

/// Event form accepted by the publication lifecycle in the current milestone.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WritePayload {
    /// Complete unsigned event body.
    Event(UnsignedEvent),
    /// Persistable protocol-owned change awaiting or surviving materialization.
    Edit {
        /// Durable protocol-owned change.
        edit: ReplaceableEventEdit,
        /// Author resolved exactly once before custody.
        author: PublicKey,
    },
    /// Verified complete signed event.
    Presigned(Event),
}

/// Relay selection for one publication obligation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WriteRouting {
    /// Use the configured ordered router chain.
    Automatic,
    /// Use exactly this relay set and open no automatic router.
    Explicit(BTreeSet<RelayUrl>),
}

/// Application request to accept responsibility for publishing one event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WriteIntent {
    payload: WritePayload,
    routing: WriteRouting,
    #[serde(default)]
    access: RelayAccess,
}

impl WriteIntent {
    /// Validate one unsigned event and its route before custody.
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
        validate_routing(&routing)?;
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
            access: RelayAccess::public(),
        })
    }

    /// Validate one signed event and its route before custody.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for an invalid, expired, or unroutable event.
    pub fn presigned(event: Event, routing: WriteRouting) -> Result<Self, WriteIntentError> {
        event
            .verify()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
        validate_routing(&routing)?;
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
            access: RelayAccess::public(),
        })
    }

    /// Select the exact relay access under which destinations execute.
    ///
    /// Relay access is authorization identity, not authorship. Two accounts
    /// publishing to one relay occupy two relay sessions, so denying one
    /// cannot terminate the other.
    #[must_use]
    pub fn with_relay_access(mut self, access: RelayAccess) -> Self {
        self.access = access;
        self
    }

    /// Selected relay access.
    #[must_use]
    pub const fn access(&self) -> &RelayAccess {
        &self.access
    }

    /// Accepted event form.
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

/// Refusal before durable write custody.
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
    /// Event id or signature is invalid.
    #[error("event verification failed: {0}")]
    InvalidEvent(String),
    /// Complete event is already expired.
    #[error("event is already expired")]
    Expired,
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
}

fn validate_routing(routing: &WriteRouting) -> Result<(), WriteIntentError> {
    if let WriteRouting::Explicit(relays) = routing {
        if relays.is_empty() {
            return Err(WriteIntentError::EmptyExplicitRelays);
        }
        if relays.len() > MAX_EXPLICIT_RELAYS {
            return Err(WriteIntentError::TooManyExplicitRelays {
                actual: relays.len(),
                maximum: MAX_EXPLICIT_RELAYS,
            });
        }
    }
    Ok(())
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
    /// Construct an id allocated by a write store.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Event body currently supplied by the write store.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EventValue {
    /// Materialized event awaiting a valid signature.
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

/// Signing fact for the current local materialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignatureState {
    /// Exact unsigned event exists.
    Unsigned,
    /// Exact signed event exists.
    Signed,
    /// Signer refused or produced invalid output.
    Refused(String),
}

/// Refusal for an event body that cannot enter accepted local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InvalidEventValue {
    /// An unsigned body was not finalized with its deterministic id.
    #[error("unsigned event has no computed event id")]
    MissingId,
}
