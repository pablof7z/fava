//! Pure universal Nostr event-state values and decisions.

use std::collections::BTreeMap;

use fava_relay::Authority;
use nostr::event::{Event, EventId, Kind, Tag};
use nostr::key::PublicKey;
use nostr::nips::nip01::Coordinate;
use nostr::types::{RelayUrl, Timestamp};

/// One admitted occurrence under one stable logical relay identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayOccurrence {
    /// Exact stable logical relay identity.
    pub session: RelayUrl,
    /// Authority the connection carried when the relay handed this event
    /// over. Content a relay hands over under one authority is not
    /// necessarily content it would hand to another: this travels with the
    /// occurrence so a later query cannot read it back under a different one.
    pub authority: Authority,
    /// Caller-supplied local ingress time.
    pub observed_at: Timestamp,
}

/// Event-id-bound aggregate of exact qualifying relay occurrences.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayOccurrences {
    event_id: EventId,
    occurrences: BTreeMap<RelayUrl, RelayOccurrence>,
}

impl RelayOccurrences {
    /// Event id validated during aggregate construction.
    #[must_use]
    pub const fn event_id(&self) -> EventId {
        self.event_id
    }

    /// Whether no relay contribution qualified.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.occurrences.is_empty()
    }

    /// Number of distinct exact logical sessions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.occurrences.len()
    }

    /// Iterate in deterministic exact-session order.
    pub fn occurrences(&self) -> impl Iterator<Item = &RelayOccurrence> {
        self.occurrences.values()
    }
}

/// One signed event plus exactly one admitted relay occurrence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayEvent {
    event: Event,
    occurrence: RelayOccurrence,
}

impl RelayEvent {
    /// Bind one signed event to one actual occurrence.
    #[must_use]
    pub fn new(
        event: Event,
        session: RelayUrl,
        authority: Authority,
        observed_at: Timestamp,
    ) -> Self {
        Self {
            event,
            occurrence: RelayOccurrence {
                session,
                authority,
                observed_at,
            },
        }
    }

    /// Borrow the exact signed event.
    #[must_use]
    pub const fn event(&self) -> &Event {
        &self.event
    }

    /// Borrow the one exact occurrence.
    #[must_use]
    pub const fn occurrence(&self) -> &RelayOccurrence {
        &self.occurrence
    }
}

/// Aggregate exact occurrences from one complete finite same-event slice.
///
/// Returns `None` when any contribution carries another event id. Repeated
/// delivery through one exact session keeps the earliest local observation
/// time, so the output cardinality cannot exceed the input cardinality.
///
/// # Arguments
///
/// * `event_id` - the exact event id every contribution must share
/// * `contributions` - the complete finite slice of relay contributions to
///   aggregate
#[must_use]
pub fn relay_occurrences_for_event(
    event_id: EventId,
    contributions: &[RelayEvent],
) -> Option<RelayOccurrences> {
    if contributions
        .iter()
        .any(|contribution| contribution.event.id != event_id)
    {
        return None;
    }
    let mut occurrences = BTreeMap::<RelayUrl, RelayOccurrence>::new();
    for contribution in contributions {
        let incoming = contribution.occurrence();
        occurrences
            .entry(incoming.session.clone())
            .and_modify(|current| {
                if incoming.observed_at < current.observed_at {
                    current.observed_at = incoming.observed_at;
                }
            })
            .or_insert_with(|| incoming.clone());
    }
    Some(RelayOccurrences {
        event_id,
        occurrences,
    })
}

/// Identity of one immutable event or one replaceable/addressable coordinate.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EventCoordinate {
    /// Exact immutable event identity.
    Event(EventId),
    /// Current-value coordinate for author, kind, and optional identifier.
    Replaceable {
        /// Exact author.
        author: PublicKey,
        /// Exact replaceable/addressable kind.
        kind: Kind,
        /// `None` for plain replaceable kinds; `Some`, including empty, for
        /// addressable kinds.
        identifier: Option<String>,
    },
}

