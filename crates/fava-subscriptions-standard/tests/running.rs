//! Evidence that a subscription the relay is already serving is immutable, and
//! that identity is minted rather than derived.

mod support;

use std::collections::BTreeSet;

use fava_subscriptions::{
    EoseCompleteness, InstalledSubscriptions, RelayReadConstraints, ShortfallReason,
    WithdrawalReason,
};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_subscriptions_testkit::{
    PlannerScenario, apply_plan, assert_conformant,
    assert_partial_withdrawal_leaves_the_wire_alone, assert_running_subscriptions_are_immutable,
};
use nostr::event::Kind;
use nostr::filter::{Filter, SingleLetterTag};
use nostr::key::Keys;
use support::{bounded_demand, declared, demand, demand_id, relay, revision};

fn planner() -> StandardSubscriptionPlanner {
    StandardSubscriptionPlanner::new()
}

fn tag_key() -> SingleLetterTag {
    SingleLetterTag::from_char('t').expect("tag key")
}

/// C1: demand that merges with a running subscription must **not** rewrite it.
///
/// Under the replanning model this was one `close` and one `open`: the merged
/// filter has different bytes, so the completed subscription is torn down and
/// the relay re-serves its whole stored window for demand that was already
/// settled. The correct answer is a second request alongside it.
#[test]
fn new_demand_never_reopens_an_installed_subscription() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let held = demand(1, Filter::new().author(alice));
    let first = PlannerScenario::fresh("running author query", relay(), vec![held.clone()]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );
    let live: Vec<_> = installed.ids().cloned().collect();
    assert_eq!(live.len(), 1);

    let arriving = demand(2, Filter::new().author(bob));
    let second = first
        .clone()
        .demanding(vec![held, arriving])
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(
        plan.close.is_empty(),
        "a running subscription is never withdrawn for a newcomer"
    );
    assert_eq!(plan.retain, live);
    assert_eq!(plan.open.len(), 1, "the newcomer gets its own request");
    assert_eq!(
        plan.open[0].serves,
        [demand_id(2)].into_iter().collect::<BTreeSet<_>>()
    );
    // Over-fetch is free; under-fetch is silent loss. The newcomer ships its
    // whole filter, never the difference against the incumbent.
    assert_eq!(plan.open[0].filters, vec![Filter::new().author(bob)]);
}

/// The mirror failure: withdrawing one member of a grouped subscription must
/// not narrow it. Narrowing changes the bytes, which costs a full re-serve for
/// the survivor and buys nothing — the surplus is discarded locally.
#[test]
fn withdrawing_one_member_leaves_the_survivors_subscription_untouched() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let both = vec![
        demand(1, Filter::new().author(alice)),
        demand(2, Filter::new().author(bob)),
    ];
    let first = PlannerScenario::fresh("grouped pair", relay(), both.clone());
    let grouped = assert_conformant(&planner(), &first);
    assert_eq!(grouped.open.len(), 1, "the pair shares one request");
    let installed = apply_plan(&InstalledSubscriptions::empty(), &grouped);
    let live: Vec<_> = installed.ids().cloned().collect();

    let second = first
        .clone()
        .demanding(vec![both[0].clone()])
        .continuing(installed.clone(), revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.close.is_empty());
    assert!(plan.open.is_empty());
    assert_eq!(plan.retain, live);
    let id = &live[0];
    assert_eq!(
        plan.attribution.serves(id),
        &[demand_id(1)].into_iter().collect::<BTreeSet<_>>()
    );
    assert_eq!(
        plan.attribution.get(id).expect("attributed").filters,
        installed.get(id).expect("installed").filters,
        "the survivor's request keeps its exact bytes"
    );
}

/// The general property, proved through the conformance kit rather than one
/// hand-built case: nothing an arriving cohort does may disturb an incumbent.
#[test]
fn arriving_demand_never_disturbs_a_running_subscription() {
    let authors: Vec<_> = (0..3).map(|_| Keys::generate().public_key()).collect();
    let held: Vec<_> = authors
        .iter()
        .enumerate()
        .map(|(index, author)| {
            demand(
                u64::try_from(index).expect("index fits") + 1,
                Filter::new().author(*author),
            )
        })
        .collect();
    let scenario = PlannerScenario::fresh("growing cohort", relay(), held);
    let arriving = vec![
        demand(10, Filter::new().author(Keys::generate().public_key())),
        demand(11, Filter::new().kind(Kind::from_u16(7))),
    ];

    let replan = assert_running_subscriptions_are_immutable(&planner(), &scenario, &arriving);

    assert!(!replan.open.is_empty(), "the newcomers reached the wire");
}

