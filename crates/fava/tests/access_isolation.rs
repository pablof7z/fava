//! Exact relay access remains isolated through the public Fava facade.

use std::sync::Arc;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{QueryEvidence, RelaySourceState};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions_no_grouping::planner;
use fava_transport::Transport;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[tokio::test(flavor = "current_thread")]
#[allow(
    clippy::too_many_lines,
    reason = "one facade flow proves identity, scripted result, occurrence, and withdrawal isolation"
)]
async fn query_access_survives_facade_planner_transport_observation_lifecycle() {
    let relay = RelayUrl::parse("wss://access-isolation.example").expect("relay URL");
    let public_key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    let authenticated_key = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Authenticated(Keys::generate().public_key()),
    };
    let transport = Arc::new(FakeTransport::new());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .build()
        .expect("assembly is complete");

    let public_query = Query::events()
        .only_from_relays([relay.clone()])
        .expect("relay selection")
        .with_relay_access(public_key.access.clone());
    let authenticated_query = Query::events()
        .only_from_relays([relay])
        .expect("relay selection")
        .with_relay_access(authenticated_key.access.clone());
    assert_ne!(
        public_query, authenticated_query,
        "exact access is part of facade query identity"
    );
    let public = fava
        .observe(public_query)
        .await
        .expect("public observation opens");
    let authenticated = fava
        .observe(authenticated_query)
        .await
        .expect("authenticated observation opens");

    wait_until(|| {
        transport.holders(&public_key).is_some() && transport.holders(&authenticated_key).is_some()
    })
    .await;
    assert_eq!(transport.dials(&public_key), 1);
    assert_eq!(transport.dials(&authenticated_key), 1);

    let public_peer = transport.relay(&public_key).expect("public peer");
    let private_peer = transport
        .relay(&authenticated_key)
        .expect("authenticated peer");
    wait_until(|| request(&public_peer).is_some() && request(&private_peer).is_some()).await;
    let public_wire = request(&public_peer).expect("public REQ");
    let private_wire = request(&private_peer).expect("authenticated REQ");
    wait_until(|| {
        plan_is_exact(&public.current().evidence, &public_key)
            && plan_is_exact(&authenticated.current().evidence, &authenticated_key)
    })
    .await;
    assert!(
        public
            .current()
            .evidence
            .relay(&authenticated_key)
            .is_none()
    );
    assert!(
        authenticated
            .current()
            .evidence
            .relay(&public_key)
            .is_none()
    );

    push(&public_peer, &RelayMessage::eose(public_wire.clone()));
    wait_until(|| {
        public
            .current()
            .evidence
            .relay(&public_key)
            .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete)
    })
    .await;
    assert!(matches!(
        authenticated
            .current()
            .evidence
            .relay(&authenticated_key)
            .map(|item| &item.state),
        Some(RelaySourceState::Open { .. })
    ));

    let keys = Keys::generate();
    let public_only = event(&keys, "public only");
    let private_only = event(&keys, "authenticated only");
    let shared = event(&keys, "shared signed event");
    push(
        &public_peer,
        &RelayMessage::event(public_wire.clone(), public_only.clone()),
    );
    push(
        &private_peer,
        &RelayMessage::event(private_wire.clone(), private_only.clone()),
    );
    push(
        &public_peer,
        &RelayMessage::event(public_wire, shared.clone()),
    );
    push(
        &private_peer,
        &RelayMessage::event(private_wire, shared.clone()),
    );
    wait_until(|| {
        public
            .current()
            .events
            .iter()
            .any(|item| item.id() == shared.id)
            && authenticated
                .current()
                .events
                .iter()
                .any(|item| item.id() == shared.id)
    })
    .await;

    let public_snapshot = public.current();
    let private_snapshot = authenticated.current();
    assert!(
        public_snapshot
            .events
            .iter()
            .any(|item| item.id() == public_only.id)
    );
    assert!(
        public_snapshot
            .events
            .iter()
            .all(|item| item.id() != private_only.id)
    );
    assert!(
        private_snapshot
            .events
            .iter()
            .any(|item| item.id() == private_only.id)
    );
    assert!(
        private_snapshot
            .events
            .iter()
            .all(|item| item.id() != public_only.id)
    );
    for (snapshot, expected) in [
        (public_snapshot.as_ref(), &public_key),
        (private_snapshot.as_ref(), &authenticated_key),
    ] {
        let record = snapshot
            .events
            .iter()
            .find(|item| item.id() == shared.id)
            .expect("shared event selected");
        let occurrences = record.relay_occurrences().occurrences().collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 1);
        assert_eq!(&occurrences[0].session, expected);
    }

    let public_generation = public
        .current()
        .evidence
        .relay(&public_key)
        .expect("public lifecycle evidence")
        .generation;
    let authenticated_generation = authenticated
        .current()
        .evidence
        .relay(&authenticated_key)
        .expect("authenticated lifecycle evidence")
        .generation;
    private_peer.reconnect();
    wait_until(|| requests(&private_peer).len() == 2).await;
    wait_until(|| {
        authenticated
            .current()
            .evidence
            .relay(&authenticated_key)
            .is_some_and(|item| item.generation > authenticated_generation)
    })
    .await;
    assert_eq!(
        public
            .current()
            .evidence
            .relay(&public_key)
            .expect("public lifecycle remains installed")
            .generation,
        public_generation
    );

    public.close();
    wait_until(|| transport.holders(&public_key).is_none()).await;
    assert!(transport.holders(&authenticated_key).is_some());
    let private_after_public_close = event(&keys, "private after public withdrawal");
    let reconnected_wire = request(&private_peer).expect("authenticated reconnect REQ");
    push(
        &private_peer,
        &RelayMessage::event(reconnected_wire, private_after_public_close.clone()),
    );
    wait_until(|| {
        authenticated
            .current()
            .events
            .iter()
            .any(|item| item.id() == private_after_public_close.id)
    })
    .await;
    authenticated.close();
}

fn plan_is_exact(evidence: &QueryEvidence, expected: &RelaySessionKey) -> bool {
    evidence
        .plan
        .as_ref()
        .is_some_and(|plan| plan.relays == [expected.clone()] && plan.installed == 1)
        && evidence
            .relay(expected)
            .is_some_and(|relay| relay.session == *expected)
}

fn event(keys: &Keys, content: &str) -> Event {
    EventBuilder::new(Kind::TextNote, content)
        .finalize(keys)
        .expect("event signs")
}

fn push(peer: &FakeRelay, message: &RelayMessage<'_>) {
    peer.push_frame(serde_json::to_vec(message).expect("relay message encodes"));
}

fn request(peer: &FakeRelay) -> Option<SubscriptionId> {
    requests(peer).into_iter().last()
}

fn requests(peer: &FakeRelay) -> Vec<SubscriptionId> {
    peer.delivered_frames()
        .into_iter()
        .filter_map(|frame| {
            match serde_json::from_slice::<ClientMessage<'static>>(&frame)
                .expect("client message decodes")
            {
                ClientMessage::Req {
                    subscription_id, ..
                } => Some(subscription_id.into_owned()),
                _ => None,
            }
        })
        .collect()
}

async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline");
}