/// Derive immutable, replaceable, or addressable event identity.
///
/// # Arguments
///
/// * `id` - the event's own id, used when `kind` is neither replaceable nor
///   addressable
/// * `author` - the event author
/// * `kind` - the event kind, which decides which identity shape applies
/// * `tags` - the event tags, searched for a `d` tag when `kind` is
///   addressable
///
/// # Examples
///
/// ```
/// # use fava_state::event_coordinate;
/// # use nostr::event::{EventId, Kind, Tag};
/// # use nostr::key::PublicKey;
/// let id = EventId::from_hex(
///     "0000000000000000000000000000000000000000000000000000000000000000",
/// )
/// .expect("valid event id");
/// let author = PublicKey::from_hex(
///     "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
/// )
/// .expect("valid hex public key");
///
/// // An ordinary kind keeps its own immutable event id.
/// let note = event_coordinate(id, author, Kind::TextNote, &[]);
///
/// // An addressable kind resolves to author, kind, and its `d` tag value.
/// let d_tag = Tag::parse(["d", "profile"]).expect("valid d tag");
/// let addressable = event_coordinate(id, author, Kind::from_u16(30_023), &[d_tag]);
/// ```
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

/// Compare same-coordinate candidates using Nostr winner ordering.
#[must_use]
pub fn event_is_newer(candidate: (Timestamp, EventId), current: (Timestamp, EventId)) -> bool {
    candidate.0 > current.0 || (candidate.0 == current.0 && candidate.1 < current.1)
}

/// Whether an authorized kind-5 event deletes the target event.
///
/// Malformed, short, repeated, extra-valued, and unknown sibling tags remain
/// scoped to themselves and never erase an independently valid target.
#[must_use]
pub fn deletion_applies(
    deletion: (PublicKey, Kind, Timestamp, &[Tag]),
    target: (EventId, PublicKey, Kind, Timestamp, &[Tag]),
) -> bool {
    let (deletion_author, deletion_kind, deletion_at, deletion_tags) = deletion;
    let (target_id, target_author, target_kind, target_at, target_tags) = target;
    if deletion_kind != Kind::EventDeletion
        || target_kind == Kind::EventDeletion
        || deletion_author != target_author
    {
        return false;
    }
    if deletion_tags.iter().any(|tag| {
        let values = tag.as_slice();
        values.first().map(String::as_str) == Some("e")
            && values
                .get(1)
                .and_then(|value| EventId::from_hex(value).ok())
                == Some(target_id)
    }) {
        return true;
    }
    if target_at > deletion_at {
        return false;
    }
    let target_coordinate = event_coordinate(target_id, target_author, target_kind, target_tags);
    deletion_tags.iter().any(|tag| {
        let values = tag.as_slice();
        if values.first().map(String::as_str) != Some("a") {
            return false;
        }
        values
            .get(1)
            .and_then(|value| Coordinate::parse(value).ok())
            .is_some_and(|coordinate| {
                target_coordinate
                    == EventCoordinate::Replaceable {
                        author: coordinate.public_key,
                        kind: coordinate.kind,
                        identifier: if (30_000..40_000).contains(&coordinate.kind.as_u16()) {
                            Some(coordinate.identifier)
                        } else {
                            None
                        },
                    }
            })
    })
}

/// Whether any valid NIP-40 expiration tag is due at `now`.
#[must_use]
pub fn event_is_expired(tags: &[Tag], now: Timestamp) -> bool {
    tags.iter().any(|tag| {
        let values = tag.as_slice();
        values.first().map(String::as_str) == Some("expiration")
            && values
                .get(1)
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|expiry| Timestamp::from(expiry) <= now)
    })
}

/// Exact reason one current source contribution was removed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetractionCause {
    /// An authorized kind-5 event covers the contribution.
    Deleted {
        /// Exact kind-5 event id.
        deletion: EventId,
    },
    /// Another event became this session's coordinate winner.
    Superseded {
        /// Exact winning event id.
        by: EventId,
    },
    /// NIP-40 expiration is due.
    Expired,
    /// A retaining provider removed the contribution under its own policy.
    Evicted,
}

