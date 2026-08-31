//! Evidence for the rules that protect a subscription the relay is already
//! serving, and for the containment test that lets new demand attach to one.

mod support;

use std::num::NonZeroUsize;

use fava_subscriptions::{
    AttributedSubscription, DeclaredLimit, EoseCompleteness, InstalledSubscription,
    InstalledSubscriptions, PlanConformanceError, PlannedSubscription, RelayReadConstraints,
    SubscriptionAttribution, SubscriptionPlan, filter_covers, validate_plan,
};
use fava_wire::SubscriptionId;
use nostr::event::{EventId, Kind};
use nostr::filter::{Filter, SingleLetterTag};
use nostr::key::Keys;
use support::{demand, demand_id, relay, revision, wire};

fn unknown() -> RelayReadConstraints {
    RelayReadConstraints::unknown()
}

fn tag(key: char) -> SingleLetterTag {
    SingleLetterTag::from_char(key).expect("ASCII letter tag key")
}

fn running(id: &SubscriptionId, filters: Vec<Filter>, serves: &[u64]) -> InstalledSubscriptions {
    InstalledSubscriptions::from_entries([(
        id.clone(),
        InstalledSubscription {
            filters,
            serves: serves.iter().map(|value| demand_id(*value)).collect(),
        },
    )])
}

