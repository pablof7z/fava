//! Reference local evaluator and behavior oracle.

use std::collections::BTreeMap;

use fava_query::{
    EventRecord, FilterSelection, Query, QueryEvaluationError, QueryEvaluator, QueryOrdering,
    QuerySnapshot, ResultAuthority, SourceEvent, SourceSnapshot,
};
use fava_state::{EventCoordinate, RelayEvidence};
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

        let mut by_coordinate = BTreeMap::<EventCoordinate, EventRecord>::new();
        for record in by_id.into_values() {
            let coordinate = record
                .event
                .coordinate()
                .map_err(|_| QueryEvaluationError::MissingEventId)?;
            match by_coordinate.entry(coordinate) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if record_is_newer(&record, entry.get()) {
                        entry.insert(record);
                    }
                }
            }
        }

        let mut events: Vec<_> = by_coordinate
            .into_values()
            .filter(|record| {
                matches_selection(query.selection(), record)
                    && matches_authority(query.source().authority(), record)
            })
            .collect();
        events.sort_by(|left, right| match query.ordering() {
            QueryOrdering::NewestFirst => right
                .created_at()
                .cmp(&left.created_at())
                .then_with(|| left.id().cmp(&right.id())),
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
