//! Event values and publication evidence shared by write owners and queries.

use std::collections::{BTreeMap, BTreeSet};

use fava_state::{EventCoordinate, RelaySessionKey, RelayUrl, event_coordinate};
pub use nostr::event::{Event, EventId, Kind, Tag, UnsignedEvent};
pub use nostr::key::PublicKey;
pub use nostr::types::Timestamp;
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod attempt_map;
mod builder;
mod delivery_map;

pub use builder::{EventBuildError, EventBuilder};

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
        })
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

    /// Consume the intent into its exact parts.
    #[must_use]
    pub fn into_parts(self) -> (WritePayload, WriteRouting) {
        (self.payload, self.routing)
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

/// Exact current publication fact for one destination.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RelayDeliveryOutcome {
    /// Destination is durable but has not crossed a handoff boundary.
    Pending,
    /// One durable attempt may already have handed bytes to transport.
    Attempting,
    /// A definite pre-handoff failure remains eligible for policy retry.
    Retryable {
        /// Exact definite pre-handoff failure.
        reason: String,
    },
    /// Relay accepted the event with this exact message.
    Acknowledged {
        /// Exact bounded relay message.
        message: String,
    },
    /// Relay rejected the event with this exact message.
    Rejected {
        /// Exact bounded relay message.
        message: String,
    },
    /// Bounded policy stopped after definite pre-handoff failure.
    GivenUp {
        /// Exact policy reason.
        reason: String,
    },
    /// Handoff or recovery cannot prove whether the relay received the event.
    Unknown {
        /// Exact ambiguity reason.
        reason: String,
    },
    /// Destination was cancelled while definitely pre-handoff.
    CancelledBeforeHandoff,
}

impl RelayDeliveryOutcome {
    /// Whether this destination has a terminal exact fact.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Acknowledged { .. }
                | Self::Rejected { .. }
                | Self::GivenUp { .. }
                | Self::Unknown { .. }
                | Self::CancelledBeforeHandoff
        )
    }
}

/// Local evidence attached to an event record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PublicationEvidence {
    /// Reattachable receipt identity.
    pub receipt_id: ReceiptId,
    /// Accepted write identity.
    pub write_id: WriteId,
    /// Current signing fact.
    pub signature: SignatureState,
    /// Exact current fact for every selected relay session.
    #[serde(with = "delivery_map")]
    pub destinations: BTreeMap<RelaySessionKey, RelayDeliveryOutcome>,
}

/// One current event contribution from the write store.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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

/// Aggregate current result of one accepted publication obligation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ReceiptOutcome {
    /// Signing or destination work remains open.
    Open,
    /// Application cancelled while every destination was definitely pre-handoff.
    Cancelled,
    /// Every selected destination has an exact terminal fact.
    Complete,
}

/// Reattachable current facts for one accepted publication obligation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Receipt {
    /// Stable accepted-write identity.
    pub write_id: WriteId,
    /// Stable application-visible receipt identity.
    pub receipt_id: ReceiptId,
    /// Last exact local event; cancelled receipts retain it as historical evidence.
    pub current: LocalWriteEvent,
    /// Selected routing mode.
    pub routing: WriteRouting,
    /// Aggregate current receipt result.
    pub outcome: ReceiptOutcome,
    /// Number of durably authorized attempts per destination.
    #[serde(with = "attempt_map")]
    pub attempts: BTreeMap<RelaySessionKey, u32>,
}

impl Receipt {
    /// Whether no signing or delivery work remains.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        !matches!(self.outcome, ReceiptOutcome::Open)
    }

    /// Current per-destination facts.
    #[must_use]
    pub fn destinations(&self) -> &BTreeMap<RelaySessionKey, RelayDeliveryOutcome> {
        &self.current.publication.destinations
    }
}

/// Refusal for an event body that cannot enter accepted local state.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum InvalidEventValue {
    /// An unsigned body was not finalized with its deterministic id.
    #[error("unsigned event has no computed event id")]
    MissingId,
}
