//! Public outside-consumer proof for semantic publication and raw future events.

mod support;

use fava::{EventBuilder, EventValue, Kind, MaterializationId, Query, ReceiptOutcome, Timestamp};
use fava_external_semantic_capability_proof::{external_kind, insert};
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent, Tag};
use nostr::key::Keys;

use support::{
    explicit_intent, harness, open_external_source, raw_intent, signed, wait_eose,
    wait_generation_record, wait_receipt,
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
    assert!(
        preview
            .destinations
            .keys()
            .any(|session| session.relay == harness.relay)
    );
    assert!(harness.fava.open_receipts().unwrap().is_empty());
    assert_eq!(harness.transport.open_count(), 0);
    assert_eq!(harness.transport.publication_count(), 0);

    let mut observation =
        open_external_source(&harness.fava, &harness.relay, actor, external_kind()).await;
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
    assert_eq!(
        content(&record.event),
        "external-set-v1\nalpha,omega\nunrelated\nsource-body"
    );

    harness.transport.deliver(&subscription, &source);
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
    assert_eq!(second.content, content(&after_retired.current.event));
    let current = harness.transport.acknowledge(1);
    harness.transport.wait_closed(current).await;
    let terminal = harness
        .fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("successor settles");
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
    let observation =
        open_external_source(&harness.fava, &harness.relay, actor, external_kind()).await;
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
    let terminal = harness
        .fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("preserved generation settles");
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
    let unknown = Tag::parse(["x-future", "kept", "verbatim"]).expect("future tag");
    let event = EventBuilder::new(actor, future_kind)
        .created_at(Timestamp::from(42))
        .tag(unknown.clone())
        .content("opaque future content")
        .build()
        .expect("raw future event builds");
    let expected_id = event.id.expect("builder assigned id");
    let mut observation = harness
        .fava
        .observe(Query::events().kind(future_kind).cache_only())
        .await
        .expect("raw future query opens");
    let accepted = harness
        .fava
        .publish(raw_intent(event, &harness.relay))
        .expect("raw future event accepts without matching materializer");
    let visible = observation
        .changed()
        .await
        .expect("raw event is query-visible");
    let record = visible.events.first().expect("one raw event");
    assert_eq!(record.id(), expected_id);
    assert_eq!(record.event.kind(), future_kind);
    assert_eq!(record.event.tags(), std::slice::from_ref(&unknown));
    assert_eq!(content(&record.event), "opaque future content");

    let published = harness.transport.published(0).await;
    assert_eq!(published.id, expected_id);
    assert_eq!(published.kind, future_kind);
    assert_eq!(published.tags.as_slice(), &[unknown]);
    assert_eq!(published.content, "opaque future content");
    let id = harness.transport.acknowledge(0);
    harness.transport.wait_closed(id).await;
    let terminal = harness
        .fava
        .wait_terminal(accepted.receipt_id)
        .await
        .expect("raw future event settles");
    assert_eq!(terminal.outcome, ReceiptOutcome::Complete);
    assert_eq!(signed(&terminal), &published);
    observation.close();
}

fn content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}
