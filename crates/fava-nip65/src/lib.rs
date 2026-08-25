//! Pure NIP-65 relay-list vocabulary and parsing.

use std::collections::BTreeSet;

use fava_query::{Query, QueryError};
use fava_write::{EventValue, Kind, PublicKey};
use nostr::types::RelayUrl;
use thiserror::Error;

const MAX_RELAYS: usize = 256;

/// One valid NIP-65 kind:10002 relay list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayList {
    author: PublicKey,
    read_relays: BTreeSet<RelayUrl>,
    write_relays: BTreeSet<RelayUrl>,
}

impl RelayList {
    /// Parse one event-shaped value as a NIP-65 relay list.
    ///
    /// # Errors
    ///
    /// Returns [`RelayListError`] for the wrong kind, malformed relay URL, or
    /// excessive relay count.
    pub fn from_event(event: &EventValue) -> Result<Self, RelayListError> {
        if event.kind() != Kind::from(10_002_u16) {
            return Err(RelayListError::WrongKind(event.kind().as_u16()));
        }
        let mut read_relays = BTreeSet::new();
        let mut write_relays = BTreeSet::new();
        for tag in event.tags() {
            let values = tag.as_slice();
            if values.first().map(String::as_str) != Some("r") {
                continue;
            }
            let Some(raw_relay) = values.get(1) else {
                continue;
            };
            let relay = RelayUrl::parse(raw_relay)
                .map_err(|_| RelayListError::InvalidRelay(raw_relay.clone()))?;
            match values.get(2).map(String::as_str) {
                Some("read") => {
                    read_relays.insert(relay);
                }
                Some("write") => {
                    write_relays.insert(relay);
                }
                None | Some("") => {
                    read_relays.insert(relay.clone());
                    write_relays.insert(relay);
                }
                Some(_) => continue,
            }
            let count = read_relays.union(&write_relays).count();
            if count > MAX_RELAYS {
                return Err(RelayListError::TooManyRelays {
                    actual: count,
                    maximum: MAX_RELAYS,
                });
            }
        }
        Ok(Self {
            author: event.author(),
            read_relays,
            write_relays,
        })
    }

    /// Relay-list author.
    #[must_use]
    pub const fn author(&self) -> PublicKey {
        self.author
    }

    /// Relays where the author declares reading events.
    #[must_use]
    pub const fn read_relays(&self) -> &BTreeSet<RelayUrl> {
        &self.read_relays
    }

    /// Relays where the author declares publishing events.
    #[must_use]
    pub const fn write_relays(&self) -> &BTreeSet<RelayUrl> {
        &self.write_relays
    }
}

/// Build the ordinary bounded query that owns NIP-65 winner selection.
///
/// # Errors
///
/// Returns the query owner's refusal unchanged.
pub fn relay_lists(authors: impl IntoIterator<Item = PublicKey>) -> Result<Query, QueryError> {
    Query::events()
        .kind(Kind::from(10_002_u16))
        .authors(authors)
        .limit(4_096)
}

/// NIP-65 relay-list parsing refusal.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RelayListError {
    /// Event kind was not 10002.
    #[error("expected kind 10002, got {0}")]
    WrongKind(u16),
    /// One `r` tag carried an invalid relay URL.
    #[error("invalid NIP-65 relay URL: {0}")]
    InvalidRelay(String),
    /// Distinct relay count exceeded the protocol-crate bound.
    #[error("relay-list count exceeds bound: {actual} > {maximum}")]
    TooManyRelays {
        /// Actual distinct relay count.
        actual: usize,
        /// Declared bound.
        maximum: usize,
    },
}

#[cfg(test)]
mod tests {
    use fava_write::EventBuilder;
    use nostr::key::Keys;
    use nostr::types::Timestamp;

    use super::*;

    #[test]
    fn parses_read_write_and_both_markers() {
        let event = EventBuilder::new(Keys::generate().public_key(), Kind::from(10_002_u16))
            .created_at(Timestamp::from(7))
            .tag(fava_write::Tag::parse(["r", "wss://read.example", "read"]).expect("read tag"))
            .tag(fava_write::Tag::parse(["r", "wss://write.example", "write"]).expect("write tag"))
            .tag(fava_write::Tag::parse(["r", "wss://both.example"]).expect("both tag"))
            .build()
            .expect("event");
        let list = RelayList::from_event(&EventValue::Unsigned(event)).expect("relay list");

        assert_eq!(list.read_relays().len(), 2);
        assert_eq!(list.write_relays().len(), 2);
    }
}
