//! One-attempt event publication contract.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use fava_state::RelaySessionKey;
use fava_transport::Transport;
use fava_write::{Event, MaterializationId, ReceiptId, WriteId};

/// Limits one relay declares for the events it will accept.
///
/// Every field is optional and absent means the relay did not say. A claim
/// Fava can evaluate locally refuses knowingly-invalid work before handoff;
/// an unknown claim never becomes an invented default.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RelayWriteLimits {
    /// Largest accepted frame in bytes.
    pub max_message_length: Option<usize>,
    /// Largest accepted event content in bytes.
    pub max_content_length: Option<usize>,
    /// Largest accepted tag count on one event.
    pub max_event_tags: Option<usize>,
    /// Smallest accepted proof-of-work difficulty.
    pub min_pow_difficulty: Option<u8>,
    /// Whether the relay requires NIP-42 before accepting an event.
    pub auth_required: Option<bool>,
    /// Whether the relay restricts who may write.
    pub restricted_writes: Option<bool>,
}

impl RelayWriteLimits {
    /// Exact reason this relay has told us it cannot accept these event facts.
    ///
    /// Only claims Fava can evaluate locally produce a refusal. An unknown
    /// claim, or one that depends on relay-side authorization, never refuses.
    #[must_use]
    pub fn refusal(&self, facts: &EventLimitFacts) -> Option<String> {
        if let Some(maximum) = self.max_message_length
            && facts.frame_bytes > maximum
        {
            return Some(format!(
                "relay declares max_message_length={maximum} but this EVENT frame uses {} bytes",
                facts.frame_bytes
            ));
        }
        if let Some(maximum) = self.max_content_length
            && facts.content_bytes > maximum
        {
            return Some(format!(
                "relay declares max_content_length={maximum} but this event content uses {} bytes",
                facts.content_bytes
            ));
        }
        if let Some(maximum) = self.max_event_tags
            && facts.tags > maximum
        {
            return Some(format!(
                "relay declares max_event_tags={maximum} but this event carries {} tags",
                facts.tags
            ));
        }
        if let Some(required) = self.min_pow_difficulty
            && facts.pow_difficulty < required
        {
            return Some(format!(
                "relay declares min_pow_difficulty={required} but this event id proves {}",
                facts.pow_difficulty
            ));
        }
        None
    }

    /// No relay claim is currently known for this relay.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            max_message_length: None,
            max_content_length: None,
            max_event_tags: None,
            min_pow_difficulty: None,
            auth_required: None,
            restricted_writes: None,
        }
    }
}

/// Locally measurable facts about one event, checked against relay claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventLimitFacts {
    /// Encoded `EVENT` frame size in bytes.
    pub frame_bytes: usize,
    /// Event content size in bytes.
    pub content_bytes: usize,
    /// Number of tags on the event.
    pub tags: usize,
    /// Leading zero bits proven by the event id.
    pub pow_difficulty: u8,
}

impl EventLimitFacts {
    /// Measure one signed event and its encoded frame.
    #[must_use]
    pub fn measure(event: &Event, frame_bytes: usize) -> Self {
        Self {
            frame_bytes,
            content_bytes: event.content.len(),
            tags: event.tags.len(),
            pow_difficulty: leading_zero_bits(event.id.as_bytes()),
        }
    }
}

/// Count the leading zero bits an event id proves.
fn leading_zero_bits(id: &[u8; 32]) -> u8 {
    let mut bits = 0_u8;
    for byte in id {
        if *byte == 0 {
            bits = bits.saturating_add(8);
        } else {
            return bits.saturating_add(byte.leading_zeros().try_into().unwrap_or(u8::MAX));
        }
    }
    bits
}

/// One exact publication attempt at one exact destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishAttempt {
    /// Stable accepted-write identity.
    pub write_id: WriteId,
    /// Stable receipt identity.
    pub receipt_id: ReceiptId,
    /// Exact immutable materialization generation being published.
    pub materialization_id: MaterializationId,
    /// One-based durable attempt count for this destination.
    pub number: u32,
    /// Exact relay and access destination.
    pub session: RelaySessionKey,
    /// Exact signed event bytes and identity.
    pub event: Event,
    /// Maximum time this one attempt may remain unresolved.
    pub timeout: Duration,
}

/// Exact result of one publisher attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    /// Relay accepted the event with this exact bounded message.
    Acknowledged {
        /// Exact bounded relay message.
        message: String,
    },
    /// Relay rejected the event with this exact bounded message.
    Rejected {
        /// Exact bounded relay message.
        message: String,
    },
    /// The relay declares it cannot accept this event, checked before handoff.
    RefusedByLimit {
        /// Exact declared limit and the actual value that exceeded it.
        reason: String,
    },
    /// Relay access was required and not granted for this exact attempt.
    AuthenticationDenied {
        /// Exact scoped authentication reason.
        reason: String,
    },
    /// Bytes definitely were not handed to transport.
    NotHandedOff {
        /// Exact definite failure reason.
        reason: String,
    },
    /// Handoff or later outcome cannot be proven.
    OutcomeUnknown {
        /// Exact ambiguity reason.
        reason: String,
    },
}

/// Replaceable mechanism performing one exact publication attempt.
pub trait Publisher: Send + Sync {
    /// Perform exactly one attempt without selecting retry or destination policy.
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>>;
}
