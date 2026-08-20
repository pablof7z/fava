//! Reference local evaluator and semantic oracle.

use std::collections::BTreeMap;

use nmp_query::{
    CanonicalQuery, EventRecord, FilterSelection, QueryEvaluationError, QueryEvaluator,
    QueryOrdering, QuerySnapshot, ResultAuthority, SourceEvent, SourceSnapshot, Timestamp,
};
use nmp_state::{EventCoordinate, RelayEvidence};
use nmp_write::EventValue;
use nostr::event::EventId;

/// Deliberately simple full-reevaluation oracle.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandardQueryEvaluator;

impl QueryEvaluator for StandardQueryEvaluator {
    fn evaluate(
        &self,
        query: &CanonicalQuery,
        sources: &[SourceSnapshot],
    ) -> Result<QuerySnapshot, QueryEvaluationError> {
        let mut by_id = BTreeMap::<EventId, EventRecord>::new();
        for source in sources {
            for contribution in &source.events {
                merge_contribution(&mut by_id, contribution)?;
            }
        }

        let query_value = query.as_query();
        let mut by_coordinate = BTreeMap::<EventCoordinate, EventRecord>::new();
        for record in by_id.into_values().filter(|record| {
            matches_selection(&query_value.selection, record)
                && matches_authority(&query_value.source.authority, record)
        }) {
            let coordinate = record
                .event
                .coordinate()
                .map_err(|_| QueryEvaluationError::MissingEventId)?;
            match by_coordinate.entry(coordinate) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(record);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    if record_order_key(&record) > record_order_key(entry.get()) {
                        entry.insert(record);
                    }
                }
            }
        }

        let mut events: Vec<_> = by_coordinate.into_values().collect();
        events.sort_by(|left, right| match query_value.ordering {
            QueryOrdering::NewestFirst => record_order_key(right).cmp(&record_order_key(left)),
            QueryOrdering::OldestFirst => record_order_key(left).cmp(&record_order_key(right)),
        });
        if let Some(limit) = query_value.limit {
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
}

fn matches_authority(authority: &ResultAuthority, record: &EventRecord) -> bool {
    match authority {
        ResultAuthority::AnyLocal => true,
        ResultAuthority::OnlyRelays(relays) => record.relay_evidence.includes_any_relay(relays),
    }
}

fn record_order_key(record: &EventRecord) -> (Timestamp, EventId) {
    (record.created_at(), record.id())
}
