//! Deterministic Nostr rules for signed event state learned from relays.

use std::collections::{BTreeMap, BTreeSet};

pub use nostr::event::{Event, EventId, Kind, Tag};
pub use nostr::key::PublicKey;
pub use nostr::types::{RelayUrl, Timestamp};

/// The application-selected authorization identity for relay work.
#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelayAccess(String);

impl RelayAccess {
    /// Ordinary unauthenticated public relay access.
    #[must_use]
    pub fn public() -> Self {
        Self::default()
    }

    /// Construct named relay access without exposing provider-private state.
    #[must_use]
    pub fn named(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Return the stable relay-access name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Exact relay and access authority for an observation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelaySessionKey {
    /// Relay that served the event.
    pub relay: RelayUrl,
    /// Relay access under which the event was served.
    pub access: RelayAccess,
}

impl RelaySessionKey {
    /// Construct a relay session key.
    #[must_use]
    pub fn new(relay: RelayUrl, access: RelayAccess) -> Self {
        Self { relay, access }
    }
}

/// Exact evidence that one relay session served an event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayObservation {
    /// Exact session authority.
    pub session: RelaySessionKey,
    /// Local time at which the event was admitted.
    pub observed_at: Timestamp,
}

/// Relay observations currently known for one event id.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RelayEvidence {
    observations: BTreeMap<RelaySessionKey, RelayObservation>,
}

impl RelayEvidence {
    /// Create evidence containing one actual relay observation.
    #[must_use]
    pub fn one(session: RelaySessionKey, observed_at: Timestamp) -> Self {
        let observation = RelayObservation {
            session: session.clone(),
            observed_at,
        };
        Self {
            observations: BTreeMap::from([(session, observation)]),
        }
    }

    /// Merge observations without fabricating or dropping source identity.
    pub fn merge(&mut self, other: &Self) {
        for (session, observation) in &other.observations {
            self.observations
                .entry(session.clone())
                .and_modify(|current| {
                    if observation.observed_at < current.observed_at {
                        current.observed_at = observation.observed_at;
                    }
                })
                .or_insert_with(|| observation.clone());
        }
    }

    /// Whether no relay has actually served the event.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    /// Number of exact relay-session observations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    /// Whether the event was served by a qualifying relay under any relay access.
    #[must_use]
    pub fn includes_any_relay(&self, relays: &BTreeSet<RelayUrl>) -> bool {
        self.observations
            .keys()
            .any(|session| relays.contains(&session.relay))
    }

    /// Iterate over exact observations.
    pub fn observations(&self) -> impl Iterator<Item = &RelayObservation> {
        self.observations.values()
    }
}

/// One signed relay-observed event and its source evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedEvent {
    /// Exact signed Nostr event.
    pub event: Event,
    /// Relays that actually served this exact event.
    pub evidence: RelayEvidence,
}

/// One atomic mutation decided by Nostr event-state rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheMutation {
    /// Insert a new event or merge evidence for the same event id.
    Upsert(CachedEvent),
    /// Retract one retained event id.
    Retract(EventId),
}

impl CachedEvent {
    /// Construct an admitted cached event.
    #[must_use]
    pub fn new(event: Event, evidence: RelayEvidence) -> Self {
        Self { event, evidence }
    }

    /// Merge evidence for the same event id.
    pub fn merge_evidence(&mut self, evidence: &RelayEvidence) {
        self.evidence.merge(evidence);
    }
}

/// Identity of one immutable event or one current replaceable event.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EventCoordinate {
    /// One immutable ordinary event.
    Event(EventId),
    /// Latest event for one author and replaceable coordinate.
    Replaceable {
        /// Event author.
        author: PublicKey,
        /// Replaceable event kind.
        kind: Kind,
        /// First `d` tag value for an addressable coordinate; otherwise absent.
        identifier: Option<String>,
    },
}

/// Determine the identity of an event-shaped value.
#[must_use]
pub fn event_coordinate(
    id: EventId,
    author: PublicKey,
    kind: Kind,
    tags: &[Tag],
) -> EventCoordinate {
    let raw_kind = kind.as_u16();
    if (30_000..40_000).contains(&raw_kind) {
        let identifier = tags
            .iter()
            .find_map(|tag| {
                let values = tag.as_slice();
                (values.first().map(String::as_str) == Some("d"))
                    .then(|| values.get(1).cloned().unwrap_or_default())
            })
            .unwrap_or_default();
        EventCoordinate::Replaceable {
            author,
            kind,
            identifier: Some(identifier),
        }
    } else if kind.is_replaceable() {
        EventCoordinate::Replaceable {
            author,
            kind,
            identifier: None,
        }
    } else {
        EventCoordinate::Event(id)
    }
}

/// Determine the identity of a signed event.
#[must_use]
pub fn coordinate_for_event(event: &Event) -> EventCoordinate {
    event_coordinate(event.id, event.pubkey, event.kind, event.tags.as_slice())
}

/// Compare two same-coordinate event candidates using Nostr winner rules.
#[must_use]
pub fn candidate_is_newer(candidate: &Event, current: &Event) -> bool {
    (candidate.created_at, candidate.id) > (current.created_at, current.id)
}

#[cfg(test)]
mod tests {
    use nostr::event::{EventBuilder, FinalizeEvent, Tag};
    use nostr::key::Keys;

    use super::*;

    #[test]
    fn replaceable_coordinate_includes_addressable_identifier() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::from_u16(30_023), "article")
            .tags([Tag::identifier("first"), Tag::identifier("second")])
            .finalize(&keys)
            .expect("event signs");

        assert_eq!(
            coordinate_for_event(&event),
            EventCoordinate::Replaceable {
                author: keys.public_key(),
                kind: Kind::from_u16(30_023),
                identifier: Some("first".to_owned()),
            }
        );
    }
}
