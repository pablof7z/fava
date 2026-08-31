//! Public-facade explicit live-query evidence over the neutral transport fake.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query::{ObservationId, QuerySnapshot, RelaySourceState, RouteOrigin};
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions::SubscriptionPlanner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::Transport;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::{Keys, PublicKey};
use nostr::types::{RelayUrl, Timestamp};

#[tokio::test(flavor = "current_thread")]
async fn relay_establishment_does_not_delay_the_coherent_local_observation() {
    let relay = relay("pending");
    let transport = Arc::new(FakeTransport::new());
    transport.hold_establishment(&session_key(&relay));
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        no_grouping(),
    );

    let observation = tokio::time::timeout(
        Duration::from_millis(50),
        fava.observe(
            Query::events()
                .only_from_relays([relay.clone()])
                .expect("explicit relay is valid"),
        ),
    )
    .await
    .expect("local observation must not await relay establishment")
    .expect("local observation opens");

    assert!(observation.current().events.is_empty());
    assert_eq!(transport.dials(&session_key(&relay)), 0);
    let planned = relay_occurrence(&observation.current(), &session_key(&relay));
    assert!(matches!(
        planned.state,
        RelaySourceState::Planned | RelaySourceState::Connecting
    ));
    assert_eq!(planned.route, RouteOrigin::Explicit);
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn equivalent_observations_share_one_relay_connection() {
    let relay = relay("shared");
    let key = session_key(&relay);
    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        no_grouping(),
    );
    let query = Query::events()
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid");

    let first = fava
        .observe(query.clone())
        .await
        .expect("first query opens");
    let second = fava.observe(query).await.expect("second query opens");

    assert_ne!(first.id(), second.id());
    wait_until(|| transport.holders(&key) == NonZeroUsize::new(1)).await;
    settle().await;
    assert_eq!(
        transport.dials(&key),
        1,
        "the second observation must reuse the session Fava already holds"
    );

    first.close();
    settle().await;
    assert_eq!(transport.holders(&key), NonZeroUsize::new(1));

    second.close();
    wait_until(|| transport.holders(&key).is_none()).await;
    assert_eq!(transport.dials(&key), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn one_admission_cohort_groups_and_the_running_request_is_never_rewritten() {
    let relay = relay("grouped");
    let key = session_key(&relay);
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();

    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        Arc::new(StandardSubscriptionPlanner::new()),
    );
    // One burst: three logical demands inside one admission window.
    let first = fava
        .observe(metadata_of(alice, &relay))
        .await
        .expect("first query opens");
    let second = fava
        .observe(metadata_of(alice, &relay))
        .await
        .expect("second query opens");
    let third = fava
        .observe(metadata_of(bob, &relay))
        .await
        .expect("third query opens");

    wait_until(|| requests(session(&transport, &key)).len() == 1).await;
    settle().await;
    let peer = established(&transport, &key);
    let grouped = requests(Some(peer.clone()));
    assert_eq!(
        grouped.len(),
        1,
        "the standard planner carries one cohort's three demands in one REQ"
    );
    let (merged, filters) = grouped[0].clone();
    let authors = filters[0].authors.clone().unwrap_or_default();
    assert!(authors.contains(&alice) && authors.contains(&bob));
    assert_eq!(transport.dials(&key), 1);

    // The refcount is the demand the request serves, not the observation
    // count: losing one of two identical demands changes nothing.
    first.close();
    settle().await;
    assert_eq!(requests(Some(peer.clone())), grouped);
    assert!(withdrawals(Some(peer.clone())).is_empty());

    // Losing the last demand for one author leaves the survivor running with
    // its over-broad filter. Narrowing would cost a full relay re-serve.
    second.close();
    settle().await;
    assert_eq!(
        requests(Some(peer.clone())),
        grouped,
        "a running request is never rewritten to narrow it"
    );
    assert!(withdrawals(Some(peer.clone())).is_empty());
    assert_eq!(transport.holders(&key), NonZeroUsize::new(1));

    third.close();
    wait_until(|| withdrawals(Some(peer.clone())) == vec![merged.clone()]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn demand_arriving_after_the_window_never_touches_the_running_request() {
    let relay = relay("late");
    let key = session_key(&relay);
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();

    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        Arc::new(StandardSubscriptionPlanner::new()),
    );
    let first = fava
        .observe(metadata_of(alice, &relay))
        .await
        .expect("first query opens");
    wait_until(|| requests(session(&transport, &key)).len() == 1).await;
    let peer = established(&transport, &key);
    let installed = requests(Some(peer.clone()))[0].clone();

    // The cohort that carried the first demand is frozen. This one is late.
    let second = fava
        .observe(metadata_of(bob, &relay))
        .await
        .expect("second query opens");

    wait_until(|| requests(Some(peer.clone())).len() == 2).await;
    settle().await;
    assert!(
        withdrawals(Some(peer.clone())).is_empty(),
        "a merge that widens a live REQ would make the relay re-serve its whole window"
    );
    let after = requests(Some(peer.clone()));
    assert_eq!(after[0], installed, "the incumbent keeps its id and filter");
    assert_ne!(after[1].0, installed.0);
    assert_eq!(transport.dials(&key), 1);
    first.close();
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn a_planner_that_never_groups_still_shares_one_connection() {
    let relay = relay("ungrouped");
    let key = session_key(&relay);
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let carol = Keys::generate().public_key();

    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        no_grouping(),
    );
    let first = fava
        .observe(metadata_of(alice, &relay))
        .await
        .expect("first query opens");
    let second = fava
        .observe(metadata_of(bob, &relay))
        .await
        .expect("second query opens");
    let third = fava
        .observe(metadata_of(carol, &relay))
        .await
        .expect("third query opens");

    wait_until(|| requests(session(&transport, &key)).len() == 3).await;
    settle().await;
    let peer = established(&transport, &key);
    assert_eq!(
        transport.dials(&key),
        1,
        "three requests still share one connection"
    );

    let installed = requests(Some(peer.clone()));
    first.close();
    second.close();
    wait_until(|| withdrawals(Some(peer.clone())).len() == 2).await;
    settle().await;
    let withdrawn = withdrawals(Some(peer.clone()));
    let surviving: Vec<SubscriptionId> = installed
        .iter()
        .map(|(id, _)| id.clone())
        .filter(|id| !withdrawn.contains(id))
        .collect();
    assert_eq!(surviving.len(), 1, "the third demand keeps its own request");
    assert_eq!(transport.holders(&key), NonZeroUsize::new(1));
    third.close();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelling_observe_while_another_relay_opens_closes_provisional_work() {
    let reachable = relay("a-open");
    let stalled = relay("b-pending");
    let transport = Arc::new(FakeTransport::new());
    transport.hold_establishment(&session_key(&stalled));
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        no_grouping(),
    );
    let query = Query::events()
        .only_from_relays([reachable.clone(), stalled.clone()])
        .expect("explicit relays are valid");

    let observation = tokio::time::timeout(Duration::from_millis(50), fava.observe(query))
        .await
        .expect("a stalled relay must not delay the handle")
        .expect("local observation opens");

    let key = session_key(&reachable);
    wait_until(|| !requests(session(&transport, &key)).is_empty()).await;
    let peer = established(&transport, &key);
    let installed = requests(Some(peer.clone()))[0].0.clone();
    assert!(
        transport.relay(&session_key(&stalled)).is_none(),
        "the stalled relay never established"
    );

    drop(observation);

    wait_until(|| withdrawals(Some(peer.clone())) == vec![installed.clone()]).await;
    wait_until(|| transport.holders(&key).is_none()).await;
    assert_eq!(transport.dials(&key), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn explicit_live_query_attributes_event_eose_and_exact_cancellation() {
    let relay = relay("relay");
    let key = session_key(&relay);
    let transport = Arc::new(FakeTransport::new());
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache), &transport, no_grouping());
    let keys = Keys::generate();
    let event = EventBuilder::new(Kind::TextNote, "stored")
        .custom_created_at(Timestamp::from(10))
        .finalize(&keys)
        .expect("event signs");
    let query = Query::events()
        .authors([keys.public_key()])
        .expect("one author is bounded")
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid");

    let mut observation = fava.observe(query).await.expect("live query opens");
    wait_until(|| !requests(session(&transport, &key)).is_empty()).await;
    let subscription = requests(session(&transport, &key))[0].0.clone();
    let peer = established(&transport, &key);
    push(
        &peer,
        &RelayMessage::event(subscription.clone(), event.clone()),
    );
    push(&peer, &RelayMessage::eose(subscription.clone()));

    let snapshot = observation
        .wait_until(Duration::from_secs(1), |snapshot| {
            !snapshot.events.is_empty()
        })
        .await
        .expect("observation stays open")
        .expect("event deadline");
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), event.id);
    assert_eq!(snapshot.events[0].relay_occurrences().len(), 1);
    wait_until(|| relay_occurrence(&observation.current(), &key).stored_events_complete()).await;

    let mut forged = event.clone();
    forged.content = "forged after signing".to_owned();
    let off_filter = EventBuilder::new(Kind::TextNote, "other author")
        .finalize(&Keys::generate())
        .expect("event signs");
    push(&peer, &RelayMessage::event(subscription.clone(), forged));
    push(
        &peer,
        &RelayMessage::event(subscription.clone(), off_filter),
    );
    settle().await;
    assert_eq!(cache.len().expect("cache readable"), 1);

    observation.close();
    observation.close();
    wait_until(|| withdrawals(Some(peer.clone())) == vec![subscription.clone()]).await;
    assert!(observation.changed().await.is_err());
    assert_eq!(cache.len().expect("cache readable"), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn silence_eose_auth_closed_and_disconnect_are_distinct_facts() {
    let relay = relay("relay");
    let key = session_key(&relay);
    let transport = Arc::new(FakeTransport::new());
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        &transport,
        no_grouping(),
    );
    let mut observation = fava
        .observe(
            Query::events()
                .only_from_relays([relay.clone()])
                .expect("explicit relay is valid"),
        )
        .await
        .expect("live query opens");
    wait_until(|| !requests(session(&transport, &key)).is_empty()).await;
    let subscription = requests(session(&transport, &key))[0].0.clone();
    let peer = established(&transport, &key);

    wait_until(|| {
        matches!(
            relay_occurrence(&observation.current(), &key).state,
            RelaySourceState::Open { .. }
        )
    })
    .await;
    let silent = relay_occurrence(&observation.current(), &key);
    assert!(
        !silent.stored_events_complete(),
        "silence is not completeness"
    );
    assert!(silent.is_live());

    push(&peer, &RelayMessage::eose(subscription.clone()));
    let complete = await_state(&mut observation, &key, |state| {
        matches!(state, RelaySourceState::StoredEventsComplete { .. })
    })
    .await;
    assert!(complete.stored_events_complete());

    // A challenge is not this owner's fact. It belongs to whoever authenticates
    // the session, and reaches it on the session's own challenge reader; the
    // observation owner no longer decodes one and never reports it.
    push(&peer, &RelayMessage::auth("challenge"));
    settle().await;
    assert!(
        matches!(
            relay_occurrence(&observation.current(), &key).state,
            RelaySourceState::StoredEventsComplete { .. }
        ),
        "an unsolicited challenge changes nothing this owner observes"
    );

    push(
        &peer,
        &RelayMessage::closed(subscription.clone(), "rate-limited"),
    );
    let refused = await_state(&mut observation, &key, |state| {
        matches!(state, RelaySourceState::Refused { .. })
    })
    .await;
    let RelaySourceState::Refused { message, .. } = &refused.state else {
        panic!("expected a refusal, got {:?}", refused.state);
    };
    assert_eq!(message.as_str(), "rate-limited");

    peer.fail_now("injected");
    let dropped = await_state(&mut observation, &key, |state| {
        matches!(
            state,
            RelaySourceState::Disconnected { .. } | RelaySourceState::Unreachable { .. }
        )
    })
    .await;
    match &dropped.state {
        RelaySourceState::Disconnected { detail }
        | RelaySourceState::Unreachable { detail, .. } => {
            assert!(
                detail.as_str().contains("injected"),
                "a disconnect must carry the relay's own verbatim reason, got {detail:?}"
            );
        }
        other => panic!("expected a disconnect, got {other:?}"),
    }
    observation.close();
}

