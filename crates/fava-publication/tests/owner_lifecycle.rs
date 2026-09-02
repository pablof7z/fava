//! Owner-level evidence for signing, routing shortfall, and delivery custody.

mod owner_lifecycle {
    pub mod harness;
}

use std::sync::Arc;
use std::time::Duration;

use fava_publisher::PublishOutcome;
use fava_publisher_nip01::Nip01Publisher;
use fava_relay::Authority;
use fava_transport::RelaySessionExt;
use fava_transport_testkit::FakeTransport;
use fava_write::{EventValue, ReceiptOutcome, RelayDeliveryOutcome, WriteRouting};
use nostr::key::Keys;
use owner_lifecycle::harness::{
    GatedPublisher, GatedSigner, Harness, HarnessBuilder, ImmediateSigner, RecordingPolicy,
    RefusingRouter, ScriptedPublisher, relay, relay_url,
};

fn explicit(url: &str) -> WriteRouting {
    WriteRouting::explicit([relay_url(url)]).expect("explicit route is valid")
}

/// Bound every polling wait in the real-transport authentication test: a
/// regression in the wait-on-connection behavior must fail it, never hang
/// the process.
const AUTHENTICATION_WAIT_DEADLINE: Duration = Duration::from_secs(5);

/// An unusable router chain must not gate signer acquisition
/// (ARCHITECTURE.md:2160, WRITE-028). `fava_routing::open` refuses an invalid
/// chain configuration outright, and that refusal must stay a typed shortfall on
/// the write rather than abandoning durably accepted custody.
#[tokio::test]
async fn signing_proceeds_when_the_router_chain_cannot_be_opened() {
    let harness = HarnessBuilder::default()
        .router(Arc::new(RefusingRouter))
        .router(Arc::new(RefusingRouter))
        .signer(Arc::new(ImmediateSigner::default()))
        .build();
    let receipt_id = harness.publish_unsigned(WriteRouting::Automatic);

    let signed = harness
        .until(receipt_id, |receipt| {
            matches!(receipt.current.event, EventValue::Signed(_))
        })
        .await;

    assert!(
        signed.is_some(),
        "an unopenable router chain must not gate signer acquisition"
    );
}

/// The chain-configuration refusal is a typed shortfall, and the write stays open.
#[tokio::test]
async fn an_unopenable_router_chain_leaves_a_typed_shortfall_on_an_open_write() {
    let harness = HarnessBuilder::default()
        .router(Arc::new(RefusingRouter))
        .router(Arc::new(RefusingRouter))
        .signer(Arc::new(ImmediateSigner::default()))
        .build();
    let receipt_id = harness.publish_unsigned(WriteRouting::Automatic);

    let shortfall = harness
        .until(receipt_id, |receipt| !receipt.route_shortfalls.is_empty())
        .await
        .expect("an unopenable chain commits a shortfall");

    assert!(
        shortfall
            .route_shortfalls
            .iter()
            .any(|reason| reason.contains("duplicate router name")),
        "shortfall must attribute the exact chain refusal: {:?}",
        shortfall.route_shortfalls
    );
    assert!(
        !shortfall.is_terminal(),
        "an unresolved route stays open rather than becoming terminal (WRITE-027)"
    );
}

/// A live router that refuses to open is isolated by the routing owner: the write
/// keeps an attributed shortfall and a live owner rather than being abandoned.
#[tokio::test]
async fn a_refusing_router_is_isolated_into_an_attributed_shortfall() {
    let harness = HarnessBuilder::default()
        .router(Arc::new(RefusingRouter))
        .signer(Arc::new(ImmediateSigner::default()))
        .build();
    let receipt_id = harness.publish_unsigned(WriteRouting::Automatic);

    let settled = harness
        .until(receipt_id, |receipt| !receipt.route_shortfalls.is_empty())
        .await
        .expect("a refused router commits a shortfall");

    assert!(
        settled
            .route_shortfalls
            .iter()
            .any(|reason| reason.contains("router refuses to open a live session")),
        "shortfall must attribute the exact router refusal: {:?}",
        settled.route_shortfalls
    );
}

