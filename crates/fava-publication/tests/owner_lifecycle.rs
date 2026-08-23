//! Owner-level evidence for signing, routing shortfall, and delivery custody.

mod owner_lifecycle {
    pub mod harness;
}

use std::sync::Arc;

use fava_publisher::PublishOutcome;
use fava_write::{EventValue, RelayDeliveryOutcome, WriteRouting};
use owner_lifecycle::harness::{
    GatedSigner, Harness, HarnessBuilder, ImmediateSigner, RecordingPolicy, RefusingRouter,
    ScriptedPublisher, relay, relay_url,
};

fn explicit(url: &str) -> WriteRouting {
    WriteRouting::explicit([relay_url(url)]).expect("explicit route is valid")
}

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
        PublishOutcome::AuthenticationRequired,
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
            PublishOutcome::AuthenticationRequired,
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
