//! Accepted-write signing, live routing, and delivery lifecycle.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fava_delivery::DeliveryPolicy;
use fava_publisher::Publisher;
use fava_query::{QueryEvaluator, QuerySource};
use fava_routing::Router;
use fava_signer::Signer;
use fava_transport::Transport;
use fava_write::{
    Kind, PublicKey, Receipt, ReceiptId, ReceiptOutcome, ReplaceableEventMaterializer, WriteIntent,
    WritePayload, WriteRouting,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use thiserror::Error;
use tokio::sync::watch;

mod delivery;
mod materialization;
mod run;

use materialization::{PreparedSemantic, SemanticState};

/// Live owner for accepted write signing and destination delivery.
#[derive(Clone)]
pub struct Publication {
    store: Arc<dyn WriteStore>,
    event_source: Arc<dyn QuerySource>,
    evaluator: Arc<dyn QueryEvaluator>,
    materializers: Arc<BTreeMap<Kind, Arc<dyn ReplaceableEventMaterializer>>>,
    signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>,
    publisher: Arc<dyn Publisher>,
    delivery: Arc<dyn DeliveryPolicy>,
    transport: Arc<dyn Transport>,
    routers: Arc<Vec<Arc<dyn Router>>>,
    cancellations: Arc<Mutex<BTreeMap<ReceiptId, watch::Sender<bool>>>>,
}

impl Publication {
    /// Assemble one publication owner from independently selected providers.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] for duplicate signer public keys.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn WriteStore>,
        event_source: Arc<dyn QuerySource>,
        evaluator: Arc<dyn QueryEvaluator>,
        materializers: impl IntoIterator<Item = Arc<dyn ReplaceableEventMaterializer>>,
        signers: impl IntoIterator<Item = Arc<dyn Signer>>,
        publisher: Arc<dyn Publisher>,
        delivery: Arc<dyn DeliveryPolicy>,
        transport: Arc<dyn Transport>,
        routers: Vec<Arc<dyn Router>>,
    ) -> Result<Self, PublicationError> {
        let materializers = Self::index_materializers(materializers)?;
        let mut indexed = BTreeMap::new();
        for signer in signers {
            let public_key = signer.public_key();
            if indexed.insert(public_key, signer).is_some() {
                return Err(PublicationError::DuplicateSigner(public_key));
            }
        }
        Ok(Self {
            store,
            event_source,
            evaluator,
            materializers: Arc::new(materializers),
            signers: Arc::new(indexed),
            publisher,
            delivery,
            transport,
            routers: Arc::new(routers),
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
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
        if let WritePayload::Edit(edit) = intent.payload() {
            let edit = edit.clone();
            let reservation = self.store.reserve_active()?;
            let PreparedSemantic {
                event,
                source,
                route,
                mut sources,
            } = match self.prepare_semantic(&intent, None, None) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = self.store.release_active(reservation);
                    return Err(error);
                }
            };
            let accepted = match self.store.accept_reserved_materialized_edit(
                reservation,
                intent.clone(),
                event,
                source.as_ref(),
            ) {
                Ok(accepted) => accepted,
                Err(error) => {
                    sources.close();
                    return Err(error.into());
                }
            };
            if matches!(intent.routing(), WriteRouting::Automatic) {
                let _ = self.store.apply_route(
                    accepted.write_id,
                    accepted.receipt_id,
                    accepted.current.publication.materialization_id,
                    accepted.current.id(),
                    &route,
                );
            }
            let semantic = SemanticState::accepted(edit, source.as_ref(), sources);
            self.start_semantic(accepted.receipt_id, semantic);
            return Ok(accepted);
        }
        let accepted = self.store.accept(intent)?;
        self.start(accepted.receipt_id);
        Ok(accepted)
    }

    /// Resume every durable open obligation after store recovery.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when recovery cannot read or start work.
    pub fn recover(&self) -> Result<usize, PublicationError> {
        tokio::runtime::Handle::try_current().map_err(|_| PublicationError::RuntimeUnavailable)?;
        let receipts = self.store.recover_open()?;
        let count = receipts.len();
        let semantic = self.store.recover_materialized_edits()?;
        for (_, edit, _, _) in &semantic {
            self.materializer(edit)?;
        }
        let mut prepared: Vec<(ReceiptId, SemanticState)> = Vec::with_capacity(semantic.len());
        for (receipt, edit, selected, failed_id) in semantic {
            let sources = match self.open_semantic_sources(&edit) {
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
                SemanticState::recovered(edit, selected_id, source_floor, failed_id, sources),
            ));
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

    /// Preview the exact current semantic materialization route without custody.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] when selection, materialization, or routing refuses.
    pub fn preview_semantic_routes(
        &self,
        intent: &WriteIntent,
    ) -> Result<fava_routing::RoutePlan, PublicationError> {
        let mut prepared = self.prepare_semantic(intent, None, None)?;
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
        let mut changes = self.store.receipt_changes();
        loop {
            let receipt = self
                .store
                .receipt(receipt_id)?
                .ok_or(PublicationError::ReceiptMissing(receipt_id))?;
            if receipt.is_terminal() {
                return Ok(receipt);
            }
            match changes.recv().await {
                Ok((changed_id, None)) if changed_id == receipt_id => {
                    return Err(PublicationError::ReceiptMissing(receipt_id));
                }
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Err(PublicationError::ReceiptChangesClosed);
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
    /// Two signer providers claimed one public key.
    #[error("duplicate signer for {0}")]
    DuplicateSigner(PublicKey),
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
