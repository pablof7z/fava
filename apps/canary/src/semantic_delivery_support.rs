//! Deterministic held delivery for semantic rematerialization evidence.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use fava::{Fava, MaterializationId, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome};
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_transport::Transport;
use fava_write::EventId;
use tokio::sync::{mpsc, oneshot};

use crate::{CanaryError, CanaryResult};

const DELIVERY_REQUEST_CAPACITY: usize = 2;

type DeliveryResponse = oneshot::Sender<PublishOutcome>;

pub(super) struct PendingDelivery {
    pub(super) attempt: PublishAttempt,
    response: DeliveryResponse,
}

impl PendingDelivery {
    pub(super) fn complete(self, outcome: PublishOutcome) -> CanaryResult<()> {
        self.response
            .send(outcome)
            .map_err(|_| CanaryError::new("delivery attempt no longer awaited its completion"))
    }
}

pub(super) struct GatePublisher {
    requests: mpsc::Sender<PendingDelivery>,
}

impl GatePublisher {
    pub(super) fn new() -> (Self, mpsc::Receiver<PendingDelivery>) {
        let (requests, receiver) = mpsc::channel(DELIVERY_REQUEST_CAPACITY);
        (Self { requests }, receiver)
    }
}

impl Publisher for GatePublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        Box::pin(async move {
            let (response, completion) = oneshot::channel();
            if let Err(error) = self
                .requests
                .try_send(PendingDelivery { attempt, response })
            {
                return PublishOutcome::NotHandedOff {
                    reason: format!("delivery request refused: {error}"),
                };
            }
            completion.await.unwrap_or(PublishOutcome::OutcomeUnknown {
                reason: "delivery completion channel closed".to_owned(),
            })
        })
    }
}

pub(super) async fn next_delivery(
    requests: &mut mpsc::Receiver<PendingDelivery>,
) -> CanaryResult<PendingDelivery> {
    tokio::time::timeout(Duration::from_secs(5), requests.recv())
        .await
        .map_err(|_| CanaryError::new("timed out awaiting exact delivery attempt"))?
        .ok_or_else(|| CanaryError::new("delivery request channel closed"))
}

pub(super) fn exact_receipt(fava: &Fava, receipt_id: ReceiptId) -> CanaryResult<Receipt> {
    fava.receipt(receipt_id)
        .map_err(|error| CanaryError::new(error.to_string()))?
        .ok_or_else(|| CanaryError::new("accepted semantic receipt disappeared"))
}

pub(super) fn require_generation_two_pending(
    receipt: &Receipt,
    event_id: EventId,
) -> CanaryResult<()> {
    if receipt.current.publication.materialization_id != MaterializationId::from_u64(2)
        || receipt.current.id() != event_id
        || receipt.outcome != ReceiptOutcome::Open
        || !receipt.attempts.is_empty()
        || receipt.current.publication.destinations.len() != 1
        || !receipt
            .current
            .publication
            .destinations
            .values()
            .all(|outcome| matches!(outcome, RelayDeliveryOutcome::Pending))
    {
        return Err(CanaryError::new(
            "generation two was not exact and pending behind retired delivery",
        ));
    }
    Ok(())
}

pub(super) fn require_exact_attempt_progress(
    pending: &Receipt,
    attempting: &Receipt,
    attempt: &PublishAttempt,
) -> CanaryResult<bool> {
    let mut expected = pending.clone();
    expected
        .attempts
        .insert(attempt.session.clone(), attempt.number);
    expected
        .current
        .publication
        .destinations
        .insert(attempt.session.clone(), RelayDeliveryOutcome::Attempting);
    if attempt.materialization_id != MaterializationId::from_u64(2)
        || attempt.event.id != pending.current.id()
        || attempt.number != 1
        || expected != *attempting
    {
        return Err(CanaryError::new(
            "retired delivery completion contaminated generation-two attempt evidence",
        ));
    }
    Ok(true)
}

pub(super) fn require_exact_terminal_progress(
    attempting: &Receipt,
    terminal: &Receipt,
) -> CanaryResult<()> {
    let session = attempting
        .desired_destinations
        .iter()
        .next()
        .cloned()
        .ok_or_else(|| CanaryError::new("generation-two route disappeared"))?;
    let mut expected = attempting.clone();
    expected.current.publication.destinations.insert(
        session,
        RelayDeliveryOutcome::Acknowledged {
            message: "current generation acknowledgement".to_owned(),
        },
    );
    expected.outcome = ReceiptOutcome::Complete;
    if expected != *terminal {
        return Err(CanaryError::new(
            "generation-two terminal receipt diverged after retired delivery completion",
        ));
    }
    Ok(())
}
