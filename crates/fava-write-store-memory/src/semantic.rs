use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_query::{SourceEvent, SourceKind, SourceRevision, SourceSnapshot, SourceStatus};
use fava_state::{EventCoordinate, event_coordinate};
use fava_write::{
    Event, EventId, EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, ReplaceableEventEdit, SignatureState,
    Timestamp, UnsignedEvent, WriteId, WriteIntent, WritePayload, WriteRouting,
};
use fava_write_store::{AcceptedWrite, WriteStoreError, destination_evidence_capacity};

use super::MemoryWriteStore;
use super::model::destinations;

#[derive(Clone, Debug)]
pub(super) struct WriteState {
    pub(super) revision: u64,
    pub(super) next_identity: u64,
    pub(super) writes: BTreeMap<ReceiptId, Receipt>,
    pub(super) coordinates: BTreeMap<EventCoordinate, ReceiptId>,
    #[allow(clippy::type_complexity)] // Existing values deliberately avoid a state wrapper.
    pub(super) edits: BTreeMap<
        ReceiptId,
        (
            ReplaceableEventEdit,
            Option<(EventId, Timestamp)>,
            Option<EventId>,
        ),
    >,
}

impl Default for WriteState {
    fn default() -> Self {
        Self {
            revision: 0,
            next_identity: 1,
            writes: BTreeMap::new(),
            coordinates: BTreeMap::new(),
            edits: BTreeMap::new(),
        }
    }
}

impl MemoryWriteStore {
    pub(super) fn snapshot(state: &WriteState) -> SourceSnapshot {
        SourceSnapshot {
            kind: SourceKind::WriteStore,
            revision: SourceRevision(state.revision),
            status: SourceStatus::Open,
            events: state
                .writes
                .values()
                .filter(|receipt| !matches!(receipt.outcome, ReceiptOutcome::Cancelled))
                .map(|receipt| SourceEvent::Local(receipt.current.clone()))
                .collect(),
        }
    }

    pub(super) fn publish_snapshot(&self, state: &WriteState) {
        self.latest.send_replace(Arc::new(Self::snapshot(state)));
    }

