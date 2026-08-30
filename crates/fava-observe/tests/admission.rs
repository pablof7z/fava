//! Owner-level evidence for the admission decision: *when* uncovered demand
//! reaches the wire, and *which* demand a running request already carries.

mod support;

use std::collections::BTreeSet;
use std::time::Duration;

use fava_query::Query;
use nostr::event::Kind;
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;
use support::{assemble, relay, requests, session_key, settle, wait_until};

/// The owner's fixed admission window, restated here because the constant it
/// mirrors (`crates/fava-observe/src/admission.rs`) is crate-private.
const ADMISSION_WINDOW: Duration = Duration::from_millis(10);

/// A present-but-empty author list is *unconstrained* on the wire.
///
/// `nostr::Filter::match_event` short-circuits every value axis on
/// `is_empty() || contains(..)`, so `authors: []` matches every author exactly
/// as an absent `authors` does. A request that names one author therefore does
/// not carry this demand's traffic, and judging it covered is silent
/// under-fetch: the owner arms no window, the planner is never called, and the
/// demand never reaches the relay at all.
#[tokio::test(flavor = "current_thread")]
async fn an_unconstrained_demand_is_not_covered_by_a_narrow_running_request() {
    let shared = relay("coverage");
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let narrow = assembly
        .observer
        .open(metadata_of([alice], &shared))
        .expect("the narrow query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let running = requests(assembly.peer(&shared))[0].clone();

    let unconstrained = assembly
        .observer
        .open(metadata_of([], &shared))
        .expect("the unconstrained query opens");

    wait_until(|| requests(assembly.peer(&shared)).len() == 2).await;
    settle().await;
    let sent = requests(assembly.peer(&shared));
    assert_eq!(
        sent[0], running,
        "the running request is never rewritten to absorb the wider demand"
    );
    assert_ne!(sent[1].0, running.0, "the wider demand asks for its own");
    assert!(
        sent[1].1[0].authors.as_ref().is_none_or(BTreeSet::is_empty),
        "the second request must carry the unconstrained author axis, got {:?}",
        sent[1].1[0].authors
    );
    narrow.close();
    unconstrained.close();
}

/// A demand a running request *does* physically carry attaches to it.
///
/// The mirror of the case above, so the fix cannot be "never covered".
#[tokio::test(flavor = "current_thread")]
async fn a_narrow_demand_attaches_to_an_unconstrained_running_request() {
    let shared = relay("coverage");
    let alice = Keys::generate().public_key();
    let assembly = assemble();

    let unconstrained = assembly
        .observer
        .open(metadata_of([], &shared))
        .expect("the unconstrained query opens");
    wait_until(|| requests(assembly.peer(&shared)).len() == 1).await;
    let running = requests(assembly.peer(&shared))[0].clone();

    let narrow = assembly
        .observer
        .open(metadata_of([alice], &shared))
        .expect("the narrow query opens");

    settle().await;
    settle().await;
    assert_eq!(
        requests(assembly.peer(&shared)),
        vec![running],
        "a demand whose events are already arriving needs no request of its own"
    );
    unconstrained.close();
    narrow.close();
}

/// Repeated arming while a window is pending never extends it.
///
/// The fixed anchor exists so that demand arriving continuously still reaches
/// the wire promptly, and so that the planner is invoked once per window rather
/// than once per arrival. Both halves are asserted, because they fail under
/// different wrong implementations: a sliding deadline postpones the first
/// request until the arrivals stop, and an unguarded re-arm keeps the first
/// request on time while multiplying admission windows and planner calls by the
/// arrival count.
///
/// Deliberate breaks this must go red under:
///
/// * drop `if slot.armed { continue; }` from `Engine::reconcile` — planner calls
///   rise from one per window to one per arrival;
/// * make the window slide (cancel the pending admission task and re-arm on
///   every arrival) — no request reaches the wire while demand keeps arriving.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn continuous_arrival_never_postpones_the_admission_window() {
    const ARRIVALS: usize = 40;
    const INTERVAL: Duration = Duration::from_millis(4);
    /// Fifteen windows of arrivals; the first request must not wait for them.
    const SPAN: Duration = Duration::from_millis(160);
    /// One window plus scheduling slack, far short of the arrival span.
    const PROMPT: Duration = Duration::from_millis(60);

    let shared = relay("steady");
    let key = session_key(&shared);
    let assembly = assemble();

    let started = tokio::time::Instant::now();
    let mut held = Vec::with_capacity(ARRIVALS);
    let mut first_request_at = None;
    for _ in 0..ARRIVALS {
        held.push(
            assembly
                .observer
                .open(metadata_of([Keys::generate().public_key()], &shared))
                .expect("the live query opens"),
        );
        tokio::time::sleep(INTERVAL).await;
        if first_request_at.is_none() && !requests(assembly.peer(&shared)).is_empty() {
            first_request_at = Some(started.elapsed());
        }
    }
    assert!(
        started.elapsed() >= SPAN,
        "the arrival stream must outlast many windows"
    );

    let first = first_request_at
        .expect("demand must reach the wire while it is still arriving, not after it stops");
    assert!(
        first <= PROMPT,
        "the first cohort waited {first:?} for a fixed {ADMISSION_WINDOW:?} window"
    );

    wait_until(|| requests(assembly.peer(&shared)).len() >= ARRIVALS).await;
    settle().await;
    let sent = requests(assembly.peer(&shared));
    let identities: BTreeSet<_> = sent.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        identities.len(),
        ARRIVALS,
        "each demand reaches the wire exactly once"
    );

    // One planner call per window, not one per arrival: with the guard in place
    // this run makes 15, and an unguarded re-arm makes one per arrival.
    let windows = SPAN.as_millis() / ADMISSION_WINDOW.as_millis();
    let bound = usize::try_from(windows).expect("the window count is small") + 4;
    let calls = assembly
        .planner
        .inputs()
        .into_iter()
        .filter(|(relay, _)| relay == &key)
        .count();
    assert!(
        calls <= bound,
        "a fixed window batches: {ARRIVALS} arrivals produced {calls} planner calls, bound {bound}"
    );
    drop(held);
}

fn metadata_of(authors: impl IntoIterator<Item = PublicKey>, relay: &RelayUrl) -> Query {
    Query::events()
        .kinds([Kind::Metadata])
        .expect("one kind is bounded")
        .authors(authors)
        .expect("test authors are bounded")
        .only_from_relays([relay.clone()])
        .expect("explicit relay is valid")
}
