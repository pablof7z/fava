//! Public receipt-summary and write-settlement evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Fava, Kind, PublishError, Receipt, RelayDeliveryOutcome, Write, all_acknowledged, all_terminal,
    at_least,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{
    BoundedReason, OpenRelaySession, RelaySessionFuture, Transport, TransportError,
    TransportFailure, TransportShutdownFuture,
};
use fava_write::{
    EventValue, LocalWriteEvent, MaterializationId, PublicationEvidence, ReceiptId, ReceiptOutcome,
    SignatureState, WriteId, WriteIntent, WriteRouting,
};
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::types::RelayUrl;
use tokio::sync::Notify;

#[test]
fn receipt_counts_preserve_complete_mixed_destination_evidence() {
    let receipt = mixed_receipt();

    assert_eq!(receipt.acknowledged(), 1);
    assert_eq!(receipt.rejected(), 1);
    assert_eq!(receipt.desired(), 3);
    assert_eq!(receipt.destinations().len(), 4);
    assert!(receipt.destinations().values().any(|outcome| matches!(
        outcome,
        RelayDeliveryOutcome::Unknown { reason } if reason == "handoff ambiguous"
    )));
    assert!(
        receipt
            .destinations()
            .values()
            .any(|outcome| matches!(outcome, RelayDeliveryOutcome::Pending))
    );
}

#[test]
fn terminal_and_acknowledged_predicates_expose_distinct_receipt_facts() {
    let mixed = mixed_receipt();
    assert!(all_terminal()(&mixed));
    assert!(!all_acknowledged()(&mixed));

    let acknowledged = mixed
        .desired_destinations
        .iter()
        .find(|session| {
            matches!(
                mixed.destinations().get(*session),
                Some(RelayDeliveryOutcome::Acknowledged { .. })
            )
        })
        .expect("fixture has one acknowledged destination")
        .clone();
    let mut all_acknowledged_receipt = mixed.clone();
    all_acknowledged_receipt.desired_destinations = BTreeSet::from([acknowledged]);
    assert!(all_acknowledged()(&all_acknowledged_receipt));

    let mut no_destination = mixed.clone();
    no_destination.outcome = ReceiptOutcome::NoDestination;
    no_destination.desired_destinations.clear();
    assert!(all_terminal()(&no_destination));
    assert!(!all_acknowledged()(&no_destination));

    let mut missing_fact = all_acknowledged_receipt.clone();
    let missing = missing_fact
        .desired_destinations
        .iter()
        .next()
        .expect("fixture has one desired destination")
        .clone();
    missing_fact
        .current
        .publication
        .destinations
        .remove(&missing);
    assert!(!all_terminal()(&missing_fact));
    assert!(!all_acknowledged()(&missing_fact));

    let mut unsettled = all_acknowledged_receipt;
    unsettled.route_settled = false;
    assert!(!all_terminal()(&unsettled));
    assert!(!all_acknowledged()(&unsettled));
}

#[test]
fn withdrawn_acknowledgement_cannot_mask_current_rejection() {
    let mut receipt = mixed_receipt();
    let rejected = receipt
        .desired_destinations
        .iter()
        .find(|session| {
            matches!(
                receipt.destinations().get(*session),
                Some(RelayDeliveryOutcome::Rejected { .. })
            )
        })
        .expect("fixture has one rejected destination")
        .clone();
    let withdrawn = receipt
        .destinations()
        .iter()
        .find_map(|(session, outcome)| {
            matches!(outcome, RelayDeliveryOutcome::Pending).then(|| session.clone())
        })
        .expect("fixture has one withdrawn destination");
    receipt.current.publication.destinations.insert(
        withdrawn,
        RelayDeliveryOutcome::Acknowledged {
            message: "historical acknowledgement".to_owned(),
        },
    );
    receipt.desired_destinations = BTreeSet::from([rejected]);

    assert_eq!(receipt.acknowledged(), 2);
    assert_eq!(receipt.desired(), 1);
    assert!(!all_acknowledged()(&receipt));
}

