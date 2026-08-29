//! Why one relay's demand ended, and how many CLOSEs ending it costs.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use fava_query::{Query, RelaySourceState, RouteOrigin};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_testkit::DelayedRouter;
use fava_routing::{CoverageState, RouteContribution, RouteDestination, RouteTarget, Router};
use fava_transport::Transport;
use nostr::event::Kind;
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;
use support::{
    assemble, assemble_with, relay, relay_evidence, requests, session_key, settle, wait_until,
    withdrawals,
};

/// An automatic route retains its original attributed acquisition evidence.
#[tokio::test(flavor = "current_thread")]
async fn automatic_route_keeps_its_initial_attributed_evidence() {
    let dropped = relay("dropped");
    let alice = Keys::generate().public_key();
    let routes = Arc::new(DelayedRouter::new("routes", contribution(&dropped)));
    let assembly = assemble_with(vec![Arc::clone(&routes) as Arc<dyn Router>]);

    let observation = assembly
        .observer
        .open(automatic(alice))
        .expect("the automatic query opens");
    wait_until(|| requests(assembly.peer(&dropped)).len() == 1).await;
    let bound = relay_evidence(&observation, &dropped);
    assert!(matches!(
        bound.route,
        RouteOrigin::Automatic { revision: 1 }
    ));
    assert!(!matches!(bound.state, RelaySourceState::Withdrawn { .. }));

    observation.close();
}

/// One shared request is withdrawn exactly once, however its holders leave.
///
/// The interesting case is not repeated `close()` on a single handle — that is
/// `BTreeMap::remove` returning `None`. It is two holders of the *same* wire
/// subscription leaving by different routes inside one scheduler turn: one
/// explicitly, one by `Drop`, with the explicitly-closed handle then dropped as
/// well. A refcount keyed on observation count, or a `Drop` that re-issues the
/// withdrawal its explicit `close()` already performed, sends a second CLOSE
/// for a subscription that is already gone.
#[tokio::test(flavor = "current_thread")]
async fn close_and_drop_in_one_turn_withdraw_a_shared_request_exactly_once() {
    let shared = relay("shared");
    let key = session_key(&shared);
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let first = assembly
        .observer
        .open(explicit(alice, &shared))
        .expect("the first query opens");
    let second = assembly
        .observer
        .open(explicit(alice, &shared))
        .expect("the second query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let installed = requests(Some(peer.clone()))[0].0.clone();
    wait_until(|| relay_evidence(&first, &shared).shared_with.len() == 2).await;

    // No yield between the departures: the explicit close, the survivor's drop,
    // and the drop of the already-closed handle all land in one turn.
    first.close();
    drop(second);
    drop(first);

    wait_until(|| !withdrawals(Some(peer.clone())).is_empty()).await;
    settle().await;
    assert_eq!(
        withdrawals(Some(peer)),
        vec![installed],
        "one wire subscription earns exactly one CLOSE"
    );
    assert_eq!(assembly.transport.dials(&key), 1);
    wait_until(|| assembly.transport.holders(&key).is_none()).await;
}

fn automatic(author: PublicKey) -> Query {
    Query::events()
        .kinds([Kind::Metadata])
        .expect("one kind is bounded")
        .authors([author])
        .expect("one author is bounded")
}

fn explicit(author: PublicKey, relay: &RelayUrl) -> Query {
    Query::events()
        .kinds([Kind::Metadata])
        .expect("one kind is bounded")
        .authors([author])
        .expect("one author is bounded")
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid")
}

/// A complete router contribution naming exactly one relay.
fn contribution(relay: &RelayUrl) -> RouteContribution {
    let session = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    RouteContribution {
        destinations: vec![RouteDestination::new(
            session.clone(),
            BTreeSet::from([RouteTarget::WholeRequest]),
            "test route",
        )],
        coverage: BTreeMap::from([(
            RouteTarget::WholeRequest,
            CoverageState::Covered(BTreeSet::from([session])),
        )]),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}
