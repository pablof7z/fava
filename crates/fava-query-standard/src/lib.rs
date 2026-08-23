//! Reference local evaluator and behavior oracle.

use std::collections::BTreeMap;

use fava_query::{
    EventRecord, FilterSelection, Query, QueryEvaluationError, QueryEvaluator, QueryOrdering,
    QuerySnapshot, ResultAuthority, SourceEvent, SourceSnapshot,
};
use fava_state::{EventCoordinate, RelayEvidence, RelayUrl};
use fava_write::EventValue;
use nostr::event::EventId;

/// Deliberately simple full-reevaluation oracle.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandardQueryEvaluator;

impl QueryEvaluator for StandardQueryEvaluator {
    fn evaluate(
        &self,
        query: &Query,
        sources: &[SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError> {
        let mut by_id = BTreeMap::<EventId, EventRecord>::new();
        for source in sources {
            for contribution in &source.events {
                merge_contribution(&mut by_id, contribution)?;
            }
        }

        let mut events: Vec<_> = coordinate_winners(query.source().authority(), by_id)?
            .into_iter()
            .filter(|record| {
                matches_selection(query.selection(), record)
                    && matches_authority(query.source().authority(), record)
            })
            .collect();
        events.sort_by(|left, right| match query.ordering() {
            QueryOrdering::NewestFirst => right
                .created_at()
                .cmp(&left.created_at())
                .then_with(|| right.id().cmp(&left.id())),
            QueryOrdering::OldestFirst => left
                .created_at()
                .cmp(&right.created_at())
                .then_with(|| left.id().cmp(&right.id())),
        });
        if let Some(limit) = query.result_limit() {
            events.truncate(limit.get());
        }
        Ok(QuerySnapshot::evaluated(events, sources))
    }
}

/// Resolve replaceable coordinates within the authority's candidate universe.
///
/// Result provenance authority selects the candidates, it does not merely filter the
/// winners. `docs/spec/partial-spec-api-semantics.md:194` scopes an `only_from_relays`
/// match to events whose provenance "MUST include at least one relay in the specified
/// set", so a record without qualifying relay provenance is not a candidate at any
/// coordinate and cannot displace one that is. Admitting it into selection and dropping
/// it afterwards would let a purely local write erase a relay-qualified result, which
/// `:200` and `:214` forbid: an unpublished local event "MUST NOT appear", and differing
/// source modes "MUST NOT accidentally share evidence or local-result visibility in a way
/// that changes either query's results".
///
/// Acquisition scope never reaches here: `from_relays` leaves the authority
/// [`ResultAuthority::AnyLocal`], so asking a relay set cannot narrow local visibility.
fn coordinate_winners(
    authority: &ResultAuthority,
    records: BTreeMap<EventId, EventRecord>,
) -> Result<Vec<EventRecord>, QueryEvaluationError> {
    match authority {
        ResultAuthority::AnyLocal => {
            let mut by_coordinate = BTreeMap::<EventCoordinate, EventRecord>::new();
            for record in records.into_values() {
                let coordinate = record
                    .event
                    .coordinate()
                    .map_err(|_| QueryEvaluationError::MissingEventId)?;
                insert_newest(&mut by_coordinate, coordinate, record);
            }
            Ok(by_coordinate.into_values().collect())
        }
        ResultAuthority::OnlyRelays(relays) => {
            let mut by_relay_coordinate =
                BTreeMap::<(RelayUrl, EventCoordinate), EventRecord>::new();
            for record in records.into_values() {
                let coordinate = record
                    .event
                    .coordinate()
                    .map_err(|_| QueryEvaluationError::MissingEventId)?;
                for observation in record.relay_evidence.observations() {
                    if relays.contains(&observation.session.relay) {
                        insert_newest(
                            &mut by_relay_coordinate,
                            (observation.session.relay.clone(), coordinate.clone()),
                            record.clone(),
                        );
                    }
                }
            }
            let mut by_id = BTreeMap::new();
            for record in by_relay_coordinate.into_values() {
                by_id.entry(record.id()).or_insert(record);
            }
            Ok(by_id.into_values().collect())
        }
    }
}

fn insert_newest<K: Ord>(records: &mut BTreeMap<K, EventRecord>, key: K, candidate: EventRecord) {
    match records.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if record_is_newer(&candidate, entry.get()) {
                entry.insert(candidate);
            }
        }
    }
}

fn merge_contribution(
    records: &mut BTreeMap<EventId, EventRecord>,
    contribution: &SourceEvent,
) -> Result<(), QueryEvaluationError> {
    match contribution {
        SourceEvent::Cached(cached) => {
            let id = cached.event.id;
            let record = match records.entry(id) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(EventRecord::new(
                        EventValue::Signed(cached.event.clone()),
                        RelayEvidence::default(),
                        None,
                    )?)
                }
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            };
            record.relay_evidence.merge(&cached.evidence);
            if matches!(record.event, EventValue::Unsigned(_)) {
                record.event = EventValue::Signed(cached.event.clone());
            }
        }
        SourceEvent::Local(local) => {
            let id = local.id();
            let record = match records.entry(id) {
                std::collections::btree_map::Entry::Vacant(entry) => entry.insert(
                    EventRecord::new(local.event.clone(), RelayEvidence::default(), None)?,
                ),
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            };
            if let Some(publication) = &record.publication {
                if publication != &local.publication {
                    return Err(QueryEvaluationError::Refused(format!(
                        "event {id} has conflicting local publication evidence"
                    )));
                }
            } else {
                record.publication = Some(local.publication.clone());
            }
            if matches!(local.event, EventValue::Signed(_)) {
                record.event = local.event.clone();
            }
        }
    }
    Ok(())
}

fn matches_selection(selection: &FilterSelection, record: &EventRecord) -> bool {
    selection
        .ids
        .as_ref()
        .is_none_or(|ids| ids.contains(&record.id()))
        && selection
            .authors
            .as_ref()
            .is_none_or(|authors| authors.contains(&record.event.author()))
        && selection
            .kinds
            .as_ref()
            .is_none_or(|kinds| kinds.contains(&record.event.kind()))
        && selection.tag_values.iter().all(|(key, values)| {
            record.event.tags().iter().any(|tag| {
                tag.single_letter_tag() == Some(*key)
                    && tag.content().is_some_and(|value| values.contains(value))
            })
        })
}

fn matches_authority(authority: &ResultAuthority, record: &EventRecord) -> bool {
    match authority {
        ResultAuthority::AnyLocal => true,
        ResultAuthority::OnlyRelays(relays) => record.relay_evidence.includes_any_relay(relays),
    }
}

fn record_is_newer(candidate: &EventRecord, current: &EventRecord) -> bool {
    candidate.created_at() > current.created_at()
        || (candidate.created_at() == current.created_at() && candidate.id() < current.id())
}
