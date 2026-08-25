use std::collections::BTreeMap;

use fava_query::SourceKind;
use fava_routing::{RouteContribution, RoutePlan, RouteRequest, RouterSession};
use fava_write::{EventValue, Receipt, ReceiptId, SignatureState, WriteRouting};
use fava_write_store::destination_evidence_capacity;
use tokio::sync::{mpsc, watch};

use super::materialization::SemanticState;
use super::{Publication, STORE_READ_RETRY_DELAY};

impl Publication {
    pub(super) fn start(&self, receipt_id: ReceiptId) {
        self.start_owned(receipt_id, None);
    }

    pub(super) fn start_semantic(&self, receipt_id: ReceiptId, semantic: SemanticState) {
        self.start_owned(receipt_id, Some(semantic));
    }

    fn start_owned(&self, receipt_id: ReceiptId, mut semantic: Option<SemanticState>) {
        let (cancel, cancel_rx) = watch::channel(false);
        let mut cancellations = self
            .cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cancellations.contains_key(&receipt_id) {
            if let Some(semantic) = &mut semantic {
                semantic.close();
            }
            return;
        }
        cancellations.insert(receipt_id, cancel);
        drop(cancellations);
        let publication = self.clone();
        tokio::spawn(async move { publication.run(receipt_id, cancel_rx, semantic).await });
    }

