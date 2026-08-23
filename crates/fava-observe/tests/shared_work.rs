//! Owner-level evidence that demand stays distinct while work is shared.

mod support;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::sync::Arc;

use fava_query::{Query, RelaySourceState, RelayWithdrawal, RouteOrigin};
use fava_router_app_relays::AppRelayRouter;
use fava_routing::Router;
use fava_state::RelayUrl;
use fava_transport::Transport;
use nostr::event::Kind;
use nostr::key::{Keys, PublicKey};
use support::{
    assemble, assemble_with, push, relay, relay_evidence, requests, session_key, settle,
    wait_until, withdrawals,
};

#[tokio::test(flavor = "current_thread")]
async fn equivalent_observations_reach_the_planner_as_two_distinct_demands() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let assembly = assemble();
    let query = Query::events()
        .only_from_relays([shared.clone()])
        .expect("explicit relay is valid");

    let first = assembly
        .observer
        .open(query.clone())
        .expect("first query opens");
    let second = assembly.observer.open(query).expect("second query opens");

    wait_until(|| assembly.planner.widest(&key).len() == 2).await;
    let demand = assembly.planner.widest(&key);
    let owners: BTreeSet<_> = demand.iter().map(|item| item.owner).collect();
    assert_eq!(
        owners,
        BTreeSet::from([first.id(), second.id()]),
        "the owner must not collapse equivalent demand before the planner sees it"
    );
    let identities: BTreeSet<_> = demand.iter().map(RelayDemandId::of).collect();
    assert_eq!(identities.len(), 2, "two observations are two DemandIds");
    assert_eq!(
        assembly.transport.dials(&key),
        1,
        "distinct demand still shares one connection"
    );
    first.close();
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn one_wire_subscription_serves_every_observation_that_shares_it() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("first query opens");
    let second = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("second query opens");

    wait_until(|| !requests(assembly.peer(&shared)).is_empty()).await;
    wait_until(|| relay_evidence(&first, &shared).shared_with.len() == 2).await;
    let sharing = relay_evidence(&first, &shared).shared_with;
    assert!(sharing.contains(&first.id()) && sharing.contains(&second.id()));
    assert_eq!(
        relay_evidence(&second, &shared).shared_with,
        sharing,
        "both holders see the same sharing set"
    );
    assert_eq!(assembly.transport.holders(&key), NonZeroUsize::new(1));
    first.close();
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn an_explicit_and_an_automatic_observation_of_one_filter_stay_distinct() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let alice = Keys::generate().public_key();
    let router: Arc<dyn Router> = Arc::new(AppRelayRouter::new("app-relays", [shared.clone()]));
    let assembly = assemble_with(vec![router]);

    let explicit = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("explicit query opens");
    let automatic = assembly
        .observer
        .open(Query::events().kind(Kind::Metadata).authors([alice]))
        .expect("automatic query opens");

    wait_until(|| assembly.planner.widest(&key).len() == 2).await;
    let demand = assembly.planner.widest(&key);
    assert_eq!(
        demand[0].filter, demand[1].filter,
        "the two demands are filter-equal"
    );
    assert_ne!(
        demand[0].id(),
        demand[1].id(),
        "filter equality must not erase demand identity"
    );

    // Identical filters, different acquisition. The evidence keeps them apart.
    assert_eq!(
        relay_evidence(&explicit, &shared).route,
        RouteOrigin::Explicit
    );
    assert!(matches!(
        relay_evidence(&automatic, &shared).route,
        RouteOrigin::Automatic { .. }
    ));
    assert_eq!(assembly.transport.dials(&key), 1);
    explicit.close();
    automatic.close();
}

