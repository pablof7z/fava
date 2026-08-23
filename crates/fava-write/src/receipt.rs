use std::collections::{BTreeMap, BTreeSet};

use fava_state::RelaySessionKey;
use serde::{Deserialize, Serialize};

use crate::{
    EventId, EventValue, InvalidEventValue, MaterializationId, ReceiptId, WriteId, WriteRouting,
};

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
    /// Relay demanded authentication this attempt did not satisfy, after handoff.
    AuthenticationDenied {
        /// Exact bounded relay authentication fact.
        reason: String,
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
                | Self::AuthenticationDenied { .. }
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
    /// Exact current immutable materialization generation.
    pub materialization_id: MaterializationId,
    /// Qualified source event used for the current materialization, when any.
    pub materialization_source: Option<EventId>,
    /// Bounded post-accept materialization failure attributed to current work.
    pub materialization_failure: Option<String>,
    /// Bounded retired generation, event, source, and optional failure facts.
    pub retired_materializations:
        Vec<(MaterializationId, EventId, Option<EventId>, Option<String>)>,
    /// Current signing fact.
    pub signature: SignatureState,
    /// Exact current fact for every selected relay session.
    #[serde(with = "crate::delivery_map")]
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
    /// Automatic routing settled without selecting a destination.
    NoDestination,
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
    /// Last route revision atomically applied to this receipt.
    pub route_revision: u64,
    /// Whether the current route has no unresolved target.
    pub route_settled: bool,
    /// Exact bounded shortfalls and settled-absence reasons for the current route.
    pub route_shortfalls: Vec<String>,
    /// Current destinations still required by the live route.
    #[serde(with = "crate::session_set")]
    pub desired_destinations: BTreeSet<RelaySessionKey>,
    /// Number of durably authorized attempts per destination.
    #[serde(with = "crate::attempt_map")]
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

    /// Number of destinations with exact relay acknowledgement evidence.
    #[must_use]
    pub fn acknowledged(&self) -> usize {
        self.destinations()
            .values()
            .filter(|outcome| matches!(outcome, RelayDeliveryOutcome::Acknowledged { .. }))
            .count()
    }

    /// Number of destinations with exact relay rejection evidence.
    #[must_use]
    pub fn rejected(&self) -> usize {
        self.destinations()
            .values()
            .filter(|outcome| matches!(outcome, RelayDeliveryOutcome::Rejected { .. }))
            .count()
    }

    /// Number of destinations required by the current live route.
    #[must_use]
    pub fn desired(&self) -> usize {
        self.desired_destinations.len()
    }

    /// Whether the current live route still requires this destination.
    #[must_use]
    pub fn desires(&self, session: &RelaySessionKey) -> bool {
        self.desired_destinations.contains(session)
    }
}
