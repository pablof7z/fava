//! Atomic durable same-coordinate edit composition.

use std::collections::BTreeMap;

use fava_routing::RoutePlan;
use fava_write::{
    EventValue, LocalWriteEvent, MaterializationId, PublicKey, PublicationEvidence, Receipt,
    ReceiptId, ReceiptOutcome, ReplaceableEventEdit, SignatureState, UnsignedEvent, WriteRouting,
};
use fava_write_store::{
    AcceptedWrite, WriteStoreError, apply_route_to_receipt, destination_evidence_capacity,
};

use crate::lifecycle::{destinations, next_revision};
use crate::semantic_acceptance::validate_source;
use crate::{RedbWriteStore, SemanticCustody, StoreState};

impl RedbWriteStore {
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)] // One transaction owns composition.
    pub(super) fn compose_semantic(
        &self,
        state: &mut StoreState,
        receipt_id: ReceiptId,
        edit: ReplaceableEventEdit,
        author: PublicKey,
        routing: &WriteRouting,
        event: UnsignedEvent,
        source: Option<&EventValue>,
        initial_route: Option<&RoutePlan>,
    ) -> Result<AcceptedWrite, WriteStoreError> {
        let receipt =
            state.receipts.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("coordinate owner is missing".to_owned())
            })?;
        let (mut edits, stored_author, current_source, _, successor) =
            state.semantics.get(&receipt_id).cloned().ok_or_else(|| {
                WriteStoreError::Refused("semantic custody is missing".to_owned())
            })?;
        if stored_author != author || &receipt.routing != routing {
            return Err(WriteStoreError::Refused(
                "same-coordinate edit author or routing differs from the active operation"
                    .to_owned(),
            ));
        }
        if !matches!(receipt.current.event, EventValue::Unsigned(_)) {
            return Err(WriteStoreError::Refused(
                "same-coordinate edit composition requires an unsigned current generation"
                    .to_owned(),
            ));
        }
        let selected = validate_source(&edit, author, source)?;
        if source != Some(&receipt.current.event)
            || selected.map(|(id, _)| id) != Some(receipt.current.id())
        {
            return Err(WriteStoreError::Refused(
                "same-coordinate edit source is not the exact current generation".to_owned(),
            ));
        }
        if receipt.current.publication.retired_materializations.len()
            >= destination_evidence_capacity()
        {
            return Err(WriteStoreError::Refused(
                "retired materialization evidence capacity reached".to_owned(),
            ));
        }
        let successor_route = initial_route
            .map(|plan| {
                let mut plan = plan.clone();
                plan.revision = receipt.route_revision.checked_add(1).ok_or_else(|| {
                    WriteStoreError::Refused("route revision exhausted".to_owned())
                })?;
                Ok::<_, WriteStoreError>(plan)
            })
            .transpose()?;
        if let Some(plan) = successor_route.as_ref() {
            let mut routed = receipt.clone();
            apply_route_to_receipt(&mut routed, plan)?;
        }
        if matches!(
            receipt.current.publication.signature,
            SignatureState::Authorized
        ) {
            if successor.is_some() {
                return Err(WriteStoreError::Refused(
                    "replaceable coordinate already has a durable successor".to_owned(),
                ));
            }
            let custody: SemanticCustody = (
                edits,
                author,
                current_source,
                None,
                Some((Some(edit), event, current_source, successor_route)),
            );
            self.commit_update(Some(&receipt), Some(&custody), &[])?;
            state.semantics.insert(receipt_id, custody);
            self.publish_receipt(Some(receipt.clone()), receipt_id);
            return Ok(AcceptedWrite {
                write_id: receipt.write_id,
                receipt_id,
                current: receipt.current,
            });
        }

        let mut retired = receipt.current.publication.retired_materializations.clone();
        retired.push((
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            receipt.current.publication.materialization_source,
            receipt.current.publication.materialization_failure.clone(),
        ));
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
        let publication = PublicationEvidence {
            receipt_id,
            write_id: receipt.write_id,
            materialization_id,
            materialization_source: current_source.map(|(id, _)| id),
            materialization_failure: None,
            retired_materializations: retired,
            signature: SignatureState::Unsigned,
            destinations: destinations(routing),
        };
        let current = LocalWriteEvent::new(EventValue::Unsigned(event), publication)?;
        let explicit = matches!(routing, WriteRouting::Explicit(_));
        let desired_destinations = current.publication.destinations.keys().cloned().collect();
        let mut updated = Receipt {
            current,
            outcome: ReceiptOutcome::Open,
            route_revision: u64::from(explicit),
            route_settled: explicit,
            route_shortfalls: Vec::new(),
            desired_destinations,
            attempts: BTreeMap::new(),
            ..receipt
        };
        if let Some(plan) = successor_route.as_ref() {
            apply_route_to_receipt(&mut updated, plan)?;
        }
        edits.push(edit);
        let custody: SemanticCustody = (edits, author, current_source, None, None);
        let next_revision = next_revision(state)?;
        self.commit_update(
            Some(&updated),
            (!updated.is_terminal()).then_some(&custody),
            &[],
        )?;
        state.revision = next_revision;
        if updated.is_terminal() {
            crate::release_semantic(state, receipt_id);
        } else {
            state.semantics.insert(receipt_id, custody);
        }
        state.receipts.insert(receipt_id, updated.clone());
        self.publish_snapshot(state);
        self.publish_receipt(Some(updated.clone()), receipt_id);
        Ok(AcceptedWrite {
            write_id: updated.write_id,
            receipt_id,
            current: updated.current,
        })
    }
}