/// Auth denial is a distinct post-handoff fact, never a pre-handoff give-up (WRITE-018).
#[tokio::test]
async fn auth_denial_is_a_distinct_post_handoff_destination_fact() {
    let publisher = Arc::new(ScriptedPublisher::new(
        PublishOutcome::AuthenticationRequired {
            message: "auth-required: we only serve authenticated users".to_owned(),
        },
    ));
    let harness = HarnessBuilder::default()
        .publisher(publisher.clone())
        .build();
    let receipt_id = harness.publish_signed(explicit("wss://relay.test"));
    let session = relay("wss://relay.test");

    let settled = harness
        .until(receipt_id, |receipt| {
            receipt
                .destinations()
                .get(&session)
                .is_some_and(RelayDeliveryOutcome::is_terminal)
        })
        .await
        .expect("the destination reaches a terminal fact");

    assert!(
        matches!(
            settled.destinations().get(&session),
            Some(RelayDeliveryOutcome::AuthenticationDenied { .. })
        ),
        "auth denial must not be reported as a pre-handoff give-up: {:?}",
        settled.destinations().get(&session)
    );
    assert_eq!(
        publisher.attempts(),
        1,
        "one auth-denied destination consumes exactly one publisher attempt"
    );
}

/// The selected delivery policy, not the owner, observes the auth fact (WRITE-018/019).
#[tokio::test]
async fn the_delivery_policy_observes_the_authentication_fact() {
    let policy = Arc::new(RecordingPolicy::default());
    let harness = HarnessBuilder::default()
        .publisher(Arc::new(ScriptedPublisher::new(
            PublishOutcome::AuthenticationRequired {
                message: "auth-required: we only serve authenticated users".to_owned(),
            },
        )))
        .delivery(policy.clone())
        .build();
    let receipt_id = harness.publish_signed(explicit("wss://relay.test"));
    let session = relay("wss://relay.test");

    harness
        .until(receipt_id, |receipt| {
            receipt
                .destinations()
                .get(&session)
                .is_some_and(RelayDeliveryOutcome::is_terminal)
        })
        .await
        .expect("the destination reaches a terminal fact");
    let saw_auth = harness
        .until(receipt_id, |_| {
            policy
                .seen()
                .iter()
                .any(|outcome| matches!(outcome, RelayDeliveryOutcome::AuthenticationDenied { .. }))
        })
        .await;

    assert!(
        saw_auth.is_some(),
        "policy must observe the auth fact, not a reason the owner invented: {:?}",
        policy.seen()
    );
}

/// A signer completion for a retired obligation is an attributable stale fact (GOAL-008).
#[tokio::test]
async fn a_late_signer_completion_for_a_retired_write_is_attributable() {
    let signer = Arc::new(GatedSigner::default());
    let harness: Harness = HarnessBuilder::default().signer(signer.clone()).build();
    let receipt_id = harness.publish_unsigned(explicit("wss://relay.test"));

    signer.started().await;
    harness
        .publication
        .cancel(receipt_id)
        .expect("pre-handoff cancellation commits");
    signer.release();

    let observed = harness
        .until(receipt_id, |_| {
            !harness.publication.stale_signer_completions().is_empty()
        })
        .await;

    assert!(
        observed.is_some(),
        "a late completion must be distinguishable from never answering"
    );
    let stale = harness.publication.stale_signer_completions();
    assert_eq!(stale.len(), 1, "exactly one stale completion: {stale:?}");
    assert_eq!(
        stale[0].0, receipt_id,
        "the stale fact must name the retired receipt"
    );
}

/// A signer completion for a retired *attachment* generation is also attributable.
/// 07.2 added the generation check but dropped the late completion silently.
#[tokio::test]
async fn a_completion_from_a_replaced_signer_attachment_is_attributable() {
    let signer = Arc::new(GatedSigner::default());
    let harness = HarnessBuilder::default().signer(signer.clone()).build();
    let receipt_id = harness.publish_unsigned(explicit("wss://relay.test"));

    signer.started().await;
    harness
        .session
        .replace_signer(Arc::new(ImmediateSigner::default()))
        .expect("the attachment exists and is replaceable at runtime");
    signer.release();

    let observed = harness
        .until(receipt_id, |_| {
            harness
                .publication
                .stale_signer_completions()
                .iter()
                .any(|(_, _, _, _, reason)| reason.contains("attachment generation"))
        })
        .await;

    assert!(
        observed.is_some(),
        "a retired attachment's late completion must be attributable, not silent: {:?}",
        harness.publication.stale_signer_completions()
    );
}