    pub(super) fn accept_semantic(
        &self,
        intent: WriteIntent,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        let (payload, routing) = intent.into_parts();
        let WritePayload::Edit(edit) = payload else {
            return Err(WriteStoreError::Refused(
                "semantic acceptance requires a replaceable-event edit".to_owned(),
            ));
        };
        let selected_source = validate_materialization(&edit, &event, source, &routing)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;

        if let Some(receipt_id) = state.coordinates.get(edit.coordinate()) {
            let receipt = state.writes.get(receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
            let stored = state.edits.get(receipt_id).ok_or_else(|| {
                WriteStoreError::Refused("semantic custody is missing".to_owned())
            })?;
            if stored.0 == edit
                && stored.1 == selected_source
                && receipt.routing == routing
                && receipt.current.event == EventValue::Unsigned(event)
            {
                return Ok(AcceptedWrite {
                    write_id: receipt.write_id,
                    receipt_id: receipt.receipt_id,
                    current: receipt.current.clone(),
                });
            }
            return Err(WriteStoreError::Refused(
                "replaceable-event coordinate already has a live edit".to_owned(),
            ));
        }

        if active_count(&state) >= self.capacity.get() {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }
        let identity = state.next_identity;
        let next_identity = identity
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("write identity exhausted".to_owned()))?;
        let write_id = WriteId::from_u64(identity);
        let receipt_id = ReceiptId::from_u64(identity);
        let publication = PublicationEvidence {
            receipt_id,
            write_id,
            materialization_id: MaterializationId::from_u64(1),
            materialization_source: selected_source.map(|(id, _)| id),
            materialization_failure: None,
            retired_materializations: Vec::new(),
            signature: SignatureState::Unsigned,
            destinations: destinations(&routing),
        };
        let current = LocalWriteEvent::new(EventValue::Unsigned(event), publication)?;
        let explicit = matches!(routing, WriteRouting::Explicit(_));
        let desired_destinations = current.publication.destinations.keys().cloned().collect();
        let receipt = Receipt {
            write_id,
            receipt_id,
            current: current.clone(),
            routing,
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: BTreeMap::new(),
        };
        let next_revision = next_revision(&state)?;

        state.next_identity = next_identity;
        state.revision = next_revision;
        state
            .coordinates
            .insert(edit.coordinate().clone(), receipt_id);
        state
            .edits
            .insert(receipt_id, (edit, selected_source, None));
        state.writes.insert(receipt_id, receipt.clone());
        self.publish_snapshot(&state);
        let _ = self.receipt_changes.send((receipt_id, Some(receipt)));
        Ok(AcceptedWrite {
            write_id,
            receipt_id,
            current,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn install_semantic(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        expected: MaterializationId,
        expected_source: Option<EventId>,
        event: UnsignedEvent,
        source: Option<&Event>,
    ) -> Result<Receipt, WriteStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        let receipt = state
            .writes
            .get(&receipt_id)
            .cloned()
            .ok_or_else(|| WriteStoreError::Refused("receipt does not exist".to_owned()))?;
        let (edit, current_source, _) = state.edits.get(&receipt_id).cloned().ok_or_else(|| {
            WriteStoreError::Refused("semantic custody does not exist".to_owned())
        })?;
        let selected_source = validate_materialization(&edit, &event, source, &receipt.routing)?;

        if receipt.write_id == write_id
            && receipt.current.event == EventValue::Unsigned(event.clone())
            && receipt.current.publication.materialization_source
                == selected_source.map(|(id, _)| id)
        {
            return Ok(receipt);
        }
        require_current(
            &receipt,
            write_id,
            expected,
            expected_source,
            current_source,
        )?;
        require_qualified_source(current_source, selected_source)?;
        if event.created_at <= receipt.current.event.created_at() {
            return Err(WriteStoreError::Refused(
                "successor materialization is not newer than current event".to_owned(),
            ));
        }
        if receipt.current.publication.retired_materializations.len()
            >= destination_evidence_capacity()
        {
            return Err(WriteStoreError::Refused(
                "retired materialization evidence capacity reached".to_owned(),
            ));
        }

        let mut retired = receipt.current.publication.retired_materializations.clone();
        retired.push((
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            receipt.current.publication.materialization_source,
            receipt.current.publication.materialization_failure.clone(),
        ));
        let mut correction_destinations: BTreeSet<_> = receipt.desired_destinations.clone();
        correction_destinations.extend(receipt.current.publication.destinations.keys().cloned());
        if correction_destinations.len() > destination_evidence_capacity() {
            return Err(WriteStoreError::Refused(
                "correction destination capacity reached".to_owned(),
            ));
        }
        let destinations = correction_destinations
            .iter()
            .cloned()
            .map(|session| (session, RelayDeliveryOutcome::Pending))
            .collect();
        let materialization_id = receipt
            .current
            .publication
            .materialization_id
            .as_u64()
            .checked_add(1)
            .map(MaterializationId::from_u64)
            .ok_or_else(|| {
                WriteStoreError::Refused("materialization identity exhausted".to_owned())
            })?;
        let current = LocalWriteEvent::new(
            EventValue::Unsigned(event),
            PublicationEvidence {
                receipt_id,
                write_id,
                materialization_id,
                materialization_source: selected_source.map(|(id, _)| id),
                materialization_failure: None,
                retired_materializations: retired,
                signature: SignatureState::Unsigned,
                destinations,
            },
        )?;
        let mut updated = receipt;
        updated.current = current;
        updated.outcome = ReceiptOutcome::Open;
        updated.desired_destinations = correction_destinations;
        updated.attempts.clear();
        let next_revision = next_revision(&state)?;

        state.revision = next_revision;
        state
            .edits
            .insert(receipt_id, (edit, selected_source, None));
        state.writes.insert(receipt_id, updated.clone());
        self.publish_snapshot(&state);
        let _ = self
            .receipt_changes
            .send((receipt_id, Some(updated.clone())));
        Ok(updated)
    }

    #[allow(clippy::type_complexity)] // Existing values deliberately avoid a recovery wrapper.
    pub(super) fn recover_semantic(
        &self,
    ) -> Result<
        Vec<(
            Receipt,
            ReplaceableEventEdit,
            Option<EventId>,
            Option<EventId>,
        )>,
        WriteStoreError,
    > {
        let state = self
            .state
            .lock()
            .map_err(|_| WriteStoreError::Refused("write state lock poisoned".to_owned()))?;
        Ok(state
            .edits
            .iter()
            .filter_map(|(receipt_id, (edit, source, failed_source))| {
                state.writes.get(receipt_id).and_then(|receipt| {
                    (!receipt.is_terminal()).then(|| {
                        (
                            receipt.clone(),
                            edit.clone(),
                            source.map(|(id, _)| id),
                            *failed_source,
                        )
                    })
                })
            })
            .collect())
    }
}

fn validate_materialization(
    edit: &ReplaceableEventEdit,
    event: &UnsignedEvent,
    source: Option<&Event>,
    routing: &WriteRouting,
) -> Result<Option<(EventId, Timestamp)>, WriteStoreError> {
    WriteIntent::event(event.clone(), routing.clone())?;
    if event.pubkey != edit.actor() || event_coordinate_of_unsigned(event)? != *edit.coordinate() {
        return Err(WriteStoreError::Refused(
            "materialization actor or coordinate does not match edit".to_owned(),
        ));
    }
    let Some(source) = source else {
        return Ok(None);
    };
    source
        .verify()
        .map_err(|error| WriteStoreError::Refused(error.to_string()))?;
    if event_coordinate(
        source.id,
        source.pubkey,
        source.kind,
        source.tags.as_slice(),
    ) != *edit.coordinate()
    {
        return Err(WriteStoreError::Refused(
            "materialization source does not match edit coordinate".to_owned(),
        ));
    }
    if source.created_at >= event.created_at {
        return Err(WriteStoreError::Refused(
            "materialization is not newer than its selected source".to_owned(),
        ));
    }
    Ok(Some((source.id, source.created_at)))
}

fn event_coordinate_of_unsigned(event: &UnsignedEvent) -> Result<EventCoordinate, WriteStoreError> {
    let id = event
        .id
        .ok_or_else(|| WriteStoreError::Refused("materialization has no event id".to_owned()))?;
    Ok(event_coordinate(
        id,
        event.pubkey,
        event.kind,
        event.tags.as_slice(),
    ))
}

fn require_current(
    receipt: &Receipt,
    write_id: WriteId,
    expected: MaterializationId,
    expected_source: Option<EventId>,
    current_source: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    if receipt.is_terminal()
        || receipt.write_id != write_id
        || receipt.current.publication.materialization_id != expected
        || current_source.map(|(id, _)| id) != expected_source
    {
        return Err(WriteStoreError::Refused(
            "semantic materialization is not current".to_owned(),
        ));
    }
    Ok(())
}

fn require_qualified_source(
    current: Option<(EventId, Timestamp)>,
    candidate: Option<(EventId, Timestamp)>,
) -> Result<(), WriteStoreError> {
    let qualified = match (current, candidate) {
        (None, Some(_)) | (Some(_), None) => true,
        (Some((current_id, current_time)), Some((candidate_id, candidate_time))) => {
            candidate_id != current_id && candidate_time > current_time
        }
        (None, None) => false,
    };
    if qualified {
        Ok(())
    } else {
        Err(WriteStoreError::Refused(
            "source event is equal, older, or already consumed".to_owned(),
        ))
    }
}

pub(super) fn next_revision(state: &WriteState) -> Result<u64, WriteStoreError> {
    state
        .revision
        .checked_add(1)
        .ok_or_else(|| WriteStoreError::Refused("source revision exhausted".to_owned()))
}

fn active_count(state: &WriteState) -> usize {
    state
        .writes
        .values()
        .filter(|receipt| !receipt.is_terminal())
        .count()
}
