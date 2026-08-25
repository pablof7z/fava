//! QUERY-010 through the public facade: reopening dropped demand uses fresh
//! request identity, so a late frame from the old request cannot settle the new
//! one (`GOALS:426`, acceptance `GOALS:428`).
//!
//! The reconnect half of this promise is proved by
//! `multi_relay.rs::reconnect_uses_fresh_identity_and_rejects_old_subscription_frames`.
//! This file proves the other half — drop and reopen on a socket that never
//! closed — which is reachable in the standard assembly because publication is
//! handed the same `Arc<dyn Transport>` and holds a real relay-session lease
//! across a publish attempt.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{OpenRelaySession, Transport, TransportBounds, TransportDeadlines};
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::types::RelayUrl;

/// A relay whose observe-side demand drains and returns while another lease
/// holder keeps the socket open must not mint the wire id it just closed there.
#[tokio::test(flavor = "current_thread")]
async fn reopening_drained_demand_on_a_retained_socket_uses_fresh_identity() {
    let relay = RelayUrl::parse("wss://retained.example").expect("relay URL");
    let key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let transport = Arc::new(FakeTransport::new());

    // The outsider stands in for the publisher: one lease on the same session
    // key, held across the whole exchange.
    let outsider = transport
        .acquire_session(OpenRelaySession {
            key: key.clone(),
            deadlines: deadlines(),
            bounds: bounds(),
            reconnect_attempts: None,
        })
        .await
        .expect("the outsider acquires the relay session");

    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .build()
        .expect("assembly is complete");
    let query = Query::events()
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid");

    let first = fava
        .observe(query.clone())
        .await
        .expect("the first query opens");
    wait_until(|| requests(&transport, &key).len() == 1).await;
    let retired = requests(&transport, &key)[0].clone();

    first.close();
    wait_until(|| withdrawals(&transport, &key) == vec![retired.clone()]).await;
    wait_until(|| transport.holders(&key) == NonZeroUsize::new(1)).await;
    settle().await;

    let second = fava.observe(query).await.expect("the second query opens");
    wait_until(|| requests(&transport, &key).len() == 2).await;
    let replacement = requests(&transport, &key)[1].clone();

    assert_eq!(
        transport.dials(&key),
        1,
        "the outsider's lease kept the socket open across the drain"
    );
    assert_ne!(
        replacement, retired,
        "a reopened request must not carry the identity of the request just closed on this socket"
    );

    // The acceptance clause: a completion for the retired request cannot settle
    // the new one.
    let peer = transport
        .relay(&key)
        .expect("the session is still registered");
    peer.push_frame(encoded(&RelayMessage::eose(retired)));
    settle().await;
    assert!(
        !settled(&second, &key),
        "a late EOSE naming the retired request must not settle the reopened one"
    );

    peer.push_frame(encoded(&RelayMessage::eose(replacement)));
    wait_until(|| settled(&second, &key)).await;

    second.close();
    drop(outsider);
}

fn deadlines() -> TransportDeadlines {
    TransportDeadlines {
        establish: Duration::from_millis(200),
        write: Duration::from_millis(200),
        idle: Duration::from_secs(60),
        close: Duration::from_millis(200),
    }
}

fn bounds() -> TransportBounds {
    TransportBounds {
        inbound_frames: nonzero(256),
        outbound_frames: nonzero(256),
        max_frame_bytes: nonzero(512 * 1024),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("constant is non-zero")
}

fn settled(observation: &fava::Observation, key: &RelaySessionKey) -> bool {
    observation
        .current()
        .evidence
        .relay(key)
        .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete)
}

fn encoded(message: &RelayMessage<'_>) -> Vec<u8> {
    serde_json::to_string(message)
        .expect("message encodes")
        .into_bytes()
}

fn client_messages(
    transport: &FakeTransport,
    key: &RelaySessionKey,
) -> Vec<ClientMessage<'static>> {
    transport
        .relay(key)
        .map(|peer: FakeRelay| peer.delivered_frames())
        .unwrap_or_default()
        .into_iter()
        .map(|frame| {
            serde_json::from_slice::<ClientMessage<'static>>(&frame)
                .expect("client message decodes")
        })
        .collect()
}

fn requests(transport: &FakeTransport, key: &RelaySessionKey) -> Vec<SubscriptionId> {
    client_messages(transport, key)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Req {
                subscription_id, ..
            } => Some(subscription_id.into_owned()),
            _ => None,
        })
        .collect()
}

fn withdrawals(transport: &FakeTransport, key: &RelaySessionKey) -> Vec<SubscriptionId> {
    client_messages(transport, key)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Close(id) => Some(id.into_owned()),
            _ => None,
        })
        .collect()
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}

/// Let every owner-held task reach quiescence without advancing wall time.
async fn settle() {
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
}