/// The withdrawal half of the same property.
#[test]
fn withdrawing_demand_never_disturbs_a_surviving_subscription() {
    let key = tag_key();
    let held: Vec<_> = (1..=6)
        .map(|index| {
            demand(
                index,
                Filter::new().custom_tag(key, format!("topic-{index}")),
            )
        })
        .collect();
    let surviving: Vec<_> = held.iter().take(3).cloned().collect();
    let scenario = PlannerScenario::fresh("shrinking cohort", relay(), held);

    assert_partial_withdrawal_leaves_the_wire_alone(&planner(), &scenario, &surviving);
}

/// A subscription closes when its last owner is gone, and at no other time.
#[test]
fn the_last_owner_leaving_closes_the_subscription() {
    let held = demand(1, Filter::new().search("solitary"));
    let first = PlannerScenario::fresh("single owner", relay(), vec![held]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );

    let second = first
        .clone()
        .demanding(Vec::new())
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert_eq!(plan.close.len(), 1);
    assert_eq!(
        plan.close[0].reason,
        WithdrawalReason::DemandWithdrawn {
            released: [demand_id(1)].into_iter().collect()
        }
    );
}

/// C3: demand a running broader subscription already covers opens nothing. The
/// events are already arriving; the local per-demand re-match keeps the surplus
/// out of the newcomer's results.
#[test]
fn demand_covered_by_a_live_broader_subscription_opens_no_req() {
    let broad = demand(1, Filter::new().kind(Kind::from_u16(1)));
    let first = PlannerScenario::fresh("broad running query", relay(), vec![broad.clone()]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );
    let live: Vec<_> = installed.ids().cloned().collect();

    let narrow = demand(
        2,
        Filter::new()
            .kind(Kind::from_u16(1))
            .author(Keys::generate().public_key()),
    );
    let second = first
        .clone()
        .demanding(vec![broad, narrow])
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.open.is_empty(), "the traffic is already arriving");
    assert!(plan.close.is_empty());
    assert_eq!(plan.retain, live);
    assert_eq!(plan.attribution.serves(&live[0]).len(), 2);
}

/// A result count is not a set axis: a running limited request never absorbs a
/// later owner, because its truncation boundary cannot be reconstructed.
#[test]
fn a_limited_subscription_never_absorbs_a_later_owner() {
    let limited = demand(1, Filter::new().kind(Kind::from_u16(1)).limit(10));
    let first = PlannerScenario::fresh("running limited query", relay(), vec![limited.clone()]);
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );

    let narrow = demand(
        2,
        Filter::new()
            .kind(Kind::from_u16(1))
            .author(Keys::generate().public_key()),
    );
    let second = first
        .clone()
        .demanding(vec![limited, narrow])
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert_eq!(plan.open.len(), 1, "the newcomer needs its own request");
    assert!(plan.close.is_empty());
}

/// C4: reopening demand must not reuse the identity of the request that closed.
///
/// A content digest recycles by construction, so a late EOSE or EVENT for the
/// closed request would settle the new one — which `GOALS:426` (QUERY-010)
/// forbids by name.
#[test]
fn a_reopened_filter_never_reuses_the_closed_subscription_id() {
    let filter = Filter::new().search("comes and goes");
    let first = PlannerScenario::fresh("open", relay(), vec![demand(1, filter.clone())]);
    let opened = assert_conformant(&planner(), &first);
    let original = opened.open[0].id.clone();
    let installed = apply_plan(&InstalledSubscriptions::empty(), &opened);

    let closing = first
        .clone()
        .demanding(Vec::new())
        .continuing(installed.clone(), revision(2));
    let closed = assert_conformant(&planner(), &closing);
    assert_eq!(closed.close.len(), 1);
    let empty = apply_plan(&installed, &closed);
    assert!(empty.is_empty());

    let reopening = first
        .clone()
        .demanding(vec![demand(1, filter)])
        .continuing(empty, revision(3));
    let reopened = assert_conformant(&planner(), &reopening);

    assert_eq!(reopened.open.len(), 1);
    assert_ne!(
        reopened.open[0].id, original,
        "a reopened request must not wear the closed request's identity"
    );
}

