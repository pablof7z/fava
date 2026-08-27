//! Gate 4: NIP-11 relay-declared `max_subscriptions` limits active subscriptions.
//!
//! Proves: when a relay's NIP-11 document declares `max_subscriptions: 2` and
//! three observations are opened, the planner receives the declared constraint
//! and one demand becomes a typed shortfall without any REQ being sent for it.
//!
//! A real mini HTTP server on 127.0.0.1 serves the NIP-11 document; the
//! transport is the in-memory `FakeTransport`. The admission window is extended
//! so the NIP-11 fetch completes before the planner is called.

mod support;

use std::sync::Arc;
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_observe::Observer;
use fava_query::{Query, QueryEvaluator, QuerySource};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_routing::Router;
use fava_subscriptions::SubscriptionPlanner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::Transport;
use fava_transport_testkit::FakeTransport;
use fava_wire::{ClientMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::Kind;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Milliseconds: long enough for local HTTP round-trip, short enough for tests.
const ADMISSION_WINDOW_MS: u64 = 200;

fn build_observer(
    _relay_url: &RelayUrl,
    transport: Arc<FakeTransport>,
) -> Observer {
    let cache = Arc::new(MemoryEventCache::default());
    let writes = Arc::new(MemoryWriteStore::default());
    let evaluator: Arc<dyn QueryEvaluator> = Arc::new(StandardQueryEvaluator);
    let planner: Arc<dyn SubscriptionPlanner> =
        Arc::new(StandardSubscriptionPlanner::new());

    Observer::new(
        cache.clone() as Arc<dyn QuerySource>,
        writes as Arc<dyn QuerySource>,
        evaluator,
    )
    .with_transport(transport as Arc<dyn Transport>)
    .with_subscription_planner(planner)
    .with_event_cache(cache as Arc<dyn EventCache>)
    .with_diagnostics(Arc::new(Diagnostics::default()))
    .with_routers(Vec::<Arc<dyn Router>>::new())
    .with_admission_window(Duration::from_millis(ADMISSION_WINDOW_MS))
}

fn query_for(relay_url: &RelayUrl, author: nostr::key::PublicKey, kind: Kind) -> Query {
    Query::events()
        .kinds([kind])
        .expect("one kind is bounded")
        .authors([author])
        .expect("one author is bounded")
        .only_from_relays([relay_url.clone()])
        .expect("explicit relay is valid")
}

fn requests_for(transport: &FakeTransport, key: &RelaySessionKey) -> Vec<SubscriptionId> {
    let Some(peer) = transport.relay(key) else {
        return Vec::new();
    };
    peer.delivered_frames()
        .into_iter()
        .filter_map(|f| {
            let msg = serde_json::from_slice::<ClientMessage<'static>>(&f).ok()?;
            if let ClientMessage::Req { subscription_id, .. } = msg {
                Some(subscription_id.into_owned())
            } else {
                None
            }
        })
        .collect()
}

/// Serve a NIP-11 document on one TCP connection and close.
async fn handle_nip11(
    mut stream: tokio::net::TcpStream,
    body: &'static str,
) {
    let mut buf = vec![0u8; 4096];
    // Consume the HTTP request (just drain what arrives).
    let _ = stream.read(&mut buf).await;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/nostr+json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    // Drop stream → EOF → client's read_to_end returns.
}

/// Relay declares `max_subscriptions: 2`; three demands → two REQs, one shortfall.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relay_max_subscriptions_limits_active_requests() {
    // Start a mini HTTP server on a random port.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind NIP-11 server");
    let port = listener.local_addr().expect("local address").port();

    // NIP-11 body: max_subscriptions = 2.
    let nip11_body: &'static str = r#"{"limitation":{"max_subscriptions":2}}"#;

    // Accept connections in the background; serve each one.
    tokio::spawn(async move {
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                tokio::spawn(handle_nip11(stream, nip11_body));
            }
        }
    });

    // Use a ws:// URL so the NIP-11 fetcher can reach the server.
    let relay_url = RelayUrl::parse(&format!("ws://127.0.0.1:{port}"))
        .expect("relay URL");
    let relay_key = RelaySessionKey {
        relay: relay_url.clone(),
        access: RelayAccess::Public,
    };

    let transport = Arc::new(FakeTransport::new());
    let observer = build_observer(&relay_url, Arc::clone(&transport));

    // Open three observations for distinct authors AND distinct kinds so the
    // planner cannot group them (filters differing on 2 axes are not mergeable).
    let k1 = Keys::generate().public_key();
    let k2 = Keys::generate().public_key();
    let k3 = Keys::generate().public_key();

    let obs1 = observer.open(query_for(&relay_url, k1, Kind::TextNote)).expect("obs1 opens");
    let obs2 = observer.open(query_for(&relay_url, k2, Kind::from(3))).expect("obs2 opens");
    let obs3 = observer.open(query_for(&relay_url, k3, Kind::from(6))).expect("obs3 opens");

    // Wait long enough for NIP-11 fetch + admission window + plan execution.
    tokio::time::sleep(Duration::from_millis(ADMISSION_WINDOW_MS * 3)).await;

    // At most 2 REQs must have been sent.
    let reqs = requests_for(&transport, &relay_key);
    assert!(
        reqs.len() <= 2,
        "max_subscriptions=2 must cap opened REQs at 2, got {}: {:?}",
        reqs.len(),
        reqs
    );

    // The third observation must carry a relay shortfall.
    let shortfall = [&obs1, &obs2, &obs3]
        .iter()
        .find_map(|obs| {
            let ev = obs.current();
            let relay_ev = ev.evidence.relay(&relay_key)?;
            relay_ev.shortfall.clone()
        });
    assert!(
        shortfall.is_some(),
        "one of the three observations must report a relay shortfall when max_subscriptions=2"
    );

    drop((obs1, obs2, obs3));
}
