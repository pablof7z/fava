//! Exact relay access remains isolated through the public Fava facade.

use std::sync::Arc;

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{QueryEvidence, RelaySourceState};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{Authentication, Authority};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{RelaySessionExt, Transport};
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
    let alice = Keys::generate().public_key();
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
        .with_relay_access(Authority::Unauthenticated);
    let authenticated_query = Query::events()
        .only_from_relays([relay.clone()])
        .expect("relay selection")
        .with_relay_access(Authority::As(alice));
    assert_ne!(
        public_query, authenticated_query,
        "exact access is part of facade query identity"
    );

    // The authenticated observation opens and authenticates first, so its
    // connection is committed to alice before the public observation ever
    // asks: a connection already authenticated as alice can never become
    // anonymous again, so the two are guaranteed distinct connections.
    let authenticated = fava
        .observe(authenticated_query)
        .await
        .expect("authenticated observation opens");
    wait_until(|| transport.relay(&relay, &Authority::As(alice)).is_some()).await;
    let private_peer = transport.relay(&relay, &Authority::As(alice)).unwrap();
    let private_session = transport
        .session(&relay, &Authority::As(alice))
        .expect("the watch acquired this session");
    RelaySessionExt::record_authentication(
        &private_session,
        Authentication::Authenticated { as_of: alice },
    );

    let public = fava
        .observe(public_query)
        .await
        .expect("public observation opens");

    wait_until(|| {
        transport
            .holders(&relay, &Authority::Unauthenticated)
            .is_some()
            && transport.holders(&relay, &Authority::As(alice)).is_some()
    })
    .await;
    assert_eq!(transport.dials(&relay), 2);

    let public_peer = transport
        .relay(&relay, &Authority::Unauthenticated)
        .expect("public peer");
    wait_until(|| request(&public_peer).is_some() && request(&private_peer).is_some()).await;
    let public_wire = request(&public_peer).expect("public REQ");
    let private_wire = request(&private_peer).expect("authenticated REQ");
    assert_ne!(public_wire, private_wire);
    wait_until(|| {
        plan_is_exact(&public.current().evidence, &relay)
            && plan_is_exact(&authenticated.current().evidence, &relay)
    })
    .await;

    push(&public_peer, &RelayMessage::eose(public_wire.clone()));
    wait_until(|| {
        public
            .current()
            .evidence
            .relay(&relay)
            .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete)
    })
    .await;
    assert!(matches!(
        authenticated
            .current()
            .evidence
            .relay(&relay)
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
    for snapshot in [public_snapshot.as_ref(), private_snapshot.as_ref()] {
        let record = snapshot
            .events
            .iter()
            .find(|item| item.id() == shared.id)
            .expect("shared event selected");
        let occurrences = record.relay_occurrences().occurrences().collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].session, relay);
    }

    public.close();
    wait_until(|| {
        transport
            .holders(&relay, &Authority::Unauthenticated)
            .is_none()
    })
    .await;
    assert!(transport.holders(&relay, &Authority::As(alice)).is_some());
    authenticated.close();
}

fn plan_is_exact(evidence: &QueryEvidence, expected: &RelayUrl) -> bool {
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
    peer.push_frame(&serde_json::to_vec(message).expect("relay message encodes"));
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
