use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fava_write::{EventBuildError, EventBuilder, EventValue, Kind, PublicKey, WriteIntentError};
use nostr::types::RelayUrl;

use crate::bounds;

/// One author's validated NIP-02 kind-3 contact list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContactList {
    author: PublicKey,
    follows: Vec<Follow>,
    evidence: Vec<ContactListRowEvidence>,
}

impl ContactList {
    /// Decode one complete kind-3 event while conserving every `p` row.
    ///
    /// # Errors
    ///
    /// Returns [`ContactListError`] when the event boundary is invalid. A
    /// malformed contact row remains successful typed row evidence instead.
    pub fn from_event(event: &EventValue) -> Result<Self, ContactListError> {
        if event.kind() != Kind::ContactList {
            return Err(ContactListError::WrongKind(event.kind().as_u16()));
        }
        validate_event(event)?;
        let _ = event.id().ok_or(ContactListError::MissingEventId)?;
        let mut follows = Vec::new();
        let mut evidence = Vec::new();
        let mut seen = BTreeSet::new();

        for (source_index, tag) in event.tags().iter().enumerate() {
            let values = tag.as_slice();
            if values.first().map(String::as_str) != Some("p") {
                continue;
            }
            match parse_row(source_index, values, &seen) {
                Ok(follow) => {
                    seen.insert(follow.pubkey);
                    follows.push(follow);
                }
                Err(row_evidence) => evidence.push(row_evidence),
            }
        }

        Ok(Self {
            author: event.author(),
            follows,
            evidence,
        })
    }

    /// Author whose follows this list describes.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Valid first-occurrence contact rows in source order.
    #[must_use]
    pub const fn follows(&self) -> &[Follow] {
        self.follows.as_slice()
    }

    /// Malformed, duplicate, or uninterpreted contact rows in source order.
    #[must_use]
    pub const fn evidence(&self) -> &[ContactListRowEvidence] {
        self.evidence.as_slice()
    }
}

/// One valid first-occurrence NIP-02 `p` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Follow {
    source_index: usize,
    pubkey: PublicKey,
    relay: Option<RelayUrl>,
    petname: Option<String>,
}

impl Follow {
    /// Original tag index in the source event.
    #[must_use]
    pub const fn source_index(&self) -> usize {
        self.source_index
    }

    /// Followed public key.
    #[must_use]
    pub const fn pubkey(&self) -> PublicKey {
        self.pubkey
    }

    /// Valid non-empty relay hint, if present.
    #[must_use]
    pub const fn relay(&self) -> Option<&RelayUrl> {
        self.relay.as_ref()
    }

    /// Petname exactly as encoded, distinguishing absent from present-empty.
    #[must_use]
    pub fn petname(&self) -> Option<&str> {
        self.petname.as_deref()
    }
}

/// Exact typed evidence for one non-valid NIP-02 `p` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContactListRowEvidence {
    /// The row contains no target column.
    MissingTarget {
        /// Original tag index.
        source_index: usize,
        /// Exact owned source row.
        raw_row: Vec<String>,
    },
    /// The target column is not a valid public key.
    InvalidPublicKey {
        /// Original tag index.
        source_index: usize,
        /// Exact owned source row.
        raw_row: Vec<String>,
    },
    /// A non-empty relay hint is not a valid relay URL.
    InvalidRelayHint {
        /// Original tag index.
        source_index: usize,
        /// Exact owned source row.
        raw_row: Vec<String>,
    },
    /// A fully valid row repeats an earlier fully valid target.
    DuplicateTarget {
        /// Original tag index.
        source_index: usize,
        /// Exact owned source row.
        raw_row: Vec<String>,
        /// Repeated valid target.
        pubkey: PublicKey,
    },
    /// Columns after the optional petname have no NIP-02 meaning here.
    UninterpretedExtraColumns {
        /// Original tag index.
        source_index: usize,
        /// Exact owned source row.
        raw_row: Vec<String>,
    },
}

impl ContactListRowEvidence {
    /// Original tag index in the source event.
    #[must_use]
    pub const fn source_index(&self) -> usize {
        match self {
            Self::MissingTarget { source_index, .. }
            | Self::InvalidPublicKey { source_index, .. }
            | Self::InvalidRelayHint { source_index, .. }
            | Self::DuplicateTarget { source_index, .. }
            | Self::UninterpretedExtraColumns { source_index, .. } => *source_index,
        }
    }

    /// Exact source row without normalization or column loss.
    #[must_use]
    pub fn raw_row(&self) -> &[String] {
        match self {
            Self::MissingTarget { raw_row, .. }
            | Self::InvalidPublicKey { raw_row, .. }
            | Self::InvalidRelayHint { raw_row, .. }
            | Self::DuplicateTarget { raw_row, .. }
            | Self::UninterpretedExtraColumns { raw_row, .. } => raw_row.as_slice(),
        }
    }
}