/// WRITE-008 regression guard: 07.2 delivered runtime signer attachment, so a write
/// parked with no signer wakes when the correct provider is attached later.
#[tokio::test]
async fn a_parked_write_is_signed_when_its_signer_is_attached_at_runtime() {
    let harness = HarnessBuilder::default().build();
    let receipt_id = harness.publish_unsigned(explicit("wss://relay.test"));

    let parked = harness.receipt(receipt_id);
    assert!(
        matches!(parked.current.event, EventValue::Unsigned(_)),
        "no signer is attached yet, so the write parks unsigned"
    );

    harness
        .session
        .add_signer(Arc::new(ImmediateSigner::default()))
        .expect("a fresh attachment is accepted at runtime");

    let signed = harness
        .until(receipt_id, |receipt| {
            matches!(receipt.current.event, EventValue::Signed(_))
        })
        .await;

    assert!(
        signed.is_some(),
        "attaching the correct signer must wake the parked write (WRITE-008)"
    );
}

/// A destination waiting on its connection's authentication spends no
/// attempt and is eligible for no policy decision while it waits: the wait
/// belongs to the connection the publisher is holding, not to a retry the
/// store tracks (5.3).
#[tokio::test]
async fn a_parked_destination_spends_no_attempt_while_it_waits() {
    let publisher = Arc::new(GatedPublisher::new(PublishOutcome::Acknowledged {
        message: "stored".to_owned(),
    }));
    let harness = HarnessBuilder::default()
        .publisher(publisher.clone())
        .build();
    let receipt_id = harness.publish_signed(explicit("wss://relay.test"));
    let session = relay("wss://relay.test");

    publisher.parked().await;
    // Give the destination loop every chance to re-decide while the single
    // authorized attempt is still in flight.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let parked = harness.receipt(receipt_id);
    assert_eq!(
        parked.attempts.get(&session).copied(),
        Some(1),
        "the durable attempt count must not advance while the destination waits"
    );
    assert_eq!(
        parked.destinations().get(&session),
        Some(&RelayDeliveryOutcome::Attempting),
        "a parked destination's standing is the one already authorized attempt in flight"
    );
    assert!(
        matches!(parked.outcome, ReceiptOutcome::Open),
        "a receipt with a destination still waiting is open, not settled: {:?}",
        parked.outcome
    );
    assert_eq!(
        publisher.attempts(),
        1,
        "still exactly one call to the publisher, not a second one racing it"
    );

    publisher.release();
    let settled = harness
        .until(receipt_id, |receipt| {
            receipt
                .destinations()
                .get(&session)
                .is_some_and(RelayDeliveryOutcome::is_terminal)
        })
        .await
        .expect("the destination settles once released");
    assert_eq!(
        settled.destinations().get(&session),
        Some(&RelayDeliveryOutcome::Acknowledged {
            message: "stored".to_owned()
        })
    );
    assert_eq!(
        publisher.attempts(),
        1,
        "resolution consumed no extra attempt either"
    );
}

/// A parked destination fails once its connection can no longer satisfy the
/// write's authority, naming that authentication was required and did not
/// happen, and consuming no more than the one attempt it was parked on (5.4).
#[tokio::test]
async fn a_parked_destination_fails_when_authentication_cannot_be_satisfied() {
    let publisher = Arc::new(GatedPublisher::new(
        PublishOutcome::AuthenticationRequired {
            message: "auth-required: relay refused the answer".to_owned(),
        },
    ));
    let harness = HarnessBuilder::default()
        .publisher(publisher.clone())
        .build();
    let receipt_id = harness.publish_signed(explicit("wss://relay.test"));
    let session = relay("wss://relay.test");

    publisher.parked().await;
    publisher.release();

    let settled = harness
        .until(receipt_id, |receipt| {
            receipt
                .destinations()
                .get(&session)
                .is_some_and(RelayDeliveryOutcome::is_terminal)
        })
        .await
        .expect("the destination settles once the connection can no longer serve it");

    assert!(
        matches!(
            settled.destinations().get(&session),
            Some(RelayDeliveryOutcome::AuthenticationDenied { .. })
        ),
        "a write that cannot authenticate fails named as such: {:?}",
        settled.destinations().get(&session)
    );
    assert_eq!(
        publisher.attempts(),
        1,
        "the failed wait consumed exactly the one attempt it was parked on"
    );
}