/// C10: what the relay advertises must never move an identity that is already
/// live. A NIP-11 refetch that changes the declared id length would otherwise
/// rename every subscription on the session.
#[test]
fn a_changed_declared_id_length_does_not_move_installed_subscription_ids() {
    let held = demand(1, Filter::new().search("stable identity"));
    let first = PlannerScenario::fresh("before the refetch", relay(), vec![held.clone()])
        .declaring(RelayReadConstraints {
            max_subscription_id_chars: declared(64),
            ..RelayReadConstraints::unknown()
        });
    let opened = assert_conformant(&planner(), &first);
    let original = opened.open[0].id.clone();
    let installed = apply_plan(&InstalledSubscriptions::empty(), &opened);

    let second = first
        .clone()
        .declaring(RelayReadConstraints {
            max_subscription_id_chars: declared(32),
            ..RelayReadConstraints::unknown()
        })
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert_eq!(plan.retain, vec![original]);
    assert!(plan.open.is_empty());
    assert!(plan.close.is_empty());
}

/// C5: two demands whose filters are byte-identical share one request even when
/// their whole-query bounds differ. Two identical live REQs would make the relay
/// double-deliver forever and split completion evidence across two identities.
#[test]
fn two_demands_with_identical_filters_and_different_bounds_share_one_subscription() {
    let filter = Filter::new().search("identical bytes");
    let asked = vec![
        bounded_demand(1, filter.clone(), 10),
        bounded_demand(2, filter, 20),
    ];
    let scenario = PlannerScenario::fresh("identical filters, different bounds", relay(), asked);

    let plan = assert_conformant(&planner(), &scenario);

    assert_eq!(plan.open.len(), 1);
    assert_eq!(plan.attribution.serves(&plan.open[0].id).len(), 2);
}

/// C14: the planner is the only component that sees both the filter it sent and
/// what the relay declared, so it records what an EOSE would actually prove.
#[test]
fn a_limited_request_records_that_its_eose_proves_nothing() {
    let asked = vec![demand(1, Filter::new().search("bounded").limit(5))];
    let scenario = PlannerScenario::fresh("limited request", relay(), asked);
    let plan = assert_conformant(&planner(), &scenario);
    assert_eq!(
        plan.attribution
            .get(&plan.open[0].id)
            .expect("attributed")
            .completeness,
        EoseCompleteness::LimitedRequest
    );

    let declared_default = PlannerScenario::fresh(
        "relay default limit",
        relay(),
        vec![demand(1, Filter::new().search("unbounded"))],
    )
    .declaring(RelayReadConstraints {
        default_filter_limit: declared(100),
        ..RelayReadConstraints::unknown()
    });
    let plan = assert_conformant(&planner(), &declared_default);
    assert_eq!(
        plan.attribution
            .get(&plan.open[0].id)
            .expect("attributed")
            .completeness,
        EoseCompleteness::RelayDefaultLimit
    );
}

/// The residual budget is what a plan may spend. A relay that lowers its
/// ceiling below what is already running leaves no residual and closes nothing.
#[test]
fn a_lowered_ceiling_spends_no_residual_and_closes_nothing() {
    let asked: Vec<_> = (1..=3)
        .map(|index| demand(index, Filter::new().search(format!("distinct-{index}"))))
        .collect();
    let first = PlannerScenario::fresh("three running", relay(), asked.clone());
    let installed = apply_plan(
        &InstalledSubscriptions::empty(),
        &assert_conformant(&planner(), &first),
    );
    assert_eq!(installed.len(), 3);

    let mut later = asked;
    later.push(demand(4, Filter::new().search("arriving")));
    let second = first
        .clone()
        .demanding(later)
        .declaring(RelayReadConstraints {
            max_subscriptions: declared(2),
            ..RelayReadConstraints::unknown()
        })
        .continuing(installed, revision(2));
    let plan = assert_conformant(&planner(), &second);

    assert!(plan.close.is_empty(), "a lowered ceiling closes nothing");
    assert!(plan.open.is_empty(), "there is no residual to spend");
    assert_eq!(plan.retain.len(), 3);
    assert_eq!(
        plan.shortfalls[0].reason,
        ShortfallReason::SubscriptionsExhausted {
            required: 4,
            maximum: 2
        }
    );
}
