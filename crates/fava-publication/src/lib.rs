//! Accepted-write signing and explicit-route delivery lifecycle.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_delivery::{DeliveryDecision, DeliveryFacts, DeliveryPolicy};
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_state::RelaySessionKey;
use fava_transport::Transport;
use fava_write::{
    EventValue, PublicKey, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, WriteIntent,
    WriteRouting,
};
use fava_write_store::{AcceptedWrite, WriteStore, WriteStoreError};
use thiserror::Error;
use tokio::sync::watch;

const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);

/// Live owner for accepted write signing and destination delivery.
#[derive(Clone)]
pub struct Publication {
    store: Arc<dyn WriteStore>,
    signers: Arc<BTreeMap<PublicKey, Arc<dyn Signer>>>,
    publisher: Arc<dyn Publisher>,
    delivery: Arc<dyn DeliveryPolicy>,
    transport: Arc<dyn Transport>,
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
            cancellations: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    /// Durably accept one explicit-route write, then begin remaining work.
    ///
    /// # Errors
    ///
    /// Returns [`PublicationError`] before custody for unsupported routing or
    /// no runtime, or after a failed acceptance commit.
    pub fn accept(&self, intent: WriteIntent) -> Result<AcceptedWrite, PublicationError> {
        if matches!(intent.routing(), WriteRouting::Automatic) {
            return Err(PublicationError::AutomaticRoutingUnavailable);
        }
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

    fn start(&self, receipt_id: ReceiptId) {
        let (cancel, cancel_rx) = watch::channel(false);
        let mut cancellations = self
            .cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cancellations.contains_key(&receipt_id) {
            return;
        }
        cancellations.insert(receipt_id, cancel);
        drop(cancellations);
        let publication = self.clone();
        tokio::spawn(async move { publication.run(receipt_id, cancel_rx).await });
    }

    async fn run(self, receipt_id: ReceiptId, cancel: watch::Receiver<bool>) {
        let Some(mut receipt) = self.store.receipt(receipt_id).ok().flatten() else {
            self.finished(receipt_id);
            return;
        };
        if let EventValue::Unsigned(unsigned) = receipt.current.event.clone() {
            let Some(signer) = self.signers.get(&unsigned.pubkey) else {
                self.finished(receipt_id);
                return;
            };
            if !matches!(signer.availability(), SignerAvailability::Available) {
                self.finished(receipt_id);
                return;
            }
            match signer.sign_event(unsigned, cancel).await {
                Ok(event) => {
                    let Ok(updated) = self.store.install_signed(receipt_id, event) else {
                        let _ = self.store.record_signer_refusal(
                            receipt_id,
                            "signer returned an event that did not match the accepted body"
                                .to_owned(),
                        );
                        self.finished(receipt_id);
                        return;
                    };
                    receipt = updated;
                }
                Err(SignerError::Cancelled) => {
                    self.finished(receipt_id);
                    return;
                }
                Err(error) => {
                    let _ = self
                        .store
                        .record_signer_refusal(receipt_id, error.to_string());
                    self.finished(receipt_id);
                    return;
                }
            }
        }
        for session in receipt.destinations().keys().cloned().collect::<Vec<_>>() {
            let publication = self.clone();
            tokio::spawn(async move { publication.run_destination(receipt_id, session).await });
        }
    }

    async fn run_destination(self, receipt_id: ReceiptId, session: RelaySessionKey) {
        loop {
            let Some(receipt) = self.store.receipt(receipt_id).ok().flatten() else {
                return;
            };
            let Some(outcome) = receipt.destinations().get(&session) else {
                return;
            };
            let attempts = receipt.attempts.get(&session).copied().unwrap_or(0);
            match self.delivery.decide(DeliveryFacts { attempts, outcome }) {
                DeliveryDecision::Settled => break,
                DeliveryDecision::GiveUp { reason } => {
                    let _ = self.store.record_outcome(
                        receipt_id,
                        &session,
                        RelayDeliveryOutcome::GivenUp { reason },
                    );
                    break;
                }
                DeliveryDecision::AttemptNow => {
                    let Ok(receipt) = self.store.begin_attempt(receipt_id, &session) else {
                        break;
                    };
                    let EventValue::Signed(event) = receipt.current.event.clone() else {
                        break;
                    };
                    let attempt = PublishAttempt {
                        write_id: receipt.write_id,
                        receipt_id,
                        number: receipt.attempts.get(&session).copied().unwrap_or(0),
                        session: session.clone(),
                        event,
                        timeout: ATTEMPT_TIMEOUT,
                    };
                    let outcome = self
                        .publisher
                        .publish(attempt, self.transport.as_ref())
                        .await;
                    let outcome = delivery_outcome(outcome);
                    if self
                        .store
                        .record_outcome(receipt_id, &session, outcome)
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        if self
            .store
            .receipt(receipt_id)
            .ok()
            .flatten()
            .is_some_and(|receipt| receipt.is_terminal())
        {
            self.finished(receipt_id);
        }
    }

    fn finished(&self, receipt_id: ReceiptId) {
        self.cancellations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&receipt_id);
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

/// Publication lifecycle refusal.
#[derive(Debug, Error)]
pub enum PublicationError {
    /// This Fava assembly did not select publication providers.
    #[error("publication is not configured")]
    NotConfigured,
    /// Automatic write routing begins in the next milestone.
    #[error("automatic write routing is not available in this assembly")]
    AutomaticRoutingUnavailable,
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
    /// Durable write-store operation failed.
    #[error(transparent)]
    Store(#[from] WriteStoreError),
}
