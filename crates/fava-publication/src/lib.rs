//! Accepted-write signing, live routing, and delivery lifecycle.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fava_delivery::DeliveryPolicy;
use fava_publisher::Publisher;
use fava_routing::Router;
use fava_signer::Signer;
use fava_transport::Transport;
use fava_write::{PublicKey, Receipt, ReceiptId, ReceiptOutcome, WriteIntent};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use thiserror::Error;
use tokio::sync::watch;

mod run;

/// Live owner for accepted write signing and destination delivery.
#[derive(Clone)]
pub struct Publication {
    store: Arc<dyn WriteStore>,
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
    pub fn new(
        store: Arc<dyn WriteStore>,
        signers: impl IntoIterator<Item = Arc<dyn Signer>>,
        publisher: Arc<dyn Publisher>,
        delivery: Arc<dyn DeliveryPolicy>,
        transport: Arc<dyn Transport>,
        routers: Vec<Arc<dyn Router>>,
    ) -> Result<Self, PublicationError> {
        let mut indexed = BTreeMap::new();
        for signer in signers {
            let public_key = signer.public_key();
            if indexed.insert(public_key, signer).is_some() {
                return Err(PublicationError::DuplicateSigner(public_key));
            }
        }
        Ok(Self {
            store,
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
        for receipt in receipts {
            self.start(receipt.receipt_id);
        }
        Ok(count)
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