/// Real proof, against the same providers a real assembly uses: a write
/// refused for want of authentication while a person is being asked stays
/// open rather than failing the instant the demand arrives, and the
/// component that eventually decides — here, a test standing in for it —
/// learns nothing about the publication it unblocks. This is 6b.8: the hold
/// removed as unreachable, restored (5.5).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_write_awaiting_a_person_stays_open_while_authentication_is_undecided() {
    let transport = FakeTransport::new();
    let account = Keys::generate().public_key();
    let authority = Authority::As(account);
    let harness = HarnessBuilder::default()
        .publisher(Arc::new(Nip01Publisher))
        .transport(Arc::new(transport.clone()))
        .build();
    let session = relay_url("ws://127.0.0.1:1/");
    let routing = WriteRouting::explicit([session.clone()]).expect("explicit route is valid");
    let receipt_id = harness.publish_signed_as(routing, authority);
    // `publish_signed_as` carries the presigned event `signed_note` builds,
    // deterministically, so its id is known without reading it back.
    let event_id = owner_lifecycle::harness::signed_note().id;

    // Let the attempt reach the relay and hand it a challenge with nobody
    // deciding it, the way an undecided person leaves it.
    let relay_session = tokio::time::timeout(AUTHENTICATION_WAIT_DEADLINE, async {
        loop {
            if let Some(relay_session) = transport.session(&session, &authority) {
                return relay_session;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a connection for this relay and authority never appeared");
    let peer = tokio::time::timeout(AUTHENTICATION_WAIT_DEADLINE, async {
        loop {
            if let Some(peer) = transport.relay(&session, &authority) {
                return peer;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the fake relay peer never registered");
    tokio::time::timeout(AUTHENTICATION_WAIT_DEADLINE, async {
        loop {
            if !peer.delivered_frames().is_empty() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the write never reached the relay");
    let refusal =
        serde_json::json!(["OK", event_id.to_hex(), false, "auth-required: sign in"]).to_string();
    peer.push_frame(refusal.as_bytes());
    RelaySessionExt::record_progress(
        &relay_session,
        fava_transport::Progress::Requested {
            challenge: "prove it".to_owned(),
        },
    );

    tokio::time::sleep(Duration::from_millis(100)).await;
    let waiting = harness.receipt(receipt_id);
    assert!(
        matches!(waiting.outcome, ReceiptOutcome::Open),
        "a write refused for want of authentication while a person is being asked \
         must stay open: {:?}",
        waiting.outcome
    );
    assert_eq!(
        waiting.destinations().get(&session),
        Some(&RelayDeliveryOutcome::Attempting),
        "the destination is still the one attempt authorized before the wait"
    );

    // Only now does the person decide, entirely through the connection —
    // nothing here calls into the publication or the receipt to unblock it.
    RelaySessionExt::record_accepted(&relay_session, account);
    tokio::time::timeout(AUTHENTICATION_WAIT_DEADLINE, async {
        loop {
            if peer.delivered_frames().len() >= 2 {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the write never resent the event once accepted");
    let acceptance = serde_json::json!(["OK", event_id.to_hex(), true, "stored"]).to_string();
    peer.push_frame(acceptance.as_bytes());

    let settled = harness
        .until(receipt_id, |receipt| {
            receipt
                .destinations()
                .get(&session)
                .is_some_and(RelayDeliveryOutcome::is_terminal)
        })
        .await
        .expect("the write resumes once the person decides");
    assert_eq!(
        settled.destinations().get(&session),
        Some(&RelayDeliveryOutcome::Acknowledged {
            message: "stored".to_owned()
        })
    );
    assert!(matches!(settled.outcome, ReceiptOutcome::Complete));
}
