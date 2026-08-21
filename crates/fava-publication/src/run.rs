use std::collections::BTreeSet;
use std::time::Duration;

use fava_delivery::{DeliveryDecision, DeliveryFacts};
use fava_publisher::{PublishAttempt, PublishOutcome};
use fava_routing::{RouteContribution, RoutePlan, RouteRequest, RouterSession};
use fava_signer::{SignerAvailability, SignerError};
use fava_state::RelaySessionKey;
use fava_write::{EventValue, Receipt, ReceiptId, RelayDeliveryOutcome, WriteIntent, WriteRouting};
use tokio::sync::{mpsc, watch};

use super::Publication;
use super::materialization::SemanticState;

const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);

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
        let Some(receipt) = self.store.receipt(receipt_id).ok().flatten() else {
            if let Some(semantic) = &mut semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return;
        };
        let mut routes = self.open_routes(&receipt);
        if matches!(receipt.routing, WriteRouting::Automatic) && routes.is_none() {
            if let Some(semantic) = &mut semantic {
                semantic.close();
            }
            self.finished(receipt_id);
            return;
        }
        let (mut signing_cancel, signing_cancel_rx) = watch::channel(false);
        self.start_signing(&receipt, signing_cancel_rx);
        let mut materialization_id = receipt.current.publication.materialization_id;

        let mut receipt_changes = self.store.receipt_changes();
        let (lane_finished, mut finished_lanes) = mpsc::unbounded_channel();
        let mut active = BTreeSet::new();
        let mut route_revision = self
            .store
            .receipt(receipt_id)
            .ok()
            .flatten()
            .map_or(0, |receipt| receipt.route_revision);

        loop {
            let Some(current) = self.store.receipt(receipt_id).ok().flatten() else {
                break;
            };
            self.start_lanes(&current, &mut active, &lane_finished);
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
                    route_revision = route_revision.saturating_add(1);
                    let request = RouteRequest::Write(current.current.event.clone());
                    self.apply_route(receipt_id, route_revision, &request, &contribution);
                }
                source_open = next_semantic_source(&mut semantic), if semantic.is_some() => {
                    if source_open {
                        if let Some(state) = &mut semantic {
                            self.rematerialize(&current, state);
                        }
                    } else if let Some(mut state) = semantic.take() {
                        state.close();
                    }
                }
                change = receipt_changes.recv() => {
                    match change {
                        Ok((changed, None)) if changed == receipt_id => break,
                        Ok((changed, Some(latest))) if changed == receipt_id => {
                            let next_materialization =
                                latest.current.publication.materialization_id;
                            if next_materialization != materialization_id {
                                signing_cancel.send_replace(true);
                                let (next_cancel, next_cancel_rx) = watch::channel(false);
                                signing_cancel = next_cancel;
                                self.start_signing(&latest, next_cancel_rx);
                                if let Some(open) = &mut routes {
                                    open.close();
                                }
                                routes = self.open_routes(&latest);
                                route_revision = latest.route_revision;
                                materialization_id = next_materialization;
                            }
                        }
                        Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
                Some(session) = finished_lanes.recv() => {
                    active.remove(&session);
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

    fn rematerialize(&self, receipt: &Receipt, state: &mut SemanticState) {
        let expected = receipt.current.publication.materialization_id;
        let expected_source = receipt.current.publication.materialization_source;
        let Ok((true, successor)) = self.semantic_successor(state, receipt.receipt_id, expected)
        else {
            return;
        };
        let Ok(intent) = WriteIntent::edit(state.edit.clone(), receipt.routing.clone()) else {
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
        let Ok(installed) = self.store.install_materialization(
            receipt.write_id,
            receipt.receipt_id,
            expected,
            expected_source,
            event,
            successor.as_ref(),
        ) else {
            return;
        };
        if matches!(receipt.routing, WriteRouting::Automatic) {
            let Some(revision) = installed.route_revision.checked_add(1) else {
                return;
            };
            route.revision = revision;
            let _ = self.store.apply_route(receipt.receipt_id, &route);
        }
        state.selected_id = successor.as_ref().map(|event| event.id);
        if let Some(source) = &successor {
            state.source_floor = Some(source.created_at);
        }
        state.failed_id = None;
    }

    fn open_routes(&self, receipt: &Receipt) -> Option<Box<dyn RouterSession>> {
        let WriteRouting::Automatic = receipt.routing else {
            return None;
        };
        let request = RouteRequest::Write(receipt.current.event.clone());
        match fava_routing::open(self.routers.as_slice(), &request) {
            Ok(routes) => {
                let revision = receipt.route_revision.saturating_add(1);
                self.apply_route(receipt.receipt_id, revision, &request, &routes.current());
                Some(routes)
            }
            Err(error) => {
                let plan = RoutePlan::shortfall(
                    receipt.route_revision.saturating_add(1),
                    &request,
                    error.to_string(),
                );
                let _ = self.store.apply_route(receipt.receipt_id, &plan);
                None
            }
        }
    }

    fn apply_route(
        &self,
        receipt_id: ReceiptId,
        revision: u64,
        request: &RouteRequest,
        contribution: &RouteContribution,
    ) {
        let plan = match RoutePlan::from_contribution(revision, contribution) {
            Ok(plan) => plan,
            Err(error) => RoutePlan::shortfall(revision, request, error.to_string()),
        };
        if let Err(error) = self.store.apply_route(receipt_id, &plan) {
            let shortfall = RoutePlan::shortfall(revision, request, error.to_string());
            let _ = self.store.apply_route(receipt_id, &shortfall);
        }
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
        let receipt_id = receipt.receipt_id;
        tokio::spawn(async move {
            let expected = unsigned.clone();
            match signer.sign_event(unsigned, cancel).await {
                Ok(event) => {
                    if publication.store.install_signed(receipt_id, event).is_err() {
                        publication.record_current_signer_refusal(
                            receipt_id,
                            &expected,
                            "signer returned an event that did not match the accepted body"
                                .to_owned(),
                        );
                    }
                }
                Err(SignerError::Cancelled) => {}
                Err(error) => {
                    publication.record_current_signer_refusal(
                        receipt_id,
                        &expected,
                        error.to_string(),
                    );
                }
            }
        });
    }

    fn record_current_signer_refusal(
        &self,
        receipt_id: ReceiptId,
        expected: &fava_write::UnsignedEvent,
        reason: String,
    ) {
        let current_matches = self
            .store
            .receipt(receipt_id)
            .ok()
            .flatten()
            .is_some_and(|receipt| {
                matches!(receipt.current.event, EventValue::Unsigned(current) if current == *expected)
            });
        if current_matches {
            let _ = self.store.record_signer_refusal(receipt_id, reason);
        }
    }

    fn start_lanes(
        &self,
        receipt: &Receipt,
        active: &mut BTreeSet<RelaySessionKey>,
        finished: &mpsc::UnboundedSender<RelaySessionKey>,
    ) {
        if !matches!(receipt.current.event, EventValue::Signed(_)) {
            return;
        }
        for session in &receipt.desired_destinations {
            let Some(outcome) = receipt.destinations().get(session) else {
                continue;
            };
            if !matches!(
                outcome,
                RelayDeliveryOutcome::Pending | RelayDeliveryOutcome::Retryable { .. }
            ) || !active.insert(session.clone())
            {
                continue;
            }
            let publication = self.clone();
            let session = session.clone();
            let finished = finished.clone();
            let receipt_id = receipt.receipt_id;
            tokio::spawn(async move {
                publication
                    .run_destination(receipt_id, session.clone())
                    .await;
                let _ = finished.send(session);
            });
        }
    }

    async fn run_destination(&self, receipt_id: ReceiptId, session: RelaySessionKey) {
        loop {
            let Some(receipt) = self.store.receipt(receipt_id).ok().flatten() else {
                return;
            };
            if !receipt.desires(&session) {
                return;
            }
            let Some(outcome) = receipt.destinations().get(&session) else {
                return;
            };
            let attempts = receipt.attempts.get(&session).copied().unwrap_or(0);
            match self.delivery.decide(DeliveryFacts { attempts, outcome }) {
                DeliveryDecision::Settled => return,
                DeliveryDecision::GiveUp { reason } => {
                    let _ = self.store.record_outcome(
                        receipt_id,
                        &session,
                        RelayDeliveryOutcome::GivenUp { reason },
                    );
                    return;
                }
                DeliveryDecision::AttemptNow => self.attempt(receipt_id, &session).await,
            }
        }
    }

    async fn attempt(&self, receipt_id: ReceiptId, session: &RelaySessionKey) {
        let Ok(receipt) = self.store.begin_attempt(receipt_id, session) else {
            return;
        };
        let EventValue::Signed(event) = receipt.current.event.clone() else {
            return;
        };
        let attempt = PublishAttempt {
            write_id: receipt.write_id,
            receipt_id,
            number: receipt.attempts.get(session).copied().unwrap_or(0),
            session: session.clone(),
            event,
            timeout: ATTEMPT_TIMEOUT,
        };
        let outcome = self
            .publisher
            .publish(attempt, self.transport.as_ref())
            .await;
        let _ = self
            .store
            .record_outcome(receipt_id, session, delivery_outcome(outcome));
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

async fn next_semantic_source(semantic: &mut Option<SemanticState>) -> bool {
    match semantic {
        Some(semantic) => semantic.sources.next_change().await,
        None => std::future::pending().await,
    }
}

fn delivery_outcome(outcome: PublishOutcome) -> RelayDeliveryOutcome {
    match outcome {
        PublishOutcome::Acknowledged { message } => RelayDeliveryOutcome::Acknowledged { message },
        PublishOutcome::Rejected { message } => RelayDeliveryOutcome::Rejected { message },
        PublishOutcome::AuthenticationRequired => RelayDeliveryOutcome::GivenUp {
            reason: "relay authentication required".to_owned(),
        },
        PublishOutcome::NotHandedOff { reason } => RelayDeliveryOutcome::Retryable { reason },
        PublishOutcome::OutcomeUnknown { reason } => RelayDeliveryOutcome::Unknown { reason },
    }
}
