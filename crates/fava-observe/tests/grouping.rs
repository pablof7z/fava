//! Owner-level evidence over the *grouping* planner.
//!
//! Everything else in this crate's corpus runs the no-grouping planner, which
//! mints one wire subscription per logical demand. That makes the owner's
//! refcount easy to read but leaves the interesting interaction untested: when
//! the planner merges a cohort, one request carries a filter that *no demand
//! asked for*, and the owner's attach and withdrawal decisions have to be taken
//! against that merged filter rather than against a demand's own.

mod support;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use fava_query::Query;
use fava_transport::Transport;
use nostr::event::Kind;
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;
use support::{
    assemble_grouping, push, relay, relay_evidence, requests, session_key, settle, wait_until,
    withdrawals,
};

/// One merged request serves demands whose filters were never equal, keeps its
/// merged filter while any of them survives, and is withdrawn once at the end.
///
/// Deliberate breaks: make `Engine::attach` a no-op (the late joiner is never
/// credited the merged request and reports no sharing); narrow the merged
/// request when a demand leaves (the surviving request's filter changes);
/// refcount on observation count (a CLOSE arrives at the first departure).
#[tokio::test(flavor = "current_thread")]
async fn a_merged_request_absorbs_a_late_joiner_and_survives_until_its_last_demand_leaves() {
    let shared = relay("grouped");
    let key = session_key(&shared);
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let assembly = assemble_grouping();

    // One cohort, two filters that are not equal. The planner merges them.
    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("the first query opens");
    let second = assembly
        .observer
        .open(metadata_of(bob, &shared))
        .expect("the second query opens");

    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    settle().await;
    let peer = assembly.established(&shared);
    let merged = requests(Some(peer.clone()));
    assert_eq!(
        merged.len(),
        1,
        "one cohort of two demands is one merged request, got {merged:?}"
    );
    let authors = merged[0].1[0].authors.clone().unwrap_or_default();
    assert_eq!(
        authors,
        BTreeSet::from([alice, bob]),
        "the merged filter is the union of the cohort"
    );
    assert_eq!(assembly.transport.dials(&key), 1);

    // The window is closed. This demand asked for a filter no request carries
    // verbatim, but the merged one physically carries its traffic, so it must
    // attach rather than open a second request.
    let joiner = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("the late query opens");

    wait_until(|| relay_evidence(&joiner, &shared).shared_with.len() == 3).await;
    settle().await;
    assert_eq!(
        requests(Some(peer.clone())),
        merged,
        "a joiner the merged request already carries never asks the relay again"
    );
    assert!(
        withdrawals(Some(peer.clone())).is_empty(),
        "attaching is a refcount edit, not wire work"
    );
    let sharing = relay_evidence(&joiner, &shared).shared_with;
    assert!(
        sharing.contains(&first.id()) && sharing.contains(&second.id()),
        "the joiner shares the merged request with both original holders"
    );

    // Two of the three demands leave. The survivor keeps the merged filter it
    // never asked for; narrowing would cost the relay a full re-serve.
    first.close();
    joiner.close();
    settle().await;
    assert_eq!(
        requests(Some(peer.clone())),
        merged,
        "a running request is never rewritten to narrow it"
    );
    assert!(withdrawals(Some(peer.clone())).is_empty());
    assert_eq!(assembly.transport.holders(&key), NonZeroUsize::new(1));

    second.close();
    wait_until(|| withdrawals(Some(peer.clone())) == vec![merged[0].0.clone()]).await;
    wait_until(|| assembly.transport.holders(&key).is_none()).await;
    assert_eq!(assembly.transport.dials(&key), 1);
}

/// A joiner of a merged request whose replay already ended is credited it.
///
/// The no-grouping path credits a joiner whose filter is byte-identical to the
/// incumbent's. Here the incumbent's filter is a union the joiner never asked
/// for, so the credit depends on containment against a merged filter.
#[tokio::test(flavor = "current_thread")]
async fn a_joiner_of_a_completed_merged_request_is_credited_its_completion() {
    let shared = relay("grouped");
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let assembly = assemble_grouping();

    let first = assembly
        .observer
        .open(metadata_of(alice, &shared))
        .expect("the first query opens");
    let second = assembly
        .observer
        .open(metadata_of(bob, &shared))
        .expect("the second query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let peer = assembly.established(&shared);
    let merged = requests(Some(peer.clone()))[0].0.clone();

    push(&peer, &fava_wire::RelayMessage::eose(merged.clone()));
    wait_until(|| relay_evidence(&first, &shared).stored_events_complete()).await;

    let joiner = assembly
        .observer
        .open(metadata_of(bob, &shared))
        .expect("the late query opens");

    wait_until(|| relay_evidence(&joiner, &shared).stored_events_complete()).await;
    settle().await;
    assert_eq!(
        requests(Some(peer)).len(),
        1,
        "the joiner is credited the merged request's completion, not re-asked"
    );
    first.close();
    second.close();
    joiner.close();
}

fn metadata_of(author: PublicKey, relay: &RelayUrl) -> Query {
    Query::events()
        .kind(Kind::Metadata)
        .authors([author])
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid")
}
