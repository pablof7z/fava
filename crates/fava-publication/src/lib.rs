//! Accepted-write signing, live routing, and delivery lifecycle.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_delivery::DeliveryPolicy;
use fava_publisher::Publisher;
use fava_query::{QueryEvaluator, QuerySource};
use fava_routing::Router;
use fava_session::Session;
use fava_transport::Transport;
use fava_write::{
    EditApplier, EventId, Kind, Receipt, ReceiptId, ReceiptOutcome, RevisionId, WriteId,
    WriteIntent, WritePayload, WriteRouting,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use thiserror::Error;
use tokio::sync::watch;

mod delivery;
mod edit_application;
mod generation;
mod recovery;
mod run;
mod semantic_refresh;
mod sign;

use edit_application::{PreparedSemantic, SemanticState};

const STORE_READ_RETRY_DELAY: Duration = Duration::from_millis(10);

/// Live owner for accepted write signing and destination delivery.
#[derive(Clone)]
pub struct Publication {
    store: Arc<dyn WriteStore>,
    event_source: Arc<dyn QuerySource>,
    evaluator: Arc<dyn QueryEvaluator>,
    appliers: Arc<BTreeMap<Kind, Arc<dyn EditApplier>>>,
    session: Session,
    publisher: Arc<dyn Publisher>,
    delivery: Arc<dyn DeliveryPolicy>,
    transport: Arc<dyn Transport>,
    routers: Arc<Vec<Arc<dyn Router>>>,
    /// What the authentication owner determined about each relay session, when
    /// one was assembled. Read, never derived: an attempt whose challenge is
    /// still awaiting a person has not been denied.
    authentication: Option<Arc<dyn fava_relay::AuthenticationOutcomes>>,
    cancellations: Arc<Mutex<BTreeMap<ReceiptId, watch::Sender<bool>>>>,
    // Rejected late signer completions: receipt, write, generation, event, reason.
    // Deliberately existing identity values rather than a new architectural noun.
    #[allow(clippy::type_complexity)]
    stale_signer_completions:
        Arc<Mutex<VecDeque<(ReceiptId, WriteId, RevisionId, EventId, String)>>>,
}

impl Publication {
    /// Assemble one publication owner from independently selected providers.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] for invalid applier selection.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn WriteStore>,
        event_source: Arc<dyn QuerySource>,
        evaluator: Arc<dyn QueryEvaluator>,
        appliers: impl IntoIterator<Item = Arc<dyn EditApplier>>,
        session: Session,
        publisher: Arc<dyn Publisher>,
        delivery: Arc<dyn DeliveryPolicy>,
        transport: Arc<dyn Transport>,
        routers: Vec<Arc<dyn Router>>,
        authentication: Option<Arc<dyn fava_relay::AuthenticationOutcomes>>,
    ) -> Result<Self, PublicationError> {
        let appliers = Self::index_appliers(appliers)?;
        Ok(Self {
            store,
            authentication,
            event_source,
            evaluator,
            appliers: Arc::new(appliers),
            session,
            publisher,
            delivery,
            transport,
            routers: Arc::new(routers),
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
            stale_signer_completions: Arc::new(Mutex::new(VecDeque::new())),
        })
    }

    /// Durably accept one checked write, then begin remaining work.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] before custody when no runtime exists, or
    /// after a failed acceptance commit.
    pub fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, PublicationError> {
        tokio::runtime::Handle::try_current().map_err(|_| PublicationError::RuntimeUnavailable)?;
        if let WritePayload::Edit { edit, author } = intent.payload() {
            let reservation = self.store.reserve_active(edit, *author)?;
            let PreparedSemantic {
                event,
                source,
                route,
                mut sources,
            } = match self.prepare_semantic(&intent, None) {
                Ok(prepared) => prepared,
                Err(error) => {
                    if let Err(release) = self.store.release_active(reservation) {
                        return Err(PublicationError::Store(WriteStoreError::Refused(format!(
                            "semantic preparation failed ({error}); active reservation release also failed ({release})"
                        ))));
                    }
                    return Err(error);
                }
            };
            let initial_route =
                matches!(intent.routing(), WriteRouting::Automatic).then_some(&route);
            let accepted = match self.store.accept_reserved_applied_edit(
                reservation,
                intent.clone(),
                event,
                source.as_ref(),
                initial_route,
            ) {
                Ok(accepted) => accepted,
                Err(error) => {
                    sources.close();
                    return Err(error.into());
                }
            };
            let Some((edits, author, selected, failed_id)) = self.store.applied_edits(
                accepted.receipt_id,
                accepted.current.publication.revision_id,
            )?
            else {
                let terminal = self
                    .store
                    .receipt(accepted.receipt_id)?
                    .is_some_and(|receipt| receipt.is_terminal());
                sources.close();
                if terminal {
                    return Ok(accepted);
                }
                return Err(PublicationError::Store(WriteStoreError::Refused(
                    "accepted semantic custody is missing".to_owned(),
                )));
            };
            let semantic = SemanticState::recovered(
                edits,
                author,
                accepted.current.publication.revision_id,
                selected.map(|(id, _)| id),
                selected.map(|(_, timestamp)| timestamp),
                failed_id,
                sources,
            );
            self.start_semantic(accepted.receipt_id, semantic);
            return Ok(accepted);
        }
        let accepted = self.store.accept(intent)?;
        self.start(accepted.receipt_id);
        Ok(accepted)
    }

    /// Reconcile every durable open obligation, then resume its remaining work.
    ///
    /// Recovered semantic coordinates apply their complete durable sequence to
    /// the initial qualified source snapshot before this method returns. A
    /// caller may therefore expose its publication facade immediately after a
    /// successful return without racing same-coordinate admission.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when recovery cannot read or start work.
    pub fn recover(&self) -> Result<usize, PublicationError> {
        tokio::runtime::Handle::try_current().map_err(|_| PublicationError::RuntimeUnavailable)?;
        let receipts = self.store.recover_open()?;
        let count = receipts.len();
        let semantic = self.store.recover_applied_edits()?;
        for (_, edits, _, _, _) in &semantic {
            for edit in edits {
                self.applier(edit)?;
            }
        }
        let mut prepared: Vec<(ReceiptId, SemanticState)> = Vec::with_capacity(semantic.len());
        for (receipt, edits, author, selected, failed_id) in semantic {
            let edit = edits.last().ok_or_else(|| {
                PublicationError::Store(WriteStoreError::Refused(
                    "recovered semantic edit sequence is empty".to_owned(),
                ))
            })?;
            let sources = match self.open_semantic_sources(edit, author) {
                Ok(sources) => sources,
                Err(error) => {
                    for (_, state) in &mut prepared {
                        state.close();
                    }
                    return Err(error);
                }
            };
            let selected_id = selected.map(|(id, _)| id);
            let source_floor = selected.map(|(_, timestamp)| timestamp);
            prepared.push((
                receipt.receipt_id,
                SemanticState::recovered(
                    edits,
                    author,
                    receipt.current.publication.revision_id,
                    selected_id,
                    source_floor,
                    failed_id,
                    sources,
                ),
            ));
        }
        for index in 0..prepared.len() {
            let (receipt_id, state) = &mut prepared[index];
            if let Err(error) = self.reconcile_recovered(*receipt_id, state) {
                for (_, state) in &mut prepared {
                    state.close();
                }
                return Err(error);
            }
        }
        let semantic_ids: std::collections::BTreeSet<_> =
            prepared.iter().map(|(receipt_id, _)| *receipt_id).collect();
        for (receipt_id, state) in prepared {
            self.start_semantic(receipt_id, state);
        }
        for receipt in receipts {
            if !semantic_ids.contains(&receipt.receipt_id) {
                self.start(receipt.receipt_id);
            }
        }
        Ok(count)
    }

    /// Preview the exact current semantic revision route without custody.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when selection, revision, or routing refuses.
    pub fn preview_semantic_routes(
        &self,
        intent: &WriteIntent,
    ) -> Result<fava_routing::RoutePlan, PublicationError> {
        let mut prepared = self.prepare_semantic(intent, None)?;
        prepared.sources.close();
        Ok(prepared.route)
    }

    /// Cancel while every selected destination is definitely pre-handoff.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when cancellation is ineligible or cannot commit.
    pub fn cancel(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, PublicationError> {
        let receipt = self.store.cancel(receipt_id)?;
        if receipt
            .as_ref()
            .is_some_and(|receipt| matches!(receipt.outcome, ReceiptOutcome::Cancelled))
            && let Some(cancel) = self
                .cancellations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&receipt_id)
        {
            cancel.send_replace(true);
        }
        Ok(receipt)
    }

    /// Read current receipt facts.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when the write store cannot read.
    pub fn receipt(&self, receipt_id: ReceiptId) -> Result<Option<Receipt>, PublicationError> {
        self.store.receipt(receipt_id).map_err(Into::into)
    }

    /// Remove a retained terminal receipt independently of cancellation.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] for active work or failed store mutation.
    pub fn remove_receipt(&self, receipt_id: ReceiptId) -> Result<bool, PublicationError> {
        self.store.remove_receipt(receipt_id).map_err(Into::into)
    }

    /// Await the exact terminal current receipt.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when the receipt disappears or cannot be read.
    pub async fn wait_terminal(&self, receipt_id: ReceiptId) -> Result<Receipt, PublicationError> {
        self.wait_until(receipt_id, Receipt::is_terminal)
            .await
            .map(|(receipt, _)| receipt)
    }

    /// Await a caller-selected receipt predicate or bounded receipt terminality.
    ///
    /// The returned boolean is true when the predicate accepted the returned
    /// complete durable receipt and false when terminality ended the wait first.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when the receipt disappears, storage
    /// cannot be read, or the receipt-change stream closes.
    pub async fn wait_until<F>(
        &self,
        receipt_id: ReceiptId,
        predicate: F,
    ) -> Result<(Receipt, bool), PublicationError>
    where
        F: Fn(&Receipt) -> bool,
    {
        let mut changes = self.store.receipt_changes();
        loop {
            let receipt = self
                .store
                .receipt(receipt_id)?
                .ok_or(PublicationError::ReceiptMissing(receipt_id))?;
            if predicate(&receipt) {
                return Ok((receipt, true));
            }
            if receipt.is_terminal() {
                return Ok((receipt, false));
            }

            loop {
                match changes.recv().await {
                    Ok((changed_id, None)) if changed_id == receipt_id => {
                        return Err(PublicationError::ReceiptMissing(receipt_id));
                    }
                    Ok((changed_id, Some(_))) if changed_id == receipt_id => break,
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return Err(PublicationError::ReceiptChangesClosed);
                    }
                }
            }
        }
    }
}

/// Publication lifecycle refusal.
#[derive(Debug, Error)]
pub enum PublicationError {
    /// This Fava assembly did not select publication providers.
    #[error("publication is not configured")]
    NotConfigured,
    /// Publication work requires a running Tokio runtime.
    #[error("publication requires a running Tokio runtime")]
    RuntimeUnavailable,
    /// Requested receipt does not exist.
    #[error("receipt {0:?} does not exist")]
    ReceiptMissing(ReceiptId),
    /// The write store ended receipt-change delivery.
    #[error("receipt change delivery closed")]
    ReceiptChangesClosed,
    /// Current routing facts or configuration were refused.
    #[error("publication routing refused: {0}")]
    Routing(String),
    /// Durable write-store operation failed.
    #[error(transparent)]
    Store(#[from] WriteStoreError),
}