#[tokio::test(flavor = "current_thread")]
async fn a_route_withdrawal_leaves_the_explicit_observation_intact() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let alice = Keys::generate().public_key();
    let router: Arc<dyn Router> = Arc::new(AppRelayRouter::new("app-relays", [shared.clone()]));
    let assembly = assemble_with(vec![router]);

    let explicit = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("explicit query opens");
    let automatic = assembly
        .observer
        .open(Query::events().kind(Kind::Metadata).authors([alice]))
        .expect("automatic query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let installed = requests(Some(peer.clone()))[0].0.clone();

    automatic.close();

    settle().await;
    assert!(
        withdrawals(Some(peer.clone())).is_empty(),
        "a request another demand still needs is never withdrawn"
    );
    assert_eq!(
        requests(Some(peer.clone()))[0].0,
        installed,
        "the surviving request keeps its identity and its filter"
    );
    assert_eq!(
        assembly.transport.holders(&key),
        NonZeroUsize::new(1),
        "the shared connection survives one holder leaving"
    );
    let surviving = relay_evidence(&explicit, &shared);
    assert!(
        !matches!(surviving.state, RelaySourceState::Withdrawn { .. }),
        "the explicit observation keeps its relay, got {:?}",
        surviving.state
    );
    assert_eq!(surviving.route, RouteOrigin::Explicit);

    explicit.close();
    wait_until(|| withdrawals(Some(peer.clone())) == vec![installed.clone()]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn the_last_holder_to_close_releases_the_shared_connection() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let assembly = assemble();
    let query = Query::events()
        .only_from_relays([shared.clone()])
        .expect("explicit relay is valid");
    let first = assembly
        .observer
        .open(query.clone())
        .expect("first query opens");
    let second = assembly.observer.open(query).expect("second query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let installed = requests(Some(peer.clone()))[0].0.clone();

    first.close();
    settle().await;
    assert!(
        withdrawals(Some(peer.clone())).is_empty(),
        "the request still serves the second observation"
    );
    assert_eq!(assembly.transport.holders(&key), NonZeroUsize::new(1));

    second.close();
    wait_until(|| assembly.transport.holders(&key).is_none()).await;
    assert_eq!(withdrawals(Some(peer)), vec![installed]);
    assert_eq!(assembly.transport.dials(&key), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_stalled_relay_never_delays_a_reachable_one() {
    let reachable = relay("reachable");
    let stalled = relay("stalled");
    let assembly = assemble();
    assembly.transport.hold_establishment(&session_key(&stalled));

    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([stalled.clone(), reachable.clone()])
                .expect("explicit relays are valid"),
        )
        .expect("the live query opens");

    wait_until(|| requests(assembly.peer(&reachable)).len() == 1).await;
    assert!(assembly.peer(&stalled).is_none());
    assert!(matches!(
        relay_evidence(&observation, &stalled).state,
        RelaySourceState::Planned | RelaySourceState::Connecting
    ));
    observation.close();
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_the_handle_closes_established_work_while_another_relay_stalls() {
    let reachable = relay("reachable");
    let stalled = relay("stalled");
    let assembly = assemble();
    assembly.transport.hold_establishment(&session_key(&stalled));
    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([stalled.clone(), reachable.clone()])
                .expect("explicit relays are valid"),
        )
        .expect("the live query opens");
    wait_until(|| requests(assembly.peer(&reachable)).len() == 1).await;
    let peer = assembly.established(&reachable);
    let installed = requests(Some(peer.clone()))[0].0.clone();

    drop(observation);

    wait_until(|| withdrawals(Some(peer.clone())) == vec![installed.clone()]).await;
    wait_until(|| {
        assembly
            .transport
            .holders(&session_key(&reachable))
            .is_none()
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn an_establishment_that_completes_after_withdrawal_is_released_not_installed() {
    let gated = relay("gated");
    let key = session_key(&gated);
    let assembly = assemble();
    assembly.transport.hold_establishment(&key);
    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([gated.clone()])
                .expect("explicit relay is valid"),
        )
        .expect("the live query opens");
    settle().await;
    assert!(assembly.peer(&gated).is_none());

    assembly.transport.release_establishment(&key);
    drop(observation);

    wait_until(|| assembly.transport.holders(&key).is_none()).await;
    settle().await;
    assert!(
        requests(assembly.peer(&gated)).is_empty(),
        "a superseded establishment must not install demand"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn repeated_close_withdraws_exactly_once() {
    let shared = relay("shared");
    let assembly = assemble();
    let observation = assembly
        .observer
        .open(
            Query::events()
                .only_from_relays([shared.clone()])
                .expect("explicit relay is valid"),
        )
        .expect("the live query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);

    observation.close();
    observation.close();
    wait_until(|| withdrawals(Some(peer.clone())).len() == 1).await;
    settle().await;

    assert_eq!(withdrawals(Some(peer)).len(), 1);
    assert_eq!(assembly.transport.dials(&session_key(&shared)), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn closing_an_observation_records_why_its_relay_demand_ended() {
    let shared = relay("shared");
    let alice = Keys::generate().public_key();
    let router: Arc<dyn Router> = Arc::new(AppRelayRouter::new("app-relays", [shared.clone()]));
    let assembly = assemble_with(vec![router]);
    let observation = assembly
        .observer
        .open(Query::events().kind(Kind::Metadata).authors([alice]))
        .expect("automatic query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;

    let bound = relay_evidence(&observation, &shared);
    assert!(matches!(bound.route, RouteOrigin::Automatic { revision: 1 }));
    assert_eq!(
        RelayWithdrawal::RouteWithdrawn,
        RelayWithdrawal::RouteWithdrawn,
        "route withdrawal is the reason a router-contributed relay leaves"
    );
    observation.close();
}

fn metadata_of(author: PublicKey, relay: &RelayUrl) -> Query {
    Query::events()
        .kind(Kind::Metadata)
        .authors([author])
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid")
}

/// The logical identity of one demand, for set comparison.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RelayDemandId(fava_subscriptions::DemandId);

impl RelayDemandId {
    fn of(demand: &fava_subscriptions::RelayDemand) -> Self {
        Self(demand.id())
    }
}

#[tokio::test(flavor = "current_thread")]
async fn byte_identical_demand_never_produces_two_requests() {
    let shared = relay("shared");
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("first query opens");
    let second = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("second query opens");
    let third = assembly
        .observer
        .open(metadata_of(Keys::generate().public_key(), &shared))
        .expect("third query opens");

    wait_until(|| requests(assembly.peer(&shared)).len() == 2).await;
    settle().await;
    let installed = requests(assembly.peer(&shared));
    assert_eq!(
        installed.len(),
        2,
        "two byte-identical filters share one request; a distinct one gets its own"
    );
    assert_ne!(installed[0].1, installed[1].1);
    first.close();
    second.close();
    third.close();
}

#[tokio::test(flavor = "current_thread")]
async fn demand_arriving_after_the_window_opens_its_own_request_and_closes_nothing() {
    let shared = relay("shared");
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let assembly = assemble();

    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("first query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let installed = requests(Some(peer.clone()))[0].clone();

    // The window has already closed. This demand cannot join that cohort.
    let second = assembly
        .observer
        .open(metadata_of(bob, &shared))
        .expect("second query opens");

    wait_until(|| requests(Some(peer.clone())).len() == 2).await;
    settle().await;
    assert!(
        withdrawals(Some(peer.clone())).is_empty(),
        "a running request is never rewritten to absorb later demand"
    );
    let after = requests(Some(peer.clone()));
    assert_eq!(after[0], installed, "the incumbent is untouched");
    assert_ne!(after[1].0, installed.0);
    first.close();
    second.close();
}

#[tokio::test(flavor = "current_thread")]
async fn a_late_joiner_of_a_completed_request_is_credited_the_earned_completion() {
    let shared = relay("shared");
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("first query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let installed = requests(Some(peer.clone()))[0].0.clone();
    push(&peer, &fava_wire::RelayMessage::eose(installed));
    wait_until(|| relay_evidence(&first, &shared).stored_events_complete()).await;

    let second = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("second query opens");

    wait_until(|| relay_evidence(&second, &shared).stored_events_complete()).await;
    settle().await;
    assert_eq!(
        requests(Some(peer)).len(),
        1,
        "the joiner attaches; it does not re-ask the relay"
    );
    first.close();
    second.close();
}