fn retaining(id: &SubscriptionId, filters: Vec<Filter>, serves: &[u64]) -> SubscriptionPlan {
    SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: Vec::new(),
        retain: vec![id.clone()],
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([(
            id.clone(),
            AttributedSubscription {
                filters,
                serves: serves.iter().map(|value| demand_id(*value)).collect(),
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    }
}

/// CR-1: a subscription the relay is already serving may not be torn down
/// because other demand arrived. The relay would re-serve a stored window it
/// had already finished.
#[test]
fn a_running_subscription_may_not_be_closed_while_it_is_still_wanted() {
    let alice = Keys::generate().public_key();
    let held = demand(1, Filter::new().author(alice));
    let arriving = demand(2, Filter::new().author(Keys::generate().public_key()));
    let live = wire("live");
    let installed = running(&live, vec![held.filter.clone()], &[1]);

    let merged = Filter::new().authors([alice, Keys::generate().public_key()]);
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: vec![PlannedSubscription {
            filters: vec![merged],
            serves: [demand_id(1), demand_id(2)].into_iter().collect(),
            completeness: EoseCompleteness::Proven,
        }],
        retain: Vec::new(),
        close: vec![live.clone()],
        // Nothing is retained, so nothing carries a wire id to attribute.
        attribution: SubscriptionAttribution::default(),
        shortfalls: Vec::new(),
    };

    assert_eq!(
        validate_plan(&relay(), &[held, arriving], &unknown(), &installed, &plan),
        Err(PlanConformanceError::RunningSubscriptionWithdrawn {
            id: live,
            still_wanted: demand_id(1),
        })
    );
}

/// The mirror rule: one of two owners leaving does not narrow the subscription
/// the other still holds. The surplus is discarded locally, and narrowing costs
/// a full re-serve for nothing.
#[test]
fn a_running_subscription_that_loses_one_of_two_owners_is_retained_unchanged() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let merged = Filter::new().authors([alice, bob]);
    let live = wire("live");
    let installed = running(&live, vec![merged.clone()], &[1, 2]);
    let surviving = demand(2, Filter::new().author(bob));

    let plan = retaining(&live, vec![merged], &[2]);

    validate_plan(&relay(), &[surviving], &unknown(), &installed, &plan)
        .expect("keeping a broader subscription for its surviving owner is conformant");
    assert!(plan.close.is_empty());
}

/// CR-2: two byte-identical requests on one session are strictly worse than
/// one. The relay double-delivers, a slot is burned, and completion evidence
/// splits across two identities so neither is credited.
#[test]
fn opening_a_second_subscription_for_running_filters_is_refused() {
    let filter = Filter::new().kind(Kind::from_u16(1));
    let live = wire("live");
    let installed = running(&live, vec![filter.clone()], &[1]);
    let held = demand(1, filter.clone());
    let joining = demand(2, filter.clone());

    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: vec![PlannedSubscription {
            filters: vec![filter.clone()],
            serves: [demand_id(2)].into_iter().collect(),
            completeness: EoseCompleteness::Proven,
        }],
        retain: vec![live.clone()],
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([(
            live.clone(),
            AttributedSubscription {
                filters: vec![filter.clone()],
                serves: [demand_id(1)].into_iter().collect(),
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    };

    assert_eq!(
        validate_plan(&relay(), &[held, joining], &unknown(), &installed, &plan),
        Err(PlanConformanceError::DuplicateFilters {
            first: live,
            second: 0,
        })
    );
}

/// A relay that lowers its declared ceiling below what is already running does
/// not thereby authorize closing a running subscription. The plan is answerable
/// only for what it opened, so a pure-retain plan over the new ceiling is
/// conformant.
#[test]
fn a_lowered_declared_ceiling_does_not_condemn_what_is_already_running() {
    let first = wire("live-one");
    let second = wire("live-two");
    let alpha = Filter::new().search("alpha");
    let beta = Filter::new().search("beta");
    let installed = InstalledSubscriptions::from_entries([
        (
            first.clone(),
            InstalledSubscription {
                filters: vec![alpha.clone()],
                serves: [demand_id(1)].into_iter().collect(),
            },
        ),
        (
            second.clone(),
            InstalledSubscription {
                filters: vec![beta.clone()],
                serves: [demand_id(2)].into_iter().collect(),
            },
        ),
    ]);
    let constraints = RelayReadConstraints {
        max_subscriptions: DeclaredLimit::Declared(NonZeroUsize::new(1).expect("non-zero")),
        ..RelayReadConstraints::unknown()
    };
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: Vec::new(),
        retain: vec![first.clone(), second.clone()],
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([
            (
                first,
                AttributedSubscription {
                    filters: vec![alpha.clone()],
                    serves: [demand_id(1)].into_iter().collect(),
                    completeness: EoseCompleteness::Proven,
                },
            ),
            (
                second,
                AttributedSubscription {
                    filters: vec![beta.clone()],
                    serves: [demand_id(2)].into_iter().collect(),
                    completeness: EoseCompleteness::Proven,
                },
            ),
        ]),
        shortfalls: Vec::new(),
    };

    validate_plan(
        &relay(),
        &[demand(1, alpha), demand(2, beta)],
        &constraints,
        &installed,
        &plan,
    )
    .expect("a plan that opens nothing cannot overspend a residual budget");
}

/// Opening past the residual budget is still refused: the plan is answerable
/// for what it adds.
#[test]
fn opening_past_the_residual_budget_is_refused() {
    let live = wire("live");
    let alpha = Filter::new().search("alpha");
    let installed = running(&live, vec![alpha.clone()], &[1]);
    let beta = Filter::new().search("beta");
    let constraints = RelayReadConstraints {
        max_subscriptions: DeclaredLimit::Declared(NonZeroUsize::new(1).expect("non-zero")),
        ..RelayReadConstraints::unknown()
    };
    let plan = SubscriptionPlan {
        relay: relay(),
        revision: revision(2),
        open: vec![PlannedSubscription {
            filters: vec![beta.clone()],
            serves: [demand_id(2)].into_iter().collect(),
            completeness: EoseCompleteness::Proven,
        }],
        retain: vec![live.clone()],
        close: Vec::new(),
        attribution: SubscriptionAttribution::from_entries([(
            live,
            AttributedSubscription {
                filters: vec![alpha.clone()],
                serves: [demand_id(1)].into_iter().collect(),
                completeness: EoseCompleteness::Proven,
            },
        )]),
        shortfalls: Vec::new(),
    };

    assert_eq!(
        validate_plan(
            &relay(),
            &[demand(1, alpha), demand(2, beta)],
            &constraints,
            &installed,
            &plan,
        ),
        Err(PlanConformanceError::DeclaredSubscriptionsExceeded {
            installed: 2,
            maximum: 1,
        })
    );
}

/// Containment: an unconstrained wide axis covers a constrained narrow one.
#[test]
fn a_broader_running_filter_covers_a_narrower_demand() {
    let alice = Keys::generate().public_key();
    let wide = Filter::new().kind(Kind::from_u16(1));
    let narrow = Filter::new().kind(Kind::from_u16(1)).author(alice);

    assert!(filter_covers(&wide, &narrow));
    assert!(!filter_covers(&narrow, &wide));
    assert!(filter_covers(&wide, &wide));
}

/// `None` and `Some(empty)` are both *unconstrained* to `match_event`, so a
/// constrained wide axis never covers either.
#[test]
fn a_constrained_axis_never_covers_an_unconstrained_one() {
    let alice = Keys::generate().public_key();
    let constrained = Filter::new().author(alice);
    let absent = Filter::new();
    let empty = Filter {
        authors: Some(std::collections::BTreeSet::new()),
        ..Filter::new()
    };

    assert!(!filter_covers(&constrained, &absent));
    assert!(!filter_covers(&constrained, &empty));
    assert!(filter_covers(&absent, &constrained));
    assert!(filter_covers(&empty, &constrained));
}

/// Tag polarity inverts: an absent name on the wide side is no constraint and
/// covers everything, while a name the narrow side adds only narrows it.
#[test]
fn tag_coverage_reads_an_absent_name_as_unconstrained() {
    let key = tag('t');
    let untagged = Filter::new().kind(Kind::from_u16(1));
    let tagged = Filter::new().kind(Kind::from_u16(1)).custom_tag(key, "x");
    let broad = Filter::new()
        .kind(Kind::from_u16(1))
        .custom_tags(key, ["x", "y"]);

    assert!(filter_covers(&untagged, &tagged));
    assert!(!filter_covers(&tagged, &untagged));
    assert!(filter_covers(&broad, &tagged));
    assert!(!filter_covers(&tagged, &broad));
}

/// A result count is not a set axis in either direction: a limited request
/// covers nothing but itself, and a limited demand attaches to nothing but a
/// byte-identical filter.
#[test]
fn a_limit_on_either_side_confines_coverage_to_byte_identity() {
    let alice = Keys::generate().public_key();
    let limited_wide = Filter::new().kind(Kind::from_u16(1)).limit(10);
    let narrow = Filter::new().kind(Kind::from_u16(1)).author(alice);
    let unlimited_wide = Filter::new().kind(Kind::from_u16(1));
    let limited_narrow = Filter::new()
        .kind(Kind::from_u16(1))
        .author(alice)
        .limit(10);

    assert!(!filter_covers(&limited_wide, &narrow));
    assert!(!filter_covers(&unlimited_wide, &limited_narrow));
    assert!(filter_covers(&limited_wide, &limited_wide));
}

/// A window is a bound, so coverage requires the wide window to contain the
/// narrow one, and a `search` term has no containment at all.
#[test]
fn windows_contain_and_search_terms_do_not() {
    let inner = Filter::new()
        .since(1_000.into())
        .until(2_000.into())
        .kind(Kind::from_u16(1));
    let outer = Filter::new()
        .since(500.into())
        .until(3_000.into())
        .kind(Kind::from_u16(1));
    assert!(filter_covers(&outer, &inner));
    assert!(!filter_covers(&inner, &outer));

    let searching = Filter::new().search("phrase");
    let plain = Filter::new();
    assert!(!filter_covers(&plain, &searching));
    assert!(!filter_covers(&searching, &plain));

    // Event ids are an ordinary containment axis.
    let one = EventId::from_slice(&[1_u8; 32]).expect("32-byte event id");
    assert!(filter_covers(&Filter::new(), &Filter::new().id(one)));
}
