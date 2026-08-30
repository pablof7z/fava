//! Wire evidence for automatic `$currentPubkey` query recompilation.

use std::sync::Arc;
use std::time::Duration;

use fava::{EventValue, Fava, Query, RelayUrl};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{EventBuilder, FinalizeEvent, Kind};
use nostr::key::{Keys, PublicKey};
use nostr::types::Timestamp;

#[tokio::test(flavor = "current_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "one causal wire transcript keeps REQ, CLOSE, EOSE, stale-event, and public-attribution assertions in order"
)]
async fn current_account_switch_replaces_exact_wire_demand() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let relay = RelayUrl::parse("wss://current-account.example").expect("relay URL");
    let key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(&transport);
    fava.add_account(alice.public_key()).expect("Alice adds");
    fava.add_account(bob.public_key()).expect("Bob adds");
    fava.select_account(alice.public_key())
        .expect("Alice selects");
    let mut observation = fava
        .observe(
            Query::events()
                .authors_current_account()
                .only_from_relays([relay])
                .expect("explicit relay"),
        )
        .await
        .expect("reactive live observation opens");
    let observation_id = observation.id();

    wait_until("Alice REQ", || {
        request_for(&transport, &key, alice.public_key()).is_some()
    })
    .await;
    let alice_subscription = request_for(&transport, &key, alice.public_key()).expect("Alice REQ");
    assert_eq!(
        fava.diagnostics()
            .queries
            .iter()
            .map(|query| query.observation)
            .collect::<Vec<_>>(),
        vec![observation_id]
    );
    let peer = transport.relay(&key).expect("relay established");

    fava.select_account(bob.public_key()).expect("Bob selects");
    wait_until("Bob REQ", || {
        request_for_peer(&peer, bob.public_key()).is_some()
    })
    .await;
    let bob_subscription = request_for_peer(&peer, bob.public_key()).expect("Bob REQ");
    wait_until("Alice CLOSE", || {
        withdrawals_peer(&peer).contains(&alice_subscription)
    })
    .await;
    assert_ne!(alice_subscription, bob_subscription);
    assert_eq!(observation.id(), observation_id);
    let diagnostics = fava.diagnostics();
    assert_eq!(
        diagnostics
            .queries
            .iter()
            .map(|query| query.observation)
            .collect::<Vec<_>>(),
        vec![observation_id]
    );
    assert!(
        diagnostics
            .relays
            .iter()
            .flat_map(|relay| &relay.subscriptions)
            .flat_map(|wire| &wire.serves)
            .all(|owner| *owner == observation_id),
        "wire diagnostics expose only the stable public observation id"
    );

    push(
        &peer,
        &RelayMessage::event(
            alice_subscription,
            EventBuilder::new(Kind::TextNote, "stale Alice")
                .custom_created_at(Timestamp::from(10))
                .finalize(&alice)
                .expect("Alice event signs"),
        ),
    );
    push(
        &peer,
        &RelayMessage::event(
            bob_subscription.clone(),
            EventBuilder::new(Kind::TextNote, "current Bob")
                .custom_created_at(Timestamp::from(11))
                .finalize(&bob)
                .expect("Bob event signs"),
        ),
    );
    let bob_snapshot = observation
        .wait_until(Duration::from_secs(1), |snapshot| {
            snapshot.events.iter().any(|record| {
                record.event().author() == bob.public_key()
                    && content(record.event()) == "current Bob"
            })
        })
        .await
        .expect("observation remains open")
        .expect("Bob event becomes current");
    assert!(
        bob_snapshot
            .events
            .iter()
            .all(|record| record.event().author() == bob.public_key())
    );

    fava.clear_current_account().expect("selection clears");
    let empty = observation
        .wait_until(Duration::from_secs(1), |snapshot| {
            snapshot.events.is_empty()
        })
        .await
        .expect("observation remains open")
        .expect("empty selection matches nothing");
    assert!(empty.events.is_empty());
    wait_until("Bob CLOSE", || {
        withdrawals_peer(&peer).contains(&bob_subscription)
    })
    .await;
    assert_eq!(requests_peer(&peer).len(), 2, "clear emits no broad REQ");
}

fn assembly(transport: &Arc<FakeTransport>) -> Fava {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(fava_subscriptions_no_grouping::planner()))
        .transport(Arc::clone(transport))
        .build()
        .expect("live assembly")
}

fn request_for(
    transport: &FakeTransport,
    key: &RelaySessionKey,
    author: PublicKey,
) -> Option<SubscriptionId> {
    requests(transport, key)
        .into_iter()
        .find_map(|(id, filters)| {
            filters
                .iter()
                .any(|filter| {
                    filter
                        .authors
                        .as_ref()
                        .is_some_and(|values| values == &[author].into_iter().collect())
                })
                .then_some(id)
        })
}

fn request_for_peer(peer: &FakeRelay, author: PublicKey) -> Option<SubscriptionId> {
    requests_peer(peer).into_iter().find_map(|(id, filters)| {
        filters
            .iter()
            .any(|filter| {
                filter
                    .authors
                    .as_ref()
                    .is_some_and(|values| values == &[author].into_iter().collect())
            })
            .then_some(id)
    })
}

fn requests(
    transport: &FakeTransport,
    key: &RelaySessionKey,
) -> Vec<(SubscriptionId, Vec<nostr::filter::Filter>)> {
    messages(transport, key)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Req {
                subscription_id,
                filters,
            } => Some((
                subscription_id.into_owned(),
                filters
                    .into_iter()
                    .map(std::borrow::Cow::into_owned)
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn requests_peer(peer: &FakeRelay) -> Vec<(SubscriptionId, Vec<nostr::filter::Filter>)> {
    peer_messages(peer)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Req {
                subscription_id,
                filters,
            } => Some((
                subscription_id.into_owned(),
                filters
                    .into_iter()
                    .map(std::borrow::Cow::into_owned)
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn withdrawals_peer(peer: &FakeRelay) -> Vec<SubscriptionId> {
    peer_messages(peer)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Close(id) => Some(id.into_owned()),
            _ => None,
        })
        .collect()
}

fn messages(transport: &FakeTransport, key: &RelaySessionKey) -> Vec<ClientMessage<'static>> {
    transport
        .relay(key)
        .map(|peer| peer.delivered_frames())
        .unwrap_or_default()
        .into_iter()
        .map(|frame| serde_json::from_slice(&frame).expect("client frame decodes"))
        .collect()
}

fn peer_messages(peer: &FakeRelay) -> Vec<ClientMessage<'static>> {
    peer.delivered_frames()
        .into_iter()
        .map(|frame| serde_json::from_slice(&frame).expect("client frame decodes"))
        .collect()
}

fn push(peer: &FakeRelay, message: &RelayMessage<'_>) {
    peer.push_frame(serde_json::to_vec(message).expect("relay frame encodes"));
}

fn content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

async fn wait_until(label: &str, predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{label} deadline elapsed"));
}
