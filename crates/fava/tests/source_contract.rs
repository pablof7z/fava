//! Shared query-source behavior corpus for the two M1 memory providers.

use std::future::Future;

use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{Query, QuerySource, SourceKind, SourceRevision};
use fava_state::{CachedEvent, RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl, Timestamp};
use fava_write::EventValue;
use fava_write_store::WriteStore;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, FinalizeUnsignedEvent, Kind};
use nostr::key::Keys;

async fn assert_add_remove_corpus<S, Add, Remove, Added>(
    source: &S,
    kind: SourceKind,
    add: Add,
    remove: Remove,
) where
    S: QuerySource,
    Add: FnOnce() -> Added,
    Remove: FnOnce(Added),
{
    let mut opened = source
        .open(&Query::events().cache_only())
        .expect("source opens");
    assert_eq!(opened.initial.kind, kind);
    assert_eq!(opened.initial.revision, SourceRevision(0));
    assert!(opened.initial.events.is_empty());

    let added_value = add();
    let added = opened
        .changes
        .next_change()
        .await
        .expect("addition revision arrives");
    assert_eq!(added.kind, kind);
    assert_eq!(added.revision, SourceRevision(1));
    assert_eq!(added.events.len(), 1);

    remove(added_value);
    let removed = opened
        .changes
        .next_change()
        .await
        .expect("removal revision arrives");
    assert_eq!(removed.kind, kind);
    assert_eq!(removed.revision, SourceRevision(2));
    assert!(removed.events.is_empty());
}

fn run(test: impl Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime builds")
        .block_on(test);
}

#[test]
fn memory_event_cache_runs_the_source_corpus() {
    let cache = MemoryEventCache::default();
    let keys = Keys::generate();
    let event = EventBuilder::new(Kind::TextNote, "cached")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .expect("event signs");
    let event_id = event.id;
    let evidence = RelayEvidence::one(
        RelaySessionKey::new(
            RelayUrl::parse("wss://relay.example").expect("relay url"),
            RelayAccess::public(),
        ),
        Timestamp::from(11),
    );

    run(assert_add_remove_corpus(
        &cache,
        SourceKind::EventCache,
        || {
            cache
                .admit(CachedEvent::new(event, evidence), Timestamp::from(11))
                .expect("event admits");
            event_id
        },
        |id| {
            cache
                .commit(vec![fava_state::CacheMutation::Retract(id)])
                .expect("event retracts");
        },
    ));
}

#[test]
fn memory_write_store_runs_the_source_corpus() {
    let writes = MemoryWriteStore::default();
    let keys = Keys::generate();
    let mut event = EventBuilder::new(Kind::TextNote, "local")
        .custom_created_at(Timestamp::from(10))
        .finalize_unsigned(keys.public_key());
    event.ensure_id();

    run(assert_add_remove_corpus(
        &writes,
        SourceKind::WriteStore,
        || {
            writes
                .accept_materialized(EventValue::Unsigned(event))
                .expect("event accepts")
                .receipt_id
        },
        |receipt_id| {
            assert!(writes.cancel(receipt_id).expect("event cancels"));
        },
    ));
}
