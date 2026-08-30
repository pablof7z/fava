//! Coverage for the protocol-extension-traits spec's enablement scenarios:
//! order independence (4.1), coexistence and collision between an
//! application-defined applier and an enabled protocol (4.2), and a
//! forgotten enabling call failing both at assembly and at first publish,
//! naming the unclaimed kind (4.3).

use std::sync::Arc;

use fava::{
    BuildError, EventBuilder, EventEdit, Kind, PublicationError, PublishError, Timestamp,
    WriteRouting,
};
use fava_bookmarks::Bookmarks;
use fava_nip02::Nip02;
use fava_write::WriteIntent;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::EventId;
use nostr::key::Keys;

#[allow(dead_code)]
#[path = "support/semantic_write.rs"]
mod support;

use support::{RecordingPublisher, TestApplier, publication_builder, relay_url};

fn nip02_edit() -> EventEdit {
    fava_nip02::follow(Keys::generate().public_key()).expect("nip02 edit validates")
}

fn bookmark_edit() -> EventEdit {
    fava_bookmarks::bookmark_event(EventId::from_byte_array([7; 32]))
        .expect("bookmark edit validates")
}

/// 4.1 — Enabling several protocols in either order produces the same
/// facade: both protocols publish successfully regardless of which
/// `with_*` call runs first.
#[tokio::test(flavor = "current_thread")]
async fn enabling_two_protocols_in_either_order_produces_the_same_facade() {
    let keys = Keys::generate();
    let actor = keys.public_key();

    let nip02_first = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .with_nip02()
    .with_bookmarks()
    .build()
    .expect("nip02-then-bookmarks assembly");
    let bookmarks_first = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .with_bookmarks()
    .with_nip02()
    .build()
    .expect("bookmarks-then-nip02 assembly");

    for fava in [nip02_first, bookmarks_first] {
        let nip02_write = fava
            .by(actor)
            .to([relay_url()])
            .expect("route validates")
            .publish(nip02_edit())
            .expect("nip02 publishes regardless of enabling order");
        let nip02_receipt = nip02_write.receipt().expect("nip02 receipt readable");
        assert_eq!(nip02_receipt.current.event.kind(), Kind::ContactList);

        let bookmark_write = fava
            .by(actor)
            .to([relay_url()])
            .expect("route validates")
            .publish(bookmark_edit())
            .expect("bookmarks publishes regardless of enabling order");
        let bookmark_receipt = bookmark_write.receipt().expect("bookmark receipt readable");
        assert_eq!(bookmark_receipt.current.event.kind(), Kind::Custom(10_003));
    }
}

/// 4.2 (coexistence) — An application-defined applier for one kind and an
/// enabled protocol for a different kind share one index: both publish.
#[tokio::test(flavor = "current_thread")]
async fn app_defined_applier_and_enabled_protocol_coexist_in_one_index() {
    let keys = Keys::generate();
    let actor = keys.public_key();
    let app_kind = Kind::Custom(19_500);
    let app_applier = Arc::new(TestApplier::new(app_kind));

    let fava = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .applier(Arc::clone(&app_applier))
    .with_nip02()
    .build()
    .expect("app-defined applier and enabled protocol coexist");

    let app_write = fava
        .by(actor)
        .to([relay_url()])
        .expect("route validates")
        .publish(EventEdit::new(app_kind, None, vec![1]).expect("bounded app edit"))
        .expect("app-defined kind publishes");
    assert_eq!(
        app_write
            .receipt()
            .expect("app receipt readable")
            .current
            .event
            .kind(),
        app_kind
    );

    let nip02_write = fava
        .by(actor)
        .to([relay_url()])
        .expect("route validates")
        .publish(nip02_edit())
        .expect("enabled protocol kind publishes alongside the app-defined one");
    assert_eq!(
        nip02_write
            .receipt()
            .expect("nip02 receipt readable")
            .current
            .event
            .kind(),
        Kind::ContactList
    );
}