// ----------------------------------------------------------------- harness

fn assembly<P>(
    cache: Arc<MemoryEventCache>,
    transport: &Arc<FakeTransport>,
    planner: Arc<P>,
) -> Fava
where
    P: SubscriptionPlanner + 'static,
{
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(planner)
        .transport(Arc::clone(transport))
        .build()
        .expect("assembly is complete")
}

fn no_grouping() -> Arc<impl SubscriptionPlanner> {
    Arc::new(fava_subscriptions_no_grouping::planner())
}

fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn session_key(relay: &RelayUrl) -> RelaySessionKey {
    RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    }
}

fn metadata_of(author: PublicKey, relay: &RelayUrl) -> Query {
    Query::events()
        .kinds([Kind::Metadata])
        .expect("one kind is bounded")
        .authors([author])
        .expect("one author is bounded")
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid")
}

fn session(transport: &FakeTransport, key: &RelaySessionKey) -> Option<FakeRelay> {
    transport.relay(key)
}

fn established(transport: &FakeTransport, key: &RelaySessionKey) -> FakeRelay {
    transport.relay(key).expect("the relay session established")
}

fn push(peer: &FakeRelay, message: &RelayMessage<'_>) {
    peer.push_frame(
        serde_json::to_string(message)
            .expect("message encodes")
            .as_bytes(),
    );
}

