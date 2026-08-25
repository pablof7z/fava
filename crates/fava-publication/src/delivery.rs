use std::collections::BTreeMap;
use std::time::Duration;

use fava_delivery::{DeliveryDecision, DeliveryFacts};
use fava_publisher::{PublishAttempt, PublishOutcome};
use fava_relay::RelaySessionKey;
use fava_write::{
    EventId, EventValue, MaterializationId, Receipt, ReceiptId, RelayDeliveryOutcome, WriteId,
};
use tokio::sync::{mpsc, watch};

use super::Publication;

const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);

impl Publication {
    pub(super) fn start_lanes(
        &self,
        receipt: &Receipt,
        active: &mut BTreeMap<
            RelaySessionKey,
            (WriteId, ReceiptId, MaterializationId, EventId, u64),
        >,
        finished: &mpsc::Sender<(
            RelaySessionKey,
            WriteId,
            ReceiptId,
            MaterializationId,
            EventId,
            u64,
        )>,
        cancel: &watch::Receiver<bool>,
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
            ) || active.contains_key(session)
            {
                continue;
            }
            let publication = self.clone();
            let session = session.clone();
            let finished = finished.clone();
            let mut cancel = cancel.clone();
            let write_id = receipt.write_id;
            let receipt_id = receipt.receipt_id;
            let materialization_id = receipt.current.publication.materialization_id;
            let event_id = receipt.current.id();
            let route_revision = receipt.route_revision;
            active.insert(
                session.clone(),
                (
                    write_id,
                    receipt_id,
                    materialization_id,
                    event_id,
                    route_revision,
                ),
            );
            tokio::spawn(async move {
                let lane_cancel = cancel.clone();
                publication
                    .run_destination(
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        session.clone(),
                        lane_cancel,
                    )
                    .await;
                tokio::select! {
                    result = finished.send((
                        session,
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        route_revision,
                    )) => { let _ = result; }
                    changed = cancel.changed() => { let _ = changed; }
                }
            });
        }
    }

    async fn run_destination(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: RelaySessionKey,
        mut cancel: watch::Receiver<bool>,
    ) {
        loop {
            let Some(receipt) = self.read_receipt(receipt_id, &mut cancel).await else {
                return;
            };
            if receipt.write_id != write_id
                || receipt.current.publication.materialization_id != materialization_id
                || receipt.current.id() != event_id
                || !receipt.desires(&session)
            {
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
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        &session,
                        attempts,
                        RelayDeliveryOutcome::GivenUp { reason },
                    );
                    return;
                }
                DeliveryDecision::AttemptNow => {
                    self.attempt(
                        write_id,
                        receipt_id,
                        materialization_id,
                        event_id,
                        &session,
                        attempts,
                    )
                    .await;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn attempt(
        &self,
        write_id: WriteId,
        receipt_id: ReceiptId,
        materialization_id: MaterializationId,
        event_id: EventId,
        session: &RelaySessionKey,
        prior_attempts: u32,
    ) {
        let Some(attempt_number) = prior_attempts.checked_add(1) else {
            return;
        };
        let Ok(receipt) = self.store.begin_attempt(
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            session,
            attempt_number,
        ) else {
            return;
        };
        let EventValue::Signed(event) = receipt.current.event.clone() else {
            return;
        };
        let attempt = PublishAttempt {
            write_id: receipt.write_id,
            receipt_id,
            materialization_id,
            number: attempt_number,
            session: session.clone(),
            event,
            timeout: ATTEMPT_TIMEOUT,
        };
        let outcome = self
            .publisher
            .publish(attempt, self.transport.as_ref())
            .await;
        let _ = self.store.record_outcome(
            write_id,
            receipt_id,
            materialization_id,
            event_id,
            session,
            attempt_number,
            delivery_outcome(outcome),
        );
    }
}

fn delivery_outcome(outcome: PublishOutcome) -> RelayDeliveryOutcome {
    match outcome {
        PublishOutcome::Acknowledged { message } => RelayDeliveryOutcome::Acknowledged { message },
        PublishOutcome::Rejected { message } => RelayDeliveryOutcome::Rejected { message },
        // The owner records only what the publisher observed. `GivenUp` is a policy
        // noun the owner must never invent, and this outcome is reached after handoff,
        // so it cannot be reported as a definite pre-handoff failure.
        PublishOutcome::AuthenticationRequired => RelayDeliveryOutcome::AuthenticationDenied {
            reason: "relay demanded authentication this attempt did not satisfy".to_owned(),
        },
        PublishOutcome::NotHandedOff { reason } => RelayDeliveryOutcome::Retryable { reason },
        PublishOutcome::OutcomeUnknown { reason } => RelayDeliveryOutcome::Unknown { reason },
    }
}