/// 4.2 (collision) — An application-defined applier and an enabled
/// protocol claiming the same kind are refused at assembly, with neither
/// registration order taking precedence.
#[tokio::test(flavor = "current_thread")]
async fn app_defined_applier_colliding_with_an_enabled_protocol_is_refused_either_order() {
    let keys = Keys::generate();
    let colliding_applier = || Arc::new(TestApplier::new(Kind::ContactList));

    let app_then_protocol = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys.clone())),
        Arc::new(RecordingPublisher::default()),
    )
    .applier(colliding_applier())
    .with_nip02()
    .build();
    assert!(
        matches!(app_then_protocol, Err(BuildError::Publication(_))),
        "app-applier-then-protocol collision must be refused"
    );

    let protocol_then_app = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys)),
        Arc::new(RecordingPublisher::default()),
    )
    .with_nip02()
    .applier(colliding_applier())
    .build();
    assert!(
        matches!(protocol_then_app, Err(BuildError::Publication(_))),
        "protocol-then-app-applier collision must be refused"
    );
}

/// 4.3 (assembly) — A forgotten enabling call fails at assembly when a
/// stored write of that kind is outstanding, naming the unclaimed kind.
#[tokio::test(flavor = "current_thread")]
async fn forgotten_enabling_call_fails_at_assembly_when_a_write_is_outstanding() {
    let keys = Keys::generate();
    let store = Arc::new(MemoryWriteStore::default());
    store
        .accept_applied_edit(
            WriteIntent::edit_as(
                nip02_edit(),
                keys.public_key(),
                WriteRouting::explicit([relay_url()]).expect("explicit route validates"),
            )
            .expect("outstanding intent validates"),
            EventBuilder::new(Kind::ContactList)
                .created_at(Timestamp::from(1))
                .content("edit")
                .by(keys.public_key())
                .build()
                .expect("outstanding unsigned event builds"),
            None,
        )
        .expect("outstanding nip02 write seeds directly, bypassing any applier");

    // `with_nip02()` is never called: nothing claims `Kind::ContactList`.
    let assembly = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        store,
        Arc::new(support::CountingSigner::new(keys)),
        Arc::new(RecordingPublisher::default()),
    )
    .build();
    match assembly {
        Err(BuildError::Publication(message)) => {
            assert!(
                message.contains(&Kind::ContactList.as_u16().to_string()),
                "assembly refusal must name the unclaimed kind: {message}"
            );
        }
        Ok(_) => panic!("expected an assembly-time publication refusal, assembly succeeded"),
        Err(other) => panic!("expected an assembly-time publication refusal, got {other:?}"),
    }
}

/// 4.3 (first publish) — A forgotten enabling call fails at first publish
/// when nothing was outstanding at assembly, naming the unclaimed kind.
#[tokio::test(flavor = "current_thread")]
async fn forgotten_enabling_call_fails_at_first_publish_naming_the_kind() {
    let keys = Keys::generate();
    let actor = keys.public_key();

    // `with_nip02()` is never called: nothing claims `Kind::ContactList`.
    let fava = publication_builder(
        Arc::new(fava_event_cache_memory::MemoryEventCache::default()),
        Arc::new(MemoryWriteStore::default()),
        Arc::new(support::CountingSigner::new(keys)),
        Arc::new(RecordingPublisher::default()),
    )
    .build()
    .expect("assembly without any outstanding write succeeds");

    let result = fava
        .by(actor)
        .to([relay_url()])
        .expect("route validates")
        .publish(nip02_edit());
    match result {
        Err(PublishError::Publication(PublicationError::Routing(message))) => {
            assert!(
                message.contains(&Kind::ContactList.as_u16().to_string()),
                "first-publish refusal must name the unclaimed kind: {message}"
            );
        }
        other => panic!("expected a first-publish routing refusal, got {other:?}"),
    }
}