/// One element of an ordered atomic transition for relay contributions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EventStateMutation {
    /// Insert or update one exact `(EventId, RelayUrl)` contribution.
    Upsert(RelayEvent),
    /// Remove one exact contribution.
    Retract {
        /// Exact event removed.
        event_id: EventId,
        /// Exact logical relay contribution removed.
        session: RelayUrl,
        /// Exact protocol or provider reason.
        cause: RetractionCause,
    },
}

/// Compute one ordered transition for a finite current contribution set.
///
/// # Arguments
///
/// * `current` - the finite set of contributions already admitted
/// * `incoming` - the new contribution being evaluated against `current`
/// * `now` - the time used to decide whether `incoming` is already expired
#[must_use]
pub fn mutations_for_event(
    current: &[RelayEvent],
    incoming: RelayEvent,
    now: Timestamp,
) -> Vec<EventStateMutation> {
    let incoming_event = incoming.event();
    if event_is_expired(incoming_event.tags.as_slice(), now)
        || current
            .iter()
            .any(|known| deletion_applies(event_tuple(known.event()), target_tuple(incoming_event)))
    {
        return Vec::new();
    }

    if incoming_event.kind == Kind::EventDeletion {
        let mut mutations = current
            .iter()
            .filter(|known| {
                deletion_applies(event_tuple(incoming_event), target_tuple(known.event()))
            })
            .map(|known| EventStateMutation::Retract {
                event_id: known.event().id,
                session: known.occurrence().session.clone(),
                cause: RetractionCause::Deleted {
                    deletion: incoming_event.id,
                },
            })
            .collect::<Vec<_>>();
        mutations.push(EventStateMutation::Upsert(incoming));
        return mutations;
    }

    let incoming_session = &incoming.occurrence().session;
    let coordinate = coordinate_of(incoming_event);
    let exact_existing = current.iter().find(|known| {
        known.event().id == incoming_event.id && known.occurrence().session == *incoming_session
    });
    if let Some(existing) = exact_existing {
        if incoming.occurrence().observed_at < existing.occurrence().observed_at {
            return vec![EventStateMutation::Upsert(incoming)];
        }
        return Vec::new();
    }

    if matches!(coordinate, EventCoordinate::Replaceable { .. }) {
        let same_session = current
            .iter()
            .filter(|known| {
                known.occurrence().session == *incoming_session
                    && coordinate_of(known.event()) == coordinate
            })
            .collect::<Vec<_>>();
        if same_session.iter().any(|known| {
            !event_is_newer(
                (incoming_event.created_at, incoming_event.id),
                (known.event().created_at, known.event().id),
            )
        }) {
            return Vec::new();
        }
        let mut mutations = same_session
            .into_iter()
            .filter(|known| known.event().id != incoming_event.id)
            .map(|known| EventStateMutation::Retract {
                event_id: known.event().id,
                session: known.occurrence().session.clone(),
                cause: RetractionCause::Superseded {
                    by: incoming_event.id,
                },
            })
            .collect::<Vec<_>>();
        mutations.push(EventStateMutation::Upsert(incoming));
        return mutations;
    }

    vec![EventStateMutation::Upsert(incoming)]
}

/// Compute exact due expiration retractions for a finite contribution set.
#[must_use]
pub fn mutations_for_expiration(current: &[RelayEvent], now: Timestamp) -> Vec<EventStateMutation> {
    current
        .iter()
        .filter(|known| event_is_expired(known.event().tags.as_slice(), now))
        .map(|known| EventStateMutation::Retract {
            event_id: known.event().id,
            session: known.occurrence().session.clone(),
            cause: RetractionCause::Expired,
        })
        .collect()
}

fn coordinate_of(event: &Event) -> EventCoordinate {
    event_coordinate(event.id, event.pubkey, event.kind, event.tags.as_slice())
}

fn event_tuple(event: &Event) -> (PublicKey, Kind, Timestamp, &[Tag]) {
    (
        event.pubkey,
        event.kind,
        event.created_at,
        event.tags.as_slice(),
    )
}

fn target_tuple(event: &Event) -> (EventId, PublicKey, Kind, Timestamp, &[Tag]) {
    (
        event.id,
        event.pubkey,
        event.kind,
        event.created_at,
        event.tags.as_slice(),
    )
}
