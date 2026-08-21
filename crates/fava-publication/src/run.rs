use std::collections::BTreeMap;
use std::time::Duration;

use fava_query::SourceKind;
use fava_routing::{RouteContribution, RoutePlan, RouteRequest, RouterSession};
use fava_signer::{SignerAvailability, SignerError};
use fava_write::{EventValue, Receipt, ReceiptId, WriteIntent, WriteRouting};
use fava_write_store::destination_evidence_capacity;
use tokio::sync::{mpsc, watch};

use super::Publication;
use super::materialization::SemanticState;

const STORE_READ_RETRY_DELAY: Duration = Duration::from_millis(10);

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
        let (mut signing_cancel, signing_cancel_rx) = watch::channel(false);
        self.start_signing(&receipt, signing_cancel_rx);
        let mut materialization_id = receipt.current.publication.materialization_id;

        let mut receipt_changes = self.store.receipt_changes();
        let (lane_finished, mut finished_lanes) = mpsc::channel(destination_evidence_capacity());
        let mut active = BTreeMap::new();
        let mut route_revision = receipt.route_revision;

        loop {
            let Some(current) = self.read_receipt(receipt_id, &mut cancel).await else {
                break;
            };
            self.start_lanes(&current, &mut active, &lane_finished, &cancel);
            if current.is_terminal() {
                break;
            }

            tokio::select! {
                biased;
                changed = cancel.changed() => {
                    if changed.is_err() || *cancel.borrow_and_update() {
                        break;
                    }
                }
                route = next_route(&mut routes), if routes.is_some() => {
                    let Ok(contribution) = route else { routes = None; continue; };
                    route_revision = self.apply_route_change(&current, route_revision, &contribution);
                }
                source = next_semantic_source(&mut semantic), if semantic.is_some() => {
                    match source {
                        Some(Ok(_)) => {
                            if let Some(state) = &mut semantic {
                                self.rematerialize(&current, state);
                            }
                        }
                        Some(Err(kind)) => {
                            if let Some(state) = &mut semantic {
                                self.record_source_failure(&current, state, kind);
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
                            let next_materialization =
                                latest.current.publication.materialization_id;
                            if next_materialization == materialization_id {
                                route_revision = route_revision.max(latest.route_revision);
                            } else {
                                route_revision = self.reopen_materialization(
                                    &latest,
                                    &mut routes,
                                    &mut signing_cancel,
                                );
                                materialization_id = next_materialization;
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
        let Some(mut receipt) = self.read_receipt(receipt_id, cancel).await else {
            if let Some(semantic) = semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return None;
        };
        if let Some(state) = semantic {
            self.rematerialize(&receipt, state);
            let Some(current) = self.read_receipt(receipt_id, cancel).await else {
                state.close();
                self.finished(receipt_id);
                return None;
            };
            receipt = current;
        }
        let (routes, _) = self.open_routes(&receipt);
        if matches!(receipt.routing, WriteRouting::Automatic)
            && routes.is_none()
            && semantic.is_none()
        {
            self.finished(receipt_id);
            return None;
        }
        let Some(current) = self.read_receipt(receipt_id, cancel).await else {
            if let Some(semantic) = semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return None;
        };
        Some((current, routes))
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

    fn record_source_failure(&self, receipt: &Receipt, state: &SemanticState, kind: SourceKind) {
        let source = state.sources.selected(state.selected_id);
        let label = match kind {
            SourceKind::EventCache => "event-cache",
            SourceKind::WriteStore => "write-store",
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

    fn rematerialize(&self, receipt: &Receipt, state: &mut SemanticState) {
        let expected = receipt.current.publication.materialization_id;
        let expected_source = receipt.current.publication.materialization_source;
        let Ok((true, successor)) = self.semantic_successor(state, receipt.receipt_id, expected)
        else {
            return;
        };
        let Ok(intent) =
            WriteIntent::edit_as(state.edit.clone(), state.author, receipt.routing.clone())
        else {
            return;
        };
        let (event, mut route) = match self.materialize_and_route(
            &intent,
            successor.as_ref(),
            Some(&receipt.current.event),
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
                state.failed_id = successor.as_ref().map(|event| event.id);
                return;
            }
        };
        let installed = self.store.install_materialization(
            receipt.write_id,
            receipt.receipt_id,
            expected,
            expected_source,
            event,
            successor.as_ref(),
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
                state.failed_id = successor.as_ref().map(|event| event.id);
                return;
            }
        };
        if matches!(receipt.routing, WriteRouting::Automatic) {
            let Some(revision) = installed.route_revision.checked_add(1) else {
                return;
            };
            route.revision = revision;
            let _ = self.store.apply_route(
                installed.write_id,
                installed.receipt_id,
                installed.current.publication.materialization_id,
                installed.current.id(),
                &route,
            );
        }
        state.selected_id = successor.as_ref().map(|event| event.id);
        if let Some(source) = &successor {
            state.source_floor = Some(source.created_at);
        }
        state.failed_id = None;
    }

    fn open_routes(&self, receipt: &Receipt) -> (Option<Box<dyn RouterSession>>, u64) {
        let WriteRouting::Automatic = receipt.routing else {
            return (None, receipt.route_revision);
        };
        let request = RouteRequest::Write(receipt.current.event.clone());
        match fava_routing::open(self.routers.as_slice(), &request) {
            Ok(routes) => {
                let revision = receipt.route_revision.saturating_add(1);
                let committed = self.apply_route(receipt, revision, &request, &routes.current());
                (Some(routes), committed)
            }
            Err(error) => {
                let plan = RoutePlan::shortfall(
                    receipt.route_revision.saturating_add(1),
                    &request,
                    error.to_string(),
                );
                let _ = self.store.apply_route(
                    receipt.write_id,
                    receipt.receipt_id,
                    receipt.current.publication.materialization_id,
                    receipt.current.id(),
                    &plan,
                );
                (None, self.committed_route_revision(receipt))
            }
        }
    }

    fn reopen_materialization(
        &self,
        receipt: &Receipt,
        routes: &mut Option<Box<dyn RouterSession>>,
        signing_cancel: &mut watch::Sender<bool>,
    ) -> u64 {
        signing_cancel.send_replace(true);
        let (next_cancel, next_cancel_rx) = watch::channel(false);
        *signing_cancel = next_cancel;
        self.start_signing(receipt, next_cancel_rx);
        if let Some(open) = routes {
            open.close();
        }
        let (opened, committed_revision) = self.open_routes(receipt);
        *routes = opened;
        committed_revision
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
        if let Err(error) = self.store.apply_route(
            receipt.write_id,
            receipt.receipt_id,
            receipt.current.publication.materialization_id,
            receipt.current.id(),
            &plan,
        ) {
            let shortfall = RoutePlan::shortfall(revision, request, error.to_string());
            let _ = self.store.apply_route(
                receipt.write_id,
                receipt.receipt_id,
                receipt.current.publication.materialization_id,
                receipt.current.id(),
                &shortfall,
            );
        }
        self.committed_route_revision(receipt)
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
        self.store
            .receipt(receipt.receipt_id)
            .ok()
            .flatten()
            .filter(|current| {
                current.current.publication.materialization_id
                    == receipt.current.publication.materialization_id
            })
            .map_or(receipt.route_revision, |current| current.route_revision)
    }

    fn start_signing(&self, receipt: &Receipt, cancel: watch::Receiver<bool>) {
        let EventValue::Unsigned(unsigned) = receipt.current.event.clone() else {
            return;
        };
        let Some(signer) = self.signers.get(&unsigned.pubkey).cloned() else {
            return;
        };
        if !matches!(signer.availability(), SignerAvailability::Available) {
            return;
        }
        let publication = self.clone();
        let write_id = receipt.write_id;
        let receipt_id = receipt.receipt_id;
        let materialization_id = receipt.current.publication.materialization_id;
        let event_id = receipt.current.id();
        tokio::spawn(async move {
            match signer.sign_event(unsigned, cancel).await {
                Ok(event) => {
                    if publication
                        .store
                        .install_signed(write_id, receipt_id, materialization_id, event_id, event)
                        .is_err()
                    {
                        let _ = publication.store.record_signer_refusal(
                            write_id,
                            receipt_id,
                            materialization_id,
                            event_id,
                            "signer returned an event that did not match the accepted body"
                                .to_owned(),
                        );
                    }
                }
                Err(SignerError::Cancelled) => {}
                Err(error) => {
                    let _ = publication.store.record_signer_refusal(
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        error.to_string(),
                    );
                }
            }
        });
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