fn client_messages(peer: Option<FakeRelay>) -> Vec<ClientMessage<'static>> {
    peer.map(|peer| peer.delivered_frames())
        .unwrap_or_default()
        .into_iter()
        .map(|frame| {
            serde_json::from_slice::<ClientMessage<'static>>(&frame)
                .expect("client message decodes")
        })
        .collect()
}

fn requests(peer: Option<FakeRelay>) -> Vec<(SubscriptionId, Vec<nostr::filter::Filter>)> {
    client_messages(peer)
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

fn withdrawals(peer: Option<FakeRelay>) -> Vec<SubscriptionId> {
    client_messages(peer)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Close(id) => Some(id.into_owned()),
            _ => None,
        })
        .collect()
}

fn relay_occurrence(
    snapshot: &QuerySnapshot,
    key: &RelaySessionKey,
) -> fava_query::RelayQueryEvidence {
    snapshot
        .evidence
        .relay(key)
        .cloned()
        .expect("the observation reports evidence for every relay it uses")
}

async fn await_state(
    observation: &mut fava::Observation,
    key: &RelaySessionKey,
    predicate: impl Fn(&RelaySourceState) -> bool,
) -> fava_query::RelayQueryEvidence {
    let snapshot = observation
        .wait_until(Duration::from_secs(1), |snapshot| {
            predicate(&relay_occurrence(snapshot, key).state)
        })
        .await
        .expect("observation stays open")
        .expect("relay state deadline elapsed");
    relay_occurrence(&snapshot, key)
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

fn _unused(_: ObservationId, _: Event) {}
