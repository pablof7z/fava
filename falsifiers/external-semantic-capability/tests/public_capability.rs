//! Public outside-consumer proof for semantic publication and raw future events.

mod support;

use std::collections::BTreeSet;
use std::sync::{Arc, Barrier};

use fava::{
    EventBuilder, EventValue, Kind, MaterializationId, Query, ReceiptOutcome, RelayDeliveryOutcome,
    Timestamp,
};
use fava_external_semantic_capability_proof::{
    decode_external_event, external_kind, insert, validate_external_event,
};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
use nostr::key::Keys;

use support::{
    explicit_intent, harness, open_external_source, open_observation, raw_intent, signed,
    wait_eose, wait_first_record, wait_generation_record, wait_receipt, wait_terminal,
};

#[tokio::test(flavor = "current_thread")]
async fn external_capability_composes_through_public_fava() {
    let keys = Keys::generate();
    let actor = keys.public_key();
    let harness = harness(keys.clone());
    let intent = explicit_intent(insert(actor, "alpha").unwrap(), &harness.relay);

    let preview = harness
        .fava
        .preview_write_routes(&intent)
        .expect("external semantic preview");
    assert!(preview.settled);
    assert_eq!(preview.destinations.len(), 1);
    let preview_keys = preview
        .destinations
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert!(
        preview
            .destinations
            .keys()
            .any(|session| session.relay == harness.relay)
    );
    assert!(harness.fava.open_receipts().unwrap().is_empty());
    assert_eq!(harness.transport.open_count(), 0);
    assert_eq!(harness.transport.publication_count(), 0);

    let mut observation = open_external_source(&harness.fava, &harness.relay, actor).await;
    let subscription = harness.transport.subscription().await;
    let accepted = harness.fava.publish(intent).expect("external edit accepts");
    let first = harness.transport.published(0).await;
    let generation_one = wait_receipt(&harness.fava, accepted.receipt_id, |receipt| {
        receipt.current.publication.materialization_id == MaterializationId::from_u64(1)
            && receipt.attempts.values().copied().sum::<u32>() == 1
    })
    .await;
    assert_eq!(accepted.write_id, generation_one.write_id);
    assert_eq!(accepted.receipt_id, generation_one.receipt_id);
    assert_eq!(first.kind, external_kind());
    assert_eq!(generation_one.desired_destinations, preview_keys);
    assert_eq!(
        generation_one
            .destinations()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        preview_keys
    );
    assert_eq!(
        generation_one
            .attempts
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        preview_keys
    );

    let preserved_tag = Tag::parse(["x-future", "opaque"]).expect("unknown tag");
    let source = NostrEventBuilder::new(
        external_kind(),
        "external-set-v1\nomega\nunrelated\nsource-body",
    )
    .tag(preserved_tag.clone())
    .custom_created_at(Timestamp::from(u64::MAX - 1))
    .finalize(&keys)
    .expect("independent source signs");
    harness.transport.deliver(&subscription, &source);

    let generation_two = wait_receipt(&harness.fava, accepted.receipt_id, |receipt| {
        receipt.current.publication.materialization_id == MaterializationId::from_u64(2)
    })
    .await;
    assert_eq!(generation_two.write_id, accepted.write_id);
    assert_eq!(generation_two.receipt_id, accepted.receipt_id);
    assert_eq!(
        generation_two.current.publication.materialization_source,
        Some(source.id)
    );
    assert_eq!(
        generation_two.current.publication.retired_materializations[0].0,
        MaterializationId::from_u64(1)
    );
    assert_eq!(
        generation_two.current.publication.retired_materializations[0].1,
        first.id
    );
    let record = wait_generation_record(&mut observation, 2).await;
    assert_eq!(record.event.kind(), external_kind());
    assert_eq!(record.event.tags(), &[preserved_tag]);
    validate_external_event(&record.event).expect("public typed validation accepts successor");
    assert_eq!(
        decode_external_event(&record.event).expect("public typed decode accepts successor"),
        (
            BTreeSet::from(["alpha".to_owned(), "omega".to_owned()]),
            "unrelated\nsource-body".to_owned()
        )
    );

    let barrier = Arc::new(Barrier::new(3));
    std::thread::scope(|scope| {
        let first_transport = Arc::clone(&harness.transport);
        let first_barrier = Arc::clone(&barrier);
        let first_subscription = subscription.clone();
        let first_source = source.clone();
        let first_duplicate = scope.spawn(move || {
            first_barrier.wait();
            first_transport.deliver(&first_subscription, &first_source);
        });
        let second_transport = Arc::clone(&harness.transport);
        let second_barrier = Arc::clone(&barrier);
        let second_subscription = subscription.clone();
        let second_source = source.clone();
        let second_duplicate = scope.spawn(move || {
            second_barrier.wait();
            second_transport.deliver(&second_subscription, &second_source);
        });
        barrier.wait();
        first_duplicate.join().expect("first concurrent duplicate");
        second_duplicate
            .join()
            .expect("second concurrent duplicate");
    });
    harness.transport.eose(&subscription);
    wait_eose(&harness.fava, &subscription).await;
    assert_eq!(harness.transport.publication_count(), 1);
    assert_eq!(
        harness
            .fava
            .receipt(accepted.receipt_id)
            .unwrap()
            .unwrap()
            .current
            .publication
            .materialization_id,
        MaterializationId::from_u64(2)
    );

    let retired = harness.transport.acknowledge(0);
    harness.transport.wait_closed(retired).await;
    let after_retired = harness.fava.receipt(accepted.receipt_id).unwrap().unwrap();
    assert_eq!(
        after_retired.current.publication.materialization_id,
        MaterializationId::from_u64(2)
    );
    let second = harness.transport.published(1).await;
    assert_ne!(second.id, retired);
    let before_successor_ack = wait_receipt(&harness.fava, accepted.receipt_id, |receipt| {
        receipt.current.publication.materialization_id == MaterializationId::from_u64(2)
            && receipt.attempts.values().copied().sum::<u32>() == 1
    })
    .await;
    assert_eq!(before_successor_ack.outcome, ReceiptOutcome::Open);
    let successor_outcome = before_successor_ack
        .destinations()
        .values()
        .next()
        .expect("successor destination exists");
    assert!(!matches!(
        successor_outcome,
        RelayDeliveryOutcome::Acknowledged { .. }
    ));
    assert!(!successor_outcome.is_terminal());
    assert_eq!(
        decode_external_event(&EventValue::Signed(second.clone())).unwrap(),
        decode_external_event(&after_retired.current.event).unwrap()
    );
    let current = harness.transport.acknowledge(1);
    harness.transport.wait_closed(current).await;
    let terminal = wait_terminal(
        &harness.fava,
        accepted.receipt_id,
        "successor terminal receipt",
    )
    .await;
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.write_id, accepted.write_id);
    assert_eq!(terminal.receipt_id, accepted.receipt_id);
    assert_eq!(terminal.current.publication.materialization_id.as_u64(), 2);
    assert_eq!(harness.transport.publication_count(), 2);
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn external_retired_completion_and_failure_preserve_current() {
    let keys = Keys::generate();
    let actor = keys.public_key();
    let harness = harness(keys.clone());
    let observation = open_external_source(&harness.fava, &harness.relay, actor).await;
    let subscription = harness.transport.subscription().await;
    let accepted = harness
        .fava
        .publish(explicit_intent(
            insert(actor, "alpha").unwrap(),
            &harness.relay,
        ))
        .expect("external edit accepts");
    let first = harness.transport.published(0).await;

    let oversized = NostrEventBuilder::new(external_kind(), "z".repeat(8_192))
        .custom_created_at(Timestamp::from(u64::MAX - 1))
        .finalize(&keys)
        .expect("core-valid oversized source signs");
    harness.transport.deliver(&subscription, &oversized);
    let failed = wait_receipt(&harness.fava, accepted.receipt_id, |receipt| {
        receipt
            .current
            .publication
            .materialization_failure
            .is_some()
    })
    .await;
    assert_eq!(
        failed.current.publication.materialization_id,
        MaterializationId::from_u64(1)
    );
    assert_eq!(failed.current.publication.materialization_source, None);
    assert_eq!(failed.current.id(), first.id);
    assert!(
        failed
            .current
            .publication
            .materialization_failure
            .as_deref()
            .is_some_and(|failure| failure.contains("4096"))
    );
    assert_eq!(harness.transport.publication_count(), 1);

    let current = harness.transport.acknowledge(0);
    harness.transport.wait_closed(current).await;
    let terminal = wait_terminal(
        &harness.fava,
        accepted.receipt_id,
        "preserved generation terminal receipt",
    )
    .await;
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(terminal.current.id(), first.id);
    assert_eq!(terminal.current.publication.materialization_id.as_u64(), 1);
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn raw_future_event_kind_publishes_unchanged() {
    let keys = Keys::generate();
    let actor = keys.public_key();
    let harness = harness(keys);
    let future_kind = Kind::Custom(50_001);
    let unknown = vec![
        Tag::parse(["something something"]).expect("future flag tag"),
        Tag::parse(["x-a", "poop"]).expect("future value tag"),
        Tag::parse(["x-future", "kept", "verbatim"]).expect("future tag"),
    ];
    let event = EventBuilder::from_parts(
        actor,
        future_kind,
        Timestamp::from(42),
        Vec::new(),
        "opaque future content".to_owned(),
    )
    .tags(unknown.clone())
    .build()
    .expect("raw future event builds");
    let expected_id = event.id.expect("builder assigned id");
    let mut observation = open_observation(
        &harness.fava,
        Query::events().kind(future_kind).cache_only(),
        "raw future observation open",
    )
    .await;
    let accepted = harness
        .fava
        .publish(raw_intent(event.clone(), &harness.relay))
        .expect("raw future event accepts without matching materializer");
    assert_eq!(accepted.current.event, EventValue::Unsigned(event.clone()));
    let record = wait_first_record(&mut observation, "raw future query visibility").await;
    assert_eq!(record.id(), expected_id);
    assert_eq!(record.event.kind(), future_kind);
    assert_eq!(record.event.created_at(), Timestamp::from(42));
    assert_eq!(record.event.tags(), unknown.as_slice());
    assert_eq!(content(&record.event), "opaque future content");

    let published = harness.transport.published(0).await;
    assert_eq!(published.id, expected_id);
    assert_eq!(published.kind, future_kind);
    assert_eq!(published.created_at, Timestamp::from(42));
    assert_eq!(published.tags.as_slice(), unknown.as_slice());
    assert_eq!(published.content, "opaque future content");
    let id = harness.transport.acknowledge(0);
    harness.transport.wait_closed(id).await;
    let terminal = wait_terminal(
        &harness.fava,
        accepted.receipt_id,
        "raw future terminal receipt",
    )
    .await;
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(signed(&terminal), &published);
    assert_eq!(signed(&terminal).tags.as_slice(), unknown.as_slice());
    observation.close();
}

fn content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}
