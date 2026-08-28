//! Public MaxAge reuse boundaries through the assembled Fava facade.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{QueryBranchId, RelaySourceState, RelayUrl};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions_no_grouping::planner;
use fava_transport::Transport;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::Kind;

#[test]
fn max_age_query_identity() {
    let short = Query::events().max_age(Duration::from_secs(1));
    let same = Query::events().max_age(Duration::from_secs(1));
    let long = Query::events().max_age(Duration::from_secs(2));
    assert_eq!(short, same);
    assert_ne!(short, long);
}

#[tokio::test(flavor = "current_thread")]
async fn close_reopen_after_proven_empty_eose_sends_zero_reqs() {
    let relay = relay("covered");
    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&transport),
    );
    let query = max_age_query(Kind::Metadata, &relay);

    let first = fava.observe(query.clone()).await.expect("first open");
    let peer = established(&transport, &relay).await;
    let request = only_request(&peer).await;
    push(&peer, &RelayMessage::eose(request.clone()));
    wait_until(|| stored_complete(&first, &relay)).await;
    first.close();

    let second = fava.observe(query.clone()).await.expect("reopen");
    settle().await;
    assert_eq!(
        requests(&peer).len(),
        1,
        "fresh proven empty coverage suppresses the exact relay REQ"
    );
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn limited_eose_does_not_create_coverage() {
    let relay = relay("limited");
    let transport = Arc::new(FakeTransport::new());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), Arc::clone(&transport));
    let query = max_age_query(Kind::Metadata, &relay)
        .limit(1)
        .expect("positive limit");

    let mut first = fava.observe(query.clone()).await.expect("first open");
    let peer = established(&transport, &relay).await;
    let request = only_request(&peer).await;
    push(&peer, &RelayMessage::eose(request.clone()));
    first
        .changed()
        .await
        .expect("limited EOSE remains a live observation update");
    let filter =
        fava_subscriptions::demand_for_query(first.id(), QueryBranchId::ROOT, &query).filter;
    assert!(
        cache
            .source_coverage(&key(&relay), &filter)
            .expect("cache readable")
            .is_none(),
        "a limited EOSE must not become reusable coverage"
    );
    first.close();
    wait_until(|| !withdrawals(&peer).is_empty()).await;
    wait_until(|| transport.holders(&key(&relay)).is_none()).await;

    let second = fava.observe(query.clone()).await.expect("reopen");
    let replay = established(&transport, &relay).await;
    let replayed = only_request(&replay).await;
    assert_ne!(
        replayed, request,
        "reopen mints a new relay request identity"
    );
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn mismatched_query_reopens_the_relay_request() {
    let relay = relay("mismatch");
    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        Arc::clone(&transport),
    );
    let covered = max_age_query(Kind::Metadata, &relay);

    let first = fava.observe(covered).await.expect("first open");
    let peer = established(&transport, &relay).await;
    push(&peer, &RelayMessage::eose(only_request(&peer).await));
    wait_until(|| stored_complete(&first, &relay)).await;
    first.close();

    let different = max_age_query(Kind::TextNote, &relay);
    let second = fava.observe(different).await.expect("different open");
    wait_until(|| requests(&peer).len() == 2).await;
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn coverage_eviction_reopens_the_relay_request() {
    let relay = relay("evicted");
    let transport = Arc::new(FakeTransport::new());
    let cache = Arc::new(MemoryEventCache::bounded(
        NonZeroUsize::new(1).expect("non-zero"),
    ));
    let fava = assembly(cache, Arc::clone(&transport));
    let first_query = max_age_query(Kind::Metadata, &relay);
    let replacement_query = max_age_query(Kind::TextNote, &relay);

    let first = fava.observe(first_query.clone()).await.expect("first open");
    let peer = established(&transport, &relay).await;
    push(&peer, &RelayMessage::eose(only_request(&peer).await));
    wait_until(|| stored_complete(&first, &relay)).await;
    first.close();

    let replacement = fava
        .observe(replacement_query)
        .await
        .expect("replacement open");
    wait_until(|| requests(&peer).len() == 2).await;
    push(&peer, &RelayMessage::eose(requests(&peer)[1].clone()));
    wait_until(|| stored_complete(&replacement, &relay)).await;
    replacement.close();

    let reopened = fava
        .observe(first_query)
        .await
        .expect("reopen after eviction");
    wait_until(|| requests(&peer).len() == 3).await;
    reopened.close();
}

fn assembly(cache: Arc<MemoryEventCache>, transport: Arc<FakeTransport>) -> Fava {
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(transport)
        .build()
        .expect("assembly")
}

fn max_age_query(kind: Kind, relay: &RelayUrl) -> Query {
    Query::events()
        .kinds([kind])
        .expect("one kind")
        .only_from_relays([relay.clone()])
        .expect("one relay")
        .max_age(Duration::from_secs(60))
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn key(relay: &RelayUrl) -> RelaySessionKey {
    RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    }
}

async fn established(transport: &FakeTransport, relay: &RelayUrl) -> FakeRelay {
    let key = key(relay);
    wait_until(|| transport.relay(&key).is_some()).await;
    transport.relay(&key).expect("relay established")
}

async fn only_request(peer: &FakeRelay) -> SubscriptionId {
    wait_until(|| !requests(peer).is_empty()).await;
    requests(peer).into_iter().next().expect("REQ")
}

fn requests(peer: &FakeRelay) -> Vec<SubscriptionId> {
    peer.delivered_frames()
        .into_iter()
        .filter_map(|frame| {
            match serde_json::from_slice::<ClientMessage<'static>>(&frame).expect("client message")
            {
                ClientMessage::Req {
                    subscription_id, ..
                } => Some(subscription_id.into_owned()),
                _ => None,
            }
        })
        .collect()
}

fn withdrawals(peer: &FakeRelay) -> Vec<SubscriptionId> {
    peer.delivered_frames()
        .into_iter()
        .filter_map(|frame| {
            match serde_json::from_slice::<ClientMessage<'static>>(&frame).expect("client message")
            {
                ClientMessage::Close(subscription_id) => Some(subscription_id.into_owned()),
                _ => None,
            }
        })
        .collect()
}

fn push(peer: &FakeRelay, message: &RelayMessage<'_>) {
    peer.push_frame(
        serde_json::to_string(message)
            .expect("relay message")
            .into_bytes(),
    );
}

fn stored_complete(observation: &fava::Observation, relay: &RelayUrl) -> bool {
    observation
        .current()
        .evidence
        .relay(&key(relay))
        .is_some_and(|evidence| {
            matches!(
                evidence.state,
                RelaySourceState::StoredEventsComplete { .. }
            )
        })
}

async fn wait_until(check: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !check() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline");
}

async fn settle() {
    for _ in 0..128 {
        tokio::task::yield_now().await;
    }
}
