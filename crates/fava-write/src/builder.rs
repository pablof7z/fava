use nostr::event::{Kind, Tag, UnsignedEvent};
use nostr::key::PublicKey;
use nostr::types::Timestamp;
use thiserror::Error;

use crate::MAX_EVENT_BYTES;

const MAX_TAGS: usize = 2_000;

/// Generic checked construction of one complete unsigned Nostr event.
pub struct EventBuilder {
    author: PublicKey,
    kind: Kind,
    created_at: Timestamp,
    content: String,
    tags: Vec<Tag>,
}

impl EventBuilder {
    /// Begin one event body without interpreting its kind.
    #[must_use]
    pub fn new(author: PublicKey, kind: Kind) -> Self {
        Self {
            author,
            kind,
            created_at: Timestamp::now(),
            content: String::new(),
            tags: Vec::new(),
        }
    }

    /// Set the exact event timestamp.
    #[must_use]
    pub const fn created_at(mut self, created_at: Timestamp) -> Self {
        self.created_at = created_at;
        self
    }

    /// Set opaque event content.
    #[must_use]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Add one already-validated Nostr tag.
    #[must_use]
    pub fn tag(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Produce the exact unsigned body and deterministic event id.
    ///
    /// # Errors
    ///
    /// Returns [`EventBuildError`] when event structure exceeds declared bounds.
    pub fn build(self) -> Result<UnsignedEvent, EventBuildError> {
        if self.tags.len() > MAX_TAGS {
            return Err(EventBuildError::TooManyTags {
                actual: self.tags.len(),
                maximum: MAX_TAGS,
            });
        }
        let mut event = UnsignedEvent::new(
            self.author,
            self.created_at,
            self.kind,
            self.tags,
            self.content,
        );
        event.ensure_id();
        let bytes = serde_json::to_vec(&event)
            .map_err(|error| EventBuildError::Encoding(error.to_string()))?
            .len();
        if bytes > MAX_EVENT_BYTES {
            return Err(EventBuildError::TooLarge {
                bytes,
                maximum: MAX_EVENT_BYTES,
            });
        }
        Ok(event)
    }
}

/// Checked event-construction refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EventBuildError {
    /// Event contains too many tags.
    #[error("event tags exceed bound: {actual} > {maximum}")]
    TooManyTags {
        /// Actual tag count.
        actual: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Serialized event exceeds the declared byte bound.
    #[error("event bytes exceed bound: {bytes} > {maximum}")]
    TooLarge {
        /// Actual serialized bytes.
        bytes: usize,
        /// Declared maximum.
        maximum: usize,
    },
    /// Exact event serialization failed.
    #[error("event encoding failed: {0}")]
    Encoding(String),
}
