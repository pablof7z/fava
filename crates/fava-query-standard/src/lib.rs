//! Reference local evaluator and behavior oracle.

use std::collections::BTreeMap;

use fava_query::{
    EventRecord, FilterSelection, Query, QueryEvaluationError, QueryEvaluator, QueryOrdering,
    QuerySnapshot, ResultAuthority, SourceEvent, SourceSnapshot,
};
use fava_state::{
    EventCoordinate, RelayEvent, event_coordinate, event_is_newer, relay_occurrences_for_event,
};
use fava_write::{EventValue, PublicationEvidence};
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
        let mut by_id = BTreeMap::<EventId, Candidate>::new();
        for source in sources {
            for contribution in &source.events {
                merge_qualifying_contribution(&mut by_id, query, contribution)?;
            }
        }

        let records = by_id
            .into_iter()
            .map(|(id, candidate)| candidate.into_record(id))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|record| matches_selection(query.selection(), record))
            .collect();
        let mut events = coordinate_winners(records);
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

struct Candidate {
    event: EventValue,
    relay: Vec<RelayEvent>,
    publication: Option<PublicationEvidence>,
}

impl Candidate {
    fn into_record(self, id: EventId) -> Result<EventRecord, QueryEvaluationError> {
        let occurrences = relay_occurrences_for_event(id, &self.relay).ok_or(
            QueryEvaluationError::RelayOccurrenceEventMismatch {
                event: id,
                occurrences: self
                    .relay
                    .iter()
                    .find(|contribution| contribution.event().id != id)
                    .map_or(id, |contribution| contribution.event().id),
            },
        )?;
        EventRecord::new(self.event, occurrences, self.publication)
    }
}

fn merge_qualifying_contribution(
    records: &mut BTreeMap<EventId, Candidate>,
    query: &Query,
    contribution: &SourceEvent,
) -> Result<(), QueryEvaluationError> {
    match contribution {
        SourceEvent::Relay(relay_event) => {
            if !relay_qualifies(query, relay_event) {
                return Ok(());
            }
            let event = relay_event.event();
            let candidate = records.entry(event.id).or_insert_with(|| Candidate {
                event: EventValue::Signed(event.clone()),
                relay: Vec::new(),
                publication: None,
            });
            candidate.relay.push(relay_event.clone());
            if matches!(candidate.event, EventValue::Unsigned(_)) {
                candidate.event = EventValue::Signed(event.clone());
            }
        }
        SourceEvent::Local(local) => {
            if !matches!(query.source().authority(), ResultAuthority::AnyLocal) {
                return Ok(());
            }
            let candidate = records.entry(local.id()).or_insert_with(|| Candidate {
                event: local.event.clone(),
                relay: Vec::new(),
                publication: None,
            });
            candidate.publication.get_or_insert_with(|| local.publication.clone());
            if matches!(local.event, EventValue::Signed(_)) {
                candidate.event = local.event.clone();
            }
        }
    }
    Ok(())
}

fn relay_qualifies(query: &Query, relay_event: &RelayEvent) -> bool {
    let occurrence = relay_event.occurrence();
    if &occurrence.session.access != query.access() {
        return false;
    }
    match query.source().authority() {
        ResultAuthority::AnyLocal => true,
        ResultAuthority::OnlyRelays(relays) => relays.contains(&occurrence.session.relay),
    }
}

fn coordinate_winners(records: Vec<EventRecord>) -> Vec<EventRecord> {
    let mut by_coordinate = BTreeMap::<EventCoordinate, EventRecord>::new();
    for record in records {
        let id = record.id();
        let event = record.event();
        let coordinate = event_coordinate(id, event.author(), event.kind(), event.tags());
        insert_newest(&mut by_coordinate, coordinate, record);
    }
    by_coordinate.into_values().collect()
}

fn insert_newest<K: Ord>(records: &mut BTreeMap<K, EventRecord>, key: K, incoming: EventRecord) {
    match records.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(incoming);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            if event_is_newer(
                (incoming.created_at(), incoming.id()),
                (entry.get().created_at(), entry.get().id()),
            ) {
                entry.insert(incoming);
            }
        }
    }
}

fn matches_selection(selection: &FilterSelection, record: &EventRecord) -> bool {
    selection
        .ids
        .as_ref()
        .is_none_or(|ids| ids.contains(&record.id()))
        && selection
            .authors
            .as_ref()
            .is_none_or(|authors| authors.contains(&record.event().author()))
        && selection
            .kinds
            .as_ref()
            .is_none_or(|kinds| kinds.contains(&record.event().kind()))
        && selection.tag_values.iter().all(|(key, values)| {
            record.event().tags().iter().any(|tag| {
                tag.single_letter_tag() == Some(*key)
                    && tag.content().is_some_and(|value| values.contains(value))
            })
        })
}
