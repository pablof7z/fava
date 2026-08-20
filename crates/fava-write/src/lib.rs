//! Event values and publication evidence shared by write owners and queries.

use fava_state::{EventCoordinate, event_coordinate};
pub use nostr::event::{Event, EventId, Kind, Tag, UnsignedEvent};
pub use nostr::key::PublicKey;
pub use nostr::types::Timestamp;
use thiserror::Error;

/// Stable identity of one accepted write.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReceiptId(u64);

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
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureState {
    /// Exact unsigned event exists.
    Unsigned,
    /// Exact signed event exists.
    Signed,
}

/// Local evidence attached to an event record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationEvidence {
    /// Reattachable receipt identity.
    pub receipt_id: ReceiptId,
    /// Accepted write identity.
    pub write_id: WriteId,
    /// Current signing fact.
    pub signature: SignatureState,
}

/// One current event contribution from the write store.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalWriteEvent {
    id: EventId,
    /// Current unsigned or signed materialization.
    pub event: EventValue,
    /// Exact local publication evidence.
    pub publication: PublicationEvidence,
}

impl LocalWriteEvent {
    /// Validate and construct a query-visible local event.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEventValue::MissingId`] when an unsigned body was not
    /// finalized with its deterministic event id.
    pub fn new(
        event: EventValue,
        publication: PublicationEvidence,
    ) -> Result<Self, InvalidEventValue> {
        let id = event.id().ok_or(InvalidEventValue::MissingId)?;
        Ok(Self {
            id,
            event,
            publication,
        })
    }

    /// Stable event id.
    #[must_use]
    pub const fn id(&self) -> EventId {
        self.id
    }
}

/// Refusal for an event body that cannot enter accepted local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InvalidEventValue {
    /// An unsigned body was not finalized with its deterministic id.
    #[error("unsigned event has no computed event id")]
    MissingId,
}