    #[allow(clippy::too_many_lines)] // One custody loop owns receipt, route, and completion order.
    async fn run(
        self,
        receipt_id: ReceiptId,
        mut cancel: watch::Receiver<bool>,
        mut semantic: Option<SemanticState>,
    ) {
        let Some((receipt, mut routes)) = self
            .initialize(receipt_id, &mut semantic, &mut cancel)
            .await
        else {
            return;
        };
        if receipt.is_terminal() {
            if let Some(mut routes) = routes {
                routes.close();
            }
            if let Some(mut semantic) = semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return;
        }
        let mut signer_changes = self.session.subscribe();
        let (mut signing_cancel, signing_cancel_rx) = watch::channel(false);
        let mut signing_generation = self.start_signing(&receipt, signing_cancel_rx);
        let mut materialization_id = receipt.current.publication.materialization_id;

        let mut receipt_changes = self.store.receipt_changes();
        let (lane_finished, mut finished_lanes) = mpsc::channel(destination_evidence_capacity());
        let mut active = BTreeMap::new();
        let mut route_revision = receipt.route_revision;

        loop {
            let Some(mut current) = self.read_receipt(receipt_id, &mut cancel).await else {
                break;
            };
            if current.is_terminal() {
                break;
            }
            let current_materialization = current.current.publication.materialization_id;
            if current_materialization > materialization_id {
                signing_cancel.send_replace(true);
                if let Some(open) = &mut routes {
                    open.close();
                }
                routes = None;
                let Some((opened_receipt, mut opened_routes)) = self
                    .open_generation(current, &mut semantic, &mut cancel)
                    .await
                else {
                    break;
                };
                if opened_receipt.is_terminal() {
                    if let Some(open) = &mut opened_routes {
                        open.close();
                    }
                    break;
                }
                current = opened_receipt;
                routes = opened_routes;
                let (next_cancel, next_cancel_rx) = watch::channel(false);
                signing_cancel = next_cancel;
                signing_generation = self.start_signing(&current, next_cancel_rx);
                materialization_id = current.current.publication.materialization_id;
                route_revision = current.route_revision;
            } else if current_materialization == materialization_id {
                route_revision = route_revision.max(current.route_revision);
            }
            if !matches!(current.current.event, EventValue::Unsigned(_)) {
                signing_generation = None;
            }
            self.start_lanes(&current, &mut active, &lane_finished, &cancel);

            tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        break;
                    }
                }
                changed = signer_changes.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    let Some(revalidated) = self.read_receipt(receipt_id, &mut cancel).await else {
                        break;
                    };
                    if revalidated.current.publication.materialization_id != materialization_id
                        || revalidated.current.id() != current.current.id()
                    {
                        continue;
                    }
                    current = revalidated;
                    let current_generation = self.signer_generation(&current);
                    if current_generation != signing_generation {
                        self.cancel_authorized_signing(
                            &current,
                            signing_generation,
                            &signing_cancel,
                            "its signer attachment was replaced or removed",
                        );
                        let (next_cancel, next_cancel_rx) = watch::channel(false);
                        signing_cancel = next_cancel;
                        signing_generation = self.start_signing(&current, next_cancel_rx);
                    }
                }
                route = next_route(&mut routes), if routes.is_some() => {
                    let Ok(contribution) = route else { routes = None; continue; };
                    let Some(revalidated) = self.read_receipt(receipt_id, &mut cancel).await else {
                        break;
                    };
                    if revalidated.current.publication.materialization_id != materialization_id
                        || revalidated.current.id() != current.current.id()
                    {
                        continue;
                    }
                    current = revalidated;
                    route_revision = route_revision.max(current.route_revision);
                    route_revision = self.apply_route_change(&current, route_revision, &contribution);
                }
                source = next_semantic_source(&mut semantic), if semantic.is_some() => {
                    match source {
                        Some(Ok(_)) => {
                            if let Some(state) = &mut semantic {
                                let Some(revalidated) = self
                                    .refresh_semantic(current.clone(), state, &mut cancel)
                                    .await
                                else {
                                    break;
                                };
                                current = revalidated;
                                self.rematerialize(&current, state);
                            }
                        }
                        Some(Err(kind)) => {
                            if let Some(state) = &mut semantic {
                                self.record_source_failure(&current, state, &kind);
                            }
                        }
                        None => {
                            if let Some(mut state) = semantic.take() {
                                state.close();
                            }
                        }
                    }
                }
                change = receipt_changes.recv() => {
                    match change {
                        Ok((changed, None)) if changed == receipt_id => break,
                        Ok((changed, Some(latest))) if changed == receipt_id => {
                            if latest.is_terminal() {
                                break;
                            }
                            let next_materialization =
                                latest.current.publication.materialization_id;
                            if next_materialization == materialization_id {
                                route_revision = route_revision.max(latest.route_revision);
                                if matches!(
                                    latest.current.publication.signature,
                                    SignatureState::Authorized
                                ) && self
                                    .store
                                    .signing_successor(
                                        latest.write_id,
                                        latest.receipt_id,
                                        next_materialization,
                                        latest.current.id(),
                                    )
                                    .unwrap_or(false)
                                {
                                    self.cancel_authorized_signing(
                                        &latest,
                                        signing_generation,
                                        &signing_cancel,
                                        "a bounded durable successor became ready",
                                    );
                                }
                                if signing_generation.is_none()
                                    && matches!(
                                        &latest.current.publication.signature,
                                        SignatureState::Retryable(reason)
                                            if reason.contains("coordinate reservation resolves")
                                    )
                                {
                                    let (next_cancel, next_cancel_rx) = watch::channel(false);
                                    signing_cancel = next_cancel;
                                    signing_generation =
                                        self.start_signing(&latest, next_cancel_rx);
                                }
                            }
                            if !matches!(latest.current.event, EventValue::Unsigned(_)) {
                                signing_generation = None;
                            }
                        }
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                Some((session, write_id, completed_receipt, completed_materialization, completed_event, completed_route)) = finished_lanes.recv() => {
                    let completed = (
                        write_id,
                        completed_receipt,
                        completed_materialization,
                        completed_event,
                        completed_route,
                    );
                    if active.get(&session) == Some(&completed) {
                        active.remove(&session);
                    }
                }
            }
        }
        if let Some(mut routes) = routes {
            routes.close();
        }
        signing_cancel.send_replace(true);
        if let Some(mut semantic) = semantic {
            semantic.close();
        }
        self.finished(receipt_id);
    }

    async fn initialize(
        &self,
        receipt_id: ReceiptId,
        semantic: &mut Option<SemanticState>,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<(Receipt, Option<Box<dyn RouterSession>>)> {
        let Some(receipt) = self.read_receipt(receipt_id, cancel).await else {
            if let Some(semantic) = semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return None;
        };
        let Some(opened) = self.open_generation(receipt, semantic, cancel).await else {
            if let Some(semantic) = semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return None;
        };
        Some(opened)
    }

    pub(super) async fn read_receipt(
        &self,
        receipt_id: ReceiptId,
        cancel: &mut watch::Receiver<bool>,
    ) -> Option<Receipt> {
        loop {
            if *cancel.borrow() {
                return None;
            }
            match self.store.receipt(receipt_id) {
                Ok(receipt) => return receipt,
                Err(_) => {
                    tokio::select! {
                        changed = cancel.changed() => {
                            if changed.is_err() || *cancel.borrow_and_update() {
                                return None;
                            }
                        }
                        () = tokio::time::sleep(STORE_READ_RETRY_DELAY) => {}
                    }
                }
            }
        }
    }

    fn record_source_failure(&self, receipt: &Receipt, state: &SemanticState, kind: &SourceKind) {
        let source = state.sources.selected(state.selected_id);
        let label = match kind {
            SourceKind::EventCache => "event-cache",
            SourceKind::WriteStore => "write-store",
            SourceKind::LiveRelay { .. } => "live-relay",
        };
        let _ = self.store.record_materialization_failure(
            receipt.write_id,
            receipt.receipt_id,
            receipt.current.publication.materialization_id,
            receipt.current.publication.materialization_source,
            source.as_ref(),
            format!("{label} source observation closed"),
        );
    }

    pub(super) fn rematerialize(&self, receipt: &Receipt, state: &mut SemanticState) {
        let expected = receipt.current.publication.materialization_id;
        let expected_source = receipt.current.publication.materialization_source;
        let Ok((true, successor)) = self.semantic_successor(state, receipt.receipt_id) else {
            return;
        };
        let (event, mut route) = match self.materialize_sequence_and_route(
            &state.edits,
            state.author,
            successor.as_ref(),
            Some(&receipt.current.event),
            &receipt.routing,
        ) {
            Ok(materialized) => materialized,
            Err(error) => {
                let _ = self.store.record_materialization_failure(
                    receipt.write_id,
                    receipt.receipt_id,
                    expected,
                    expected_source,
                    successor.as_ref(),
                    error.to_string(),
                );
                state.failed_id = successor.as_ref().and_then(EventValue::id);
                return;
            }
        };
        let initial_route = if matches!(receipt.routing, WriteRouting::Automatic) {
            let Some(revision) = receipt.route_revision.checked_add(1) else {
                return;
            };
            route.revision = revision;
            Some(&route)
        } else {
            None
        };
        let installed = self.store.install_materialization(
            receipt.write_id,
            receipt.receipt_id,
            expected,
            expected_source,
            &state.edits,
            event,
            successor.as_ref(),
            initial_route,
        );
        let installed = match installed {
            Ok(installed) => installed,
            Err(error) => {
                let _ = self.store.record_materialization_failure(
                    receipt.write_id,
                    receipt.receipt_id,
                    expected,
                    expected_source,
                    successor.as_ref(),
                    error.to_string(),
                );
                state.failed_id = successor.as_ref().and_then(EventValue::id);
                return;
            }
        };
        state.selected_id = successor.as_ref().and_then(EventValue::id);
        state.materialization_id = installed.current.publication.materialization_id;
        if let Some(source) = &successor {
            state.source_floor = Some(source.created_at());
        }
        state.failed_id = None;
    }

    fn apply_route(
        &self,
        receipt: &Receipt,
        revision: u64,
        request: &RouteRequest,
        contribution: &RouteContribution,
    ) -> u64 {
        let plan = match RoutePlan::from_contribution(revision, contribution) {
            Ok(plan) => plan,
            Err(error) => RoutePlan::shortfall(revision, request, error.to_string()),
        };
        match self.store.apply_route(
            receipt.write_id,
            receipt.receipt_id,
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            &plan,
        ) {
            Ok(committed) => committed.route_revision,
            Err(error) => {
                let shortfall = RoutePlan::shortfall(revision, request, error.to_string());
                self.store
                    .apply_route(
                        receipt.write_id,
                        receipt.receipt_id,
                        receipt.current.publication.materialization_id,
                        receipt.current.id(),
                        &shortfall,
                    )
                    .map_or_else(
                        |_| self.committed_route_revision(receipt),
                        |committed| committed.route_revision,
                    )
            }
        }
    }

    fn apply_route_change(
        &self,
        receipt: &Receipt,
        revision: u64,
        contribution: &RouteContribution,
    ) -> u64 {
        let request = RouteRequest::Write(receipt.current.event.clone());
        self.apply_route(receipt, revision.saturating_add(1), &request, contribution)
    }

    fn committed_route_revision(&self, receipt: &Receipt) -> u64 {
        match self.store.receipt(receipt.receipt_id) {
            Ok(Some(current))
                if current.current.publication.materialization_id
                    == receipt.current.publication.materialization_id =>
            {
                current.route_revision
            }
            Ok(Some(_) | None) | Err(_) => receipt.route_revision,
        }
    }

    fn finished(&self, receipt_id: ReceiptId) {
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&receipt_id);
    }
}

async fn next_route(
    routes: &mut Option<Box<dyn RouterSession>>,
) -> Result<fava_routing::RouteContribution, fava_routing::RouterError> {
    match routes {
        Some(routes) => routes.next_change().await,
        None => std::future::pending().await,
    }
}

async fn next_semantic_source(
    semantic: &mut Option<SemanticState>,
) -> Option<Result<SourceKind, SourceKind>> {
    match semantic {
        Some(semantic) => semantic.sources.next_change().await,
        None => std::future::pending().await,
    }
}