#[test]
fn zero_threshold_is_a_typed_refusal_before_waiting() {
    assert!(matches!(
        at_least(0),
        Err(PublishError::InvalidSettlementThreshold)
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn exact_threshold_returns_first_complete_satisfying_revision() {
    let publisher = Arc::new(ManualPublisher::default());
    let (fava, store) = assembly(Arc::clone(&publisher));
    let relays = [
        relay("threshold-a"),
        relay("threshold-b"),
        relay("threshold-c"),
    ];
    let write = publish(&fava, &relays, "exact threshold");
    publisher.wait_started(3).await;
    let mut changes = store.receipt_changes();
    let settlement = tokio::spawn({
        let write = write.clone();
        async move {
            write
                .settled(at_least(2).expect("positive threshold"))
                .await
        }
    });

    publisher.release(
        &relays[0],
        PublishOutcome::Acknowledged {
            message: "first".to_owned(),
        },
    );
    wait_for_counts(&mut changes, write.receipt_id(), 1, 0).await;
    assert!(!settlement.is_finished());
    publisher.release(
        &relays[1],
        PublishOutcome::Acknowledged {
            message: "second".to_owned(),
        },
    );

    let receipt = deadline(settlement)
        .await
        .expect("settlement task joins")
        .expect("threshold is reached");
    assert_eq!(receipt.acknowledged(), 2);
    assert_eq!(receipt.desired(), 3);
    assert_eq!(receipt.outcome, ReceiptOutcome::Open);
    assert_eq!(receipt.destinations().len(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn all_acknowledged_waits_for_every_current_destination() {
    let publisher = Arc::new(ManualPublisher::default());
    let (fava, _) = assembly(Arc::clone(&publisher));
    let relays = [relay("all-acknowledged-a"), relay("all-acknowledged-b")];
    let write = publish(&fava, &relays, "all acknowledged");
    publisher.wait_started(2).await;
    let settlement = tokio::spawn({
        let write = write.clone();
        async move { write.settled(all_acknowledged()).await }
    });

    publisher.release(
        &relays[0],
        PublishOutcome::Acknowledged {
            message: "first".to_owned(),
        },
    );
    tokio::task::yield_now().await;
    assert!(!settlement.is_finished());
    publisher.release(
        &relays[1],
        PublishOutcome::Acknowledged {
            message: "second".to_owned(),
        },
    );

    let receipt = deadline(settlement)
        .await
        .expect("settlement task joins")
        .expect("every current destination acknowledged");
    assert_eq!(receipt.acknowledged(), 2);
    assert_eq!(receipt.desired(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn all_terminal_and_custom_predicates_receive_complete_mixed_revisions() {
    let publisher = Arc::new(ManualPublisher::default());
    let (fava, _) = assembly(Arc::clone(&publisher));
    let relays = [
        relay("all_terminal-ack"),
        relay("all_terminal-reject"),
        relay("all_terminal-unknown"),
    ];
    let all_write = publish(&fava, &relays, "all_terminal mixed");
    let custom_write = publish(&fava, &relays, "custom mixed");
    publisher.wait_started(6).await;
    let evaluations = Arc::new(AtomicUsize::new(0));
    let custom = tokio::spawn({
        let evaluations = Arc::clone(&evaluations);
        async move {
            custom_write
                .settled(move |receipt| {
                    evaluations.fetch_add(1, Ordering::SeqCst);
                    receipt.route_settled
                        && receipt.acknowledged() == 1
                        && receipt.rejected() == 1
                        && receipt
                            .destinations()
                            .values()
                            .any(|outcome| matches!(outcome, RelayDeliveryOutcome::Unknown { .. }))
                })
                .await
        }
    });
    let all_settlement = tokio::spawn(async move { all_write.settled(all_terminal()).await });

    for write_number in 0..2 {
        publisher.release_for_receipt(
            write_number,
            &relays[0],
            PublishOutcome::Acknowledged {
                message: "stored".to_owned(),
            },
        );
        publisher.release_for_receipt(
            write_number,
            &relays[1],
            PublishOutcome::Rejected {
                message: "blocked".to_owned(),
            },
        );
        publisher.release_for_receipt(
            write_number,
            &relays[2],
            PublishOutcome::OutcomeUnknown {
                reason: "ambiguous".to_owned(),
            },
        );
    }

    let all_receipt = deadline(all_settlement)
        .await
        .expect("all_terminal task joins")
        .expect("all_terminal terminal facts satisfy all_terminal");
    let custom_receipt = deadline(custom)
        .await
        .expect("custom task joins")
        .expect("custom mixed predicate succeeds");
    for receipt in [all_receipt, custom_receipt] {
        assert_eq!(receipt.acknowledged(), 1);
        assert_eq!(receipt.rejected(), 1);
        assert_eq!(receipt.desired(), 3);
        assert_eq!(receipt.destinations().len(), 3);
    }
    assert!(evaluations.load(Ordering::SeqCst) >= 2);
}

#[tokio::test(flavor = "current_thread")]
async fn all_acknowledged_not_reached_preserves_every_destination_fact() {
    let publisher = Arc::new(ManualPublisher::default());
    let (fava, _) = assembly(Arc::clone(&publisher));
    let relays = [relay("not-reached-ack"), relay("not-reached-reject")];
    let write = publish(&fava, &relays, "not reached");
    publisher.wait_started(2).await;
    let settlement = tokio::spawn({
        let write = write.clone();
        async move { write.settled(all_acknowledged()).await }
    });

    publisher.release(
        &relays[0],
        PublishOutcome::Acknowledged {
            message: "stored exactly".to_owned(),
        },
    );
    publisher.release(
        &relays[1],
        PublishOutcome::Rejected {
            message: "blocked exactly".to_owned(),
        },
    );

    let error = deadline(settlement)
        .await
        .expect("settlement task joins")
        .expect_err("terminal receipt cannot reach two acknowledgements");
    let PublishError::NotReached { receipt } = error else {
        panic!("unexpected settlement error: {error}");
    };
    assert_eq!(receipt.outcome, ReceiptOutcome::Complete);
    assert_eq!(receipt.acknowledged(), 1);
    assert_eq!(receipt.rejected(), 1);
    assert_eq!(receipt.desired(), 2);
    assert_eq!(receipt.destinations().len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn lagged_and_redundant_notifications_reload_durable_current_receipt() {
    let publisher = Arc::new(ManualPublisher::default());
    let (fava, store) = assembly(Arc::clone(&publisher));
    let target_relay = relay("lag-target");
    let write = publish(&fava, std::slice::from_ref(&target_relay), "lag target");
    publisher.wait_started(1).await;
    let evaluated = Arc::new(Notify::new());
    let evaluations = Arc::new(AtomicUsize::new(0));
    let settlement = tokio::spawn({
        let evaluated = Arc::clone(&evaluated);
        let evaluations = Arc::clone(&evaluations);
        async move {
            write
                .settled(move |receipt| {
                    evaluations.fetch_add(1, Ordering::SeqCst);
                    evaluated.notify_one();
                    receipt.acknowledged() == 1
                })
                .await
        }
    });
    evaluated.notified().await;

    for index in 0..300 {
        let keys = Keys::generate();
        let event = NostrEventBuilder::new(Kind::TextNote, format!("lag flood {index}"))
            .finalize(&keys)
            .expect("flood event signs");
        store
            .accept(WriteIntent::presigned(event, WriteRouting::Automatic).expect("intent"))
            .expect("flood receipt accepts");
    }
    publisher.release(
        &target_relay,
        PublishOutcome::Acknowledged {
            message: "after lag".to_owned(),
        },
    );

    let receipt = deadline(settlement)
        .await
        .expect("settlement task joins")
        .expect("durable reread observes acknowledgement after lag");
    assert_eq!(receipt.acknowledged(), 1);
    assert!(evaluations.load(Ordering::SeqCst) >= 2);
}

fn mixed_receipt() -> Receipt {
    let keys = Keys::generate();
    let event = NostrEventBuilder::new(Kind::TextNote, "mixed receipt")
        .finalize(&keys)
        .expect("event signs");
    let acknowledged = session("acknowledged");
    let rejected = session("rejected");
    let unknown = session("unknown");
    let withdrawn = session("withdrawn-pending");
    let destinations = BTreeMap::from([
        (
            acknowledged.clone(),
            RelayDeliveryOutcome::Acknowledged {
                message: "stored exactly".to_owned(),
            },
        ),
        (
            rejected.clone(),
            RelayDeliveryOutcome::Rejected {
                message: "blocked exactly".to_owned(),
            },
        ),
        (
            unknown.clone(),
            RelayDeliveryOutcome::Unknown {
                reason: "handoff ambiguous".to_owned(),
            },
        ),
        (withdrawn, RelayDeliveryOutcome::Pending),
    ]);
    let write_id = WriteId::try_from(41).expect("nonzero write identity");
    let receipt_id = ReceiptId::try_from(41).expect("nonzero receipt identity");
    let current = LocalWriteEvent::new(
        EventValue::Signed(event),
        PublicationEvidence {
            receipt_id,
            write_id,
            materialization_id: MaterializationId::FIRST,
            materialization_source: None,
            materialization_failure: None,
            retired_materializations: Vec::new(),
            signature: SignatureState::Signed,
            destinations,
        },
    )
    .expect("signed event is query-visible");

    Receipt {
        write_id,
        receipt_id,
        current,
        routing: WriteRouting::Explicit(vec![
            acknowledged.relay.clone(),
            rejected.relay.clone(),
            unknown.relay.clone(),
        ]),
        outcome: ReceiptOutcome::Complete,
        route_revision: 1,
        route_settled: true,
        route_shortfalls: Vec::new(),
        desired_destinations: BTreeSet::from([acknowledged, rejected, unknown]),
        attempts: BTreeMap::new(),
    }
}

fn session(name: &str) -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL"),
        access: RelayAccess::Public,
    }
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn assembly(publisher: Arc<ManualPublisher>) -> (Fava, Arc<MemoryWriteStore>) {
    let store = Arc::new(MemoryWriteStore::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::clone(&store))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .transport(Arc::new(NoopTransport))
        .publisher(publisher)
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("publication assembly");
    (fava, store)
}

fn publish(fava: &Fava, relays: &[RelayUrl], content: &str) -> Write {
    let keys = Keys::generate();
    let event = NostrEventBuilder::new(Kind::TextNote, content)
        .finalize(&keys)
        .expect("event signs");
    fava.to(relays.to_vec())
        .expect("explicit route validates")
        .publish(event)
        .expect("publication accepts")
}

async fn wait_for_counts(
    changes: &mut tokio::sync::broadcast::Receiver<(ReceiptId, Option<Receipt>)>,
    receipt_id: ReceiptId,
    acknowledged: usize,
    rejected: usize,
) {
    loop {
        let (changed_id, receipt) = changes.recv().await.expect("receipt change remains open");
        if changed_id == receipt_id
            && receipt.as_ref().is_some_and(|receipt| {
                receipt.acknowledged() == acknowledged && receipt.rejected() == rejected
            })
        {
            return;
        }
    }
}

async fn deadline<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(1), future)
        .await
        .expect("settlement deadline elapsed")
}

#[derive(Default)]
struct ManualPublisher {
    lanes: Mutex<Vec<(RelaySessionKey, Arc<ManualLane>)>>,
    started: AtomicUsize,
    started_changed: Notify,
}

impl ManualPublisher {
    async fn wait_started(&self, expected: usize) {
        loop {
            let changed = self.started_changed.notified();
            if self.started.load(Ordering::SeqCst) >= expected {
                return;
            }
            changed.await;
        }
    }

    fn release(&self, relay: &RelayUrl, outcome: PublishOutcome) {
        let lane = self
            .lanes
            .lock()
            .expect("lane lock")
            .iter()
            .find(|(session, lane)| {
                session.relay == *relay && lane.outcome.lock().unwrap().is_none()
            })
            .map(|(_, lane)| Arc::clone(lane))
            .expect("pending relay lane exists");
        lane.release(outcome);
    }

    fn release_for_receipt(&self, receipt_index: usize, relay: &RelayUrl, outcome: PublishOutcome) {
        let lane = self
            .lanes
            .lock()
            .expect("lane lock")
            .iter()
            .filter(|(session, _)| session.relay == *relay)
            .nth(receipt_index)
            .map(|(_, lane)| Arc::clone(lane))
            .expect("indexed relay lane exists");
        lane.release(outcome);
    }
}

impl Publisher for ManualPublisher {
    fn publish<'a>(
        &'a self,
        attempt: PublishAttempt,
        _transport: &'a dyn Transport,
    ) -> Pin<Box<dyn Future<Output = PublishOutcome> + Send + 'a>> {
        let lane = Arc::new(ManualLane::default());
        self.lanes
            .lock()
            .expect("lane lock")
            .push((attempt.session, Arc::clone(&lane)));
        self.started.fetch_add(1, Ordering::SeqCst);
        self.started_changed.notify_waiters();
        Box::pin(async move { lane.outcome().await })
    }
}

#[derive(Default)]
struct ManualLane {
    outcome: Mutex<Option<PublishOutcome>>,
    changed: Notify,
}

impl ManualLane {
    fn release(&self, outcome: PublishOutcome) {
        *self.outcome.lock().expect("outcome lock") = Some(outcome);
        self.changed.notify_waiters();
    }

    async fn outcome(&self) -> PublishOutcome {
        loop {
            let changed = self.changed.notified();
            if let Some(outcome) = self.outcome.lock().expect("outcome lock").take() {
                return outcome;
            }
            changed.await;
        }
    }
}

struct NoopTransport;

impl Transport for NoopTransport {
    fn acquire_session(&self, request: OpenRelaySession) -> RelaySessionFuture<'_> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::ConnectionRefused(
                TransportFailure::Disconnected {
                    detail: BoundedReason::new("manual publisher owns the result"),
                },
            ))
        })
    }

    fn holders(&self, _key: &RelaySessionKey) -> Option<NonZeroUsize> {
        None
    }

    fn shutdown(&self, _deadline: Duration) -> TransportShutdownFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