/// Event-level refusal while decoding a NIP-02 contact list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContactListError {
    /// Event kind was not 3.
    WrongKind(u16),
    /// An unsigned event body had no finalized deterministic id.
    MissingEventId,
    /// Event id or signature verification failed.
    InvalidEvent(String),
    /// Source tag count exceeded the protocol-crate bound.
    TooManyTags {
        /// Actual tag count.
        actual: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Source event bytes exceeded the protocol-crate bound.
    TooLarge {
        /// Actual encoded byte count.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Exact event encoding could not be measured.
    Encoding(String),
    /// The publication route repeats one exact relay identity.
    ///
    /// This is a route defect, not a malformed contact list. Reporting it as
    /// [`Self::InvalidEvent`] sends the caller to fix the event it wrote
    /// instead of the relay set it asked for.
    DuplicateRelay {
        /// Repeated exact relay identity.
        relay: RelayUrl,
    },
    /// The publication route is empty or exceeds its bound.
    InvalidRoute(String),
}

impl fmt::Display for ContactListError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongKind(kind) => write!(formatter, "expected kind 3, got {kind}"),
            Self::MissingEventId => formatter.write_str("contact-list event has no event id"),
            Self::InvalidEvent(reason) => write!(formatter, "invalid contact-list event: {reason}"),
            Self::TooManyTags { actual, maximum } => {
                write!(
                    formatter,
                    "contact-list tags exceed bound: {actual} > {maximum}"
                )
            }
            Self::TooLarge { bytes, maximum } => {
                write!(
                    formatter,
                    "contact-list bytes exceed bound: {bytes} > {maximum}"
                )
            }
            Self::Encoding(reason) => write!(formatter, "contact-list encoding failed: {reason}"),
            Self::DuplicateRelay { relay } => write!(
                formatter,
                "contact-list publication route repeats relay identity {relay}"
            ),
            Self::InvalidRoute(reason) => {
                write!(
                    formatter,
                    "contact-list publication route is invalid: {reason}"
                )
            }
        }
    }
}

impl Error for ContactListError {}

fn parse_row(
    source_index: usize,
    values: &[String],
    seen: &BTreeSet<PublicKey>,
) -> Result<Follow, ContactListRowEvidence> {
    let raw_row = || values.to_vec();
    let Some(raw_pubkey) = values.get(1) else {
        return Err(ContactListRowEvidence::MissingTarget {
            source_index,
            raw_row: raw_row(),
        });
    };
    let pubkey =
        PublicKey::from_hex(raw_pubkey).map_err(|_| ContactListRowEvidence::InvalidPublicKey {
            source_index,
            raw_row: raw_row(),
        })?;
    let relay = match values.get(2).map(String::as_str) {
        None | Some("") => None,
        Some(raw_relay) => Some(RelayUrl::parse(raw_relay).map_err(|_| {
            ContactListRowEvidence::InvalidRelayHint {
                source_index,
                raw_row: raw_row(),
            }
        })?),
    };
    let petname = values.get(3).cloned();
    if values.len() > 4 {
        return Err(ContactListRowEvidence::UninterpretedExtraColumns {
            source_index,
            raw_row: raw_row(),
        });
    }
    if seen.contains(&pubkey) {
        return Err(ContactListRowEvidence::DuplicateTarget {
            source_index,
            raw_row: raw_row(),
            pubkey,
        });
    }
    Ok(Follow {
        source_index,
        pubkey,
        relay,
        petname,
    })
}

fn validate_event(event: &EventValue) -> Result<(), ContactListError> {
    match event {
        EventValue::Unsigned(unsigned) => {
            validate_unsigned_bound(unsigned)?;
            if unsigned.id.is_none() {
                return Err(ContactListError::MissingEventId);
            }
            unsigned
                .verify_id()
                .map_err(|error| ContactListError::InvalidEvent(error.to_string()))
        }
        EventValue::Signed(signed) => {
            bounds::validate_source(signed).map_err(map_write_error)?;
            signed
                .verify()
                .map_err(|error| ContactListError::InvalidEvent(error.to_string()))
        }
    }
}

fn validate_unsigned_bound(event: &fava_write::UnsignedEvent) -> Result<(), ContactListError> {
    EventBuilder::from_parts(
        event.pubkey,
        event.kind,
        event.created_at,
        event.tags.iter().cloned().collect(),
        event.content.clone(),
    )
    .build()
    .map(|_| ())
    .map_err(map_build_error)
}

fn map_build_error(error: EventBuildError) -> ContactListError {
    match error {
        EventBuildError::TooManyTags { actual, maximum } => {
            ContactListError::TooManyTags { actual, maximum }
        }
        EventBuildError::TooLarge { bytes, maximum } => {
            ContactListError::TooLarge { bytes, maximum }
        }
        EventBuildError::Encoding(reason) => ContactListError::Encoding(reason),
    }
}

pub(crate) fn map_write_error(error: WriteIntentError) -> ContactListError {
    match error {
        WriteIntentError::TooLarge { bytes, maximum } if maximum == bounds::MAX_TAGS => {
            ContactListError::TooManyTags {
                actual: bytes,
                maximum,
            }
        }
        WriteIntentError::TooLarge { bytes, maximum } => {
            ContactListError::TooLarge { bytes, maximum }
        }
        WriteIntentError::Encoding(reason) => ContactListError::Encoding(reason),
        // Route refusals are about where the write goes, never about the event
        // body. They must not arrive as `InvalidEvent`.
        WriteIntentError::DuplicateExplicitRelay { relay } => {
            ContactListError::DuplicateRelay { relay }
        }
        error @ (WriteIntentError::EmptyExplicitRelays
        | WriteIntentError::TooManyExplicitRelays { .. }) => {
            ContactListError::InvalidRoute(error.to_string())
        }
        WriteIntentError::InvalidEvent(reason) => ContactListError::InvalidEvent(reason),
        other @ WriteIntentError::Expired => ContactListError::InvalidEvent(other.to_string()),
    }
}
