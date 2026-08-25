//! Deterministic Nostr rules for signed event state learned from relays.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use nostr::event::{Event, EventId, Kind, Tag};
pub use nostr::key::PublicKey;
pub use nostr::types::{RelayUrl, Timestamp};
/// The application-selected authorization identity for relay work.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
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
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Why Nostr event-state rules removed one retained event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetractionCause {
    /// An authorized NIP-09 deletion event covers the retained event.
    Deleted {
        /// The kind-5 event that authorized the retraction and remains retained
        /// as the tombstone preventing resurrection.
        deletion: EventId,
    },
    /// Another event won the same replaceable coordinate.
    Superseded {
        /// The coordinate whose current winner changed.
        coordinate: EventCoordinate,
    },
    /// The event's NIP-40 expiration timestamp has passed.
    Expired,
    /// The provider removed retained state under its own bound or maintenance
    /// rather than under a Nostr rule.
    Evicted,
}

/// One atomic mutation decided by Nostr event-state rules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheMutation {
    /// Insert a new event or merge evidence for the same event id.
    Upsert(CachedEvent),
    /// Retract one retained event id for an exact cause.
    Retract {
        /// The retained event removed.
        event_id: EventId,
        /// The rule that removed it.
        cause: RetractionCause,
    },
}

impl CacheMutation {
    /// Whether the mutation removes retained state.
    ///
    /// A retraction is always applicable: a provider may refuse an insertion
    /// for capacity, but never a removal.
    #[must_use]
    pub const fn is_retraction(&self) -> bool {
        matches!(self, Self::Retract { .. })
    }
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
    candidate.created_at > current.created_at
        || (candidate.created_at == current.created_at && candidate.id < current.id)
}

/// Decide cache mutations for one verified relay observation at an exact time.
#[must_use]
pub fn admission_mutations(
    current: &[CachedEvent],
    incoming: CachedEvent,
    now: Timestamp,
) -> Vec<CacheMutation> {
    if event_is_expired(&incoming.event, now)
        || current
            .iter()
            .any(|known| deletion_applies(&known.event, &incoming.event))
    {
        return Vec::new();
    }

    if incoming.event.kind == Kind::EventDeletion {
        // Retract first: a bounded provider must be able to free room before it
        // records the kind-5 tombstone, so a full cache can still delete.
        let mut mutations: Vec<_> = current
            .iter()
            .filter(|known| deletion_applies(&incoming.event, &known.event))
            .map(|known| CacheMutation::Retract {
                event_id: known.event.id,
                cause: RetractionCause::Deleted {
                    deletion: incoming.event.id,
                },
            })
            .collect();
        mutations.push(CacheMutation::Upsert(incoming));
        return mutations;
    }

    let coordinate = coordinate_for_event(&incoming.event);
    let same_coordinate: Vec<_> = current
        .iter()
        .filter(|known| coordinate_for_event(&known.event) == coordinate)
        .collect();
    if matches!(coordinate, EventCoordinate::Replaceable { .. }) {
        let mut candidates: Vec<_> = same_coordinate
            .iter()
            .map(|known| (*known).clone())
            .collect();
        let existing = candidates
            .iter_mut()
            .find(|known| known.event.id == incoming.event.id);
        let evidence_changed = if let Some(existing) = existing {
            if existing.event != incoming.event {
                return vec![CacheMutation::Upsert(incoming)];
            }
            let previous = existing.evidence.clone();
            existing.merge_evidence(&incoming.evidence);
            existing.evidence != previous
        } else {
            candidates.push(incoming.clone());
            true
        };
        let retained = relay_replaceable_winners(&candidates);
        let mut mutations: Vec<_> = same_coordinate
            .iter()
            .filter(|known| !retained.contains(&known.event.id))
            .map(|known| CacheMutation::Retract {
                event_id: known.event.id,
                cause: RetractionCause::Superseded {
                    coordinate: coordinate.clone(),
                },
            })
            .collect();
        if retained.contains(&incoming.event.id) && evidence_changed {
            mutations.push(CacheMutation::Upsert(incoming));
        }
        return mutations;
    }
    if same_coordinate
        .iter()
        .any(|known| known.event.id == incoming.event.id)
    {
        return vec![CacheMutation::Upsert(incoming)];
    }
    vec![CacheMutation::Upsert(incoming)]
}

fn relay_replaceable_winners(candidates: &[CachedEvent]) -> BTreeSet<EventId> {
    let mut by_relay = BTreeMap::<RelayUrl, &CachedEvent>::new();
    for candidate in candidates {
        for observation in candidate.evidence.observations() {
            by_relay
                .entry(observation.session.relay.clone())
                .and_modify(|current| {
                    if candidate_is_newer(&candidate.event, &current.event) {
                        *current = candidate;
                    }
                })
                .or_insert(candidate);
        }
    }
    if by_relay.is_empty() {
        candidates
            .iter()
            .reduce(|current, candidate| {
                if candidate_is_newer(&candidate.event, &current.event) {
                    candidate
                } else {
                    current
                }
            })
            .map(|winner| BTreeSet::from([winner.event.id]))
            .unwrap_or_default()
    } else {
        by_relay
            .into_values()
            .map(|winner| winner.event.id)
            .collect()
    }
}

/// Decide retractions for events expired at an exact time.
#[must_use]
pub fn expiration_mutations(current: &[CachedEvent], now: Timestamp) -> Vec<CacheMutation> {
    current
        .iter()
        .filter(|known| event_is_expired(&known.event, now))
        .map(|known| CacheMutation::Retract {
            event_id: known.event.id,
            cause: RetractionCause::Expired,
        })
        .collect()
}

/// Whether a NIP-40 expiration timestamp has passed at an exact time.
#[must_use]
pub fn event_is_expired(event: &Event, now: Timestamp) -> bool {
    event.tags.expiration().is_some_and(|expiry| expiry <= now)
}

fn deletion_applies(deletion: &Event, target: &Event) -> bool {
    if deletion.kind != Kind::EventDeletion
        || target.kind == Kind::EventDeletion
        || deletion.pubkey != target.pubkey
    {
        return false;
    }
    if deletion.tags.event_ids().any(|id| id == target.id) {
        return true;
    }
    if target.created_at > deletion.created_at {
        return false;
    }
    let target_coordinate = coordinate_for_event(target);
    deletion.tags.coordinates().any(|coordinate| {
        target_coordinate
            == EventCoordinate::Replaceable {
                author: coordinate.public_key,
                kind: coordinate.kind,
                identifier: if coordinate.identifier.is_empty() {
                    None
                } else {
                    Some(coordinate.identifier)
                },
            }
    })
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
