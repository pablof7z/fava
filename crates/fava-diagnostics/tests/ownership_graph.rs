//! Falsifiers for the diagnostics ownership graph and its two-dimensional bound.
//!
//! Authority: FROZEN-CONTRACTS §8 rows for §4.

use std::num::{NonZeroU64, NonZeroUsize};

use fava_diagnostics::{
    BoundKind, BoundedText, Diagnostics, LimitDiagnostic, LimitScope, LogicalDemandDiagnostic,
    ObservationId, ObservationWireBinding, OperationGeneration, QueryBranchId, QueryDiagnostic,
    RelayDiagnostic, RelaySessionState, RelaySourceState, WireSubscriptionDiagnostic,
};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl, Timestamp};
use fava_wire::SubscriptionId;

fn session(name: &str) -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL"),
        RelayAccess::public(),
    )
}

fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation"))
}

fn capacity(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("non-zero capacity")
}

fn shared(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("non-zero holder count")
}

/// The snapshot names which relay session serves which observation, from both
/// directions, without private inspection (ARCH:2320, ARCH:2072).
#[test]
fn diagnostics_attribute_each_relay_session_to_its_observation() {
    let diagnostics = Diagnostics::default();
    let shared_relay = session("shared");
    let solo_relay = session("solo");
    let grouped = SubscriptionId::new("grouped");
    let solo = SubscriptionId::new("solo");
    let first = observation(1);
    let second = observation(2);

    diagnostics.relay(RelayDiagnostic {
        session: shared_relay.clone(),
        generation: OperationGeneration(3),
        state: RelaySessionState::Open,
        holders: 2,
        subscriptions: vec![WireSubscriptionDiagnostic {
            id: grouped.clone(),
            serves: vec![first, second],
            stored_events_complete: true,
            closed: None,
        }],
        reconnect_attempts: 0,
    });
    diagnostics.relay(RelayDiagnostic {
        session: solo_relay.clone(),
        generation: OperationGeneration(1),
        state: RelaySessionState::Open,
        holders: 1,
        subscriptions: vec![WireSubscriptionDiagnostic {
            id: solo.clone(),
            serves: vec![second],
            stored_events_complete: false,
            closed: None,
        }],
        reconnect_attempts: 0,
    });
    diagnostics.query(QueryDiagnostic {
        observation: first,
        route_revision: Some(7),
        route_relays: vec![shared_relay.clone()],
        demand: vec![LogicalDemandDiagnostic {
            session: shared_relay.clone(),
            branch: QueryBranchId::ROOT,
            state: RelaySourceState::StoredEventsComplete {
                at: Timestamp::from_secs(100),
            },
        }],
        plan_revision: Some(2),
        wire: vec![ObservationWireBinding {
            session: shared_relay.clone(),
            subscription: grouped.clone(),
            shared_holders: shared(2),
        }],
        shortfalls: Vec::new(),
        pending_operation: None,
        coalesced_updates: 0,
    });
    diagnostics.query(QueryDiagnostic {
        observation: second,
        route_revision: Some(7),
        route_relays: vec![shared_relay.clone(), solo_relay.clone()],
        demand: vec![
            LogicalDemandDiagnostic {
                session: shared_relay.clone(),
                branch: QueryBranchId::ROOT,
                state: RelaySourceState::StoredEventsComplete {
                    at: Timestamp::from_secs(100),
                },
            },
            LogicalDemandDiagnostic {
                session: solo_relay.clone(),
                branch: QueryBranchId(1),
                state: RelaySourceState::Open {
                    requested_at: Timestamp::from_secs(90),
                },
            },
        ],
        plan_revision: Some(2),
        wire: vec![
            ObservationWireBinding {
                session: shared_relay.clone(),
                subscription: grouped.clone(),
                shared_holders: shared(2),
            },
            ObservationWireBinding {
                session: solo_relay.clone(),
                subscription: solo,
                shared_holders: shared(1),
            },
        ],
        shortfalls: Vec::new(),
        pending_operation: None,
        coalesced_updates: 4,
    });

    let snapshot = diagnostics.snapshot();

    // Relay side: one wire subscription, two named owners, one refcount.
    let relay = snapshot
        .relays
        .iter()
        .find(|fact| fact.session == shared_relay)
        .expect("shared relay session is retained");
    assert_eq!(relay.holders, 2);
    assert_eq!(relay.subscriptions[0].serves, vec![first, second]);

    // Observation side: each observation names its own route revision and its
    // own per-relay demand, and the two histories stay distinguishable.
    let owner = snapshot
        .queries
        .iter()
        .find(|fact| fact.observation == first)
        .expect("first observation is retained");
    let peer = snapshot
        .queries
        .iter()
        .find(|fact| fact.observation == second)
        .expect("second observation is retained");
    assert_eq!(owner.demand.len(), 1);
    assert_eq!(peer.demand.len(), 2);
    assert_eq!(owner.route_relays, vec![shared_relay.clone()]);
    assert_eq!(peer.route_relays, vec![shared_relay.clone(), solo_relay]);
    assert_eq!(owner.wire[0].shared_holders, shared(2));
    assert_eq!(owner.coalesced_updates, 0);
    assert_eq!(peer.coalesced_updates, 4);

    // Closing one observation removes only its record.
    diagnostics.forget_query(first);
    let after = diagnostics.snapshot();
    assert_eq!(after.queries.len(), 1);
    assert_eq!(after.queries[0].observation, second);
    assert_eq!(after.dropped_facts.queries, 0);
}

/// Relay-, OS-, and application-supplied text is bounded in bytes before it is
/// retained, and the count bound reports what it discarded (GOALS:1439-1448).
#[test]
fn hostile_relay_text_is_bounded_in_retained_diagnostics() {
    let diagnostics = Diagnostics::bounded(capacity(2));
    let hostile = "\u{1f4a3}".repeat(4096);

    for index in 0..3u64 {
        diagnostics.relay(RelayDiagnostic {
            session: session(&format!("relay{index}")),
            generation: OperationGeneration(index),
            state: RelaySessionState::Reconnecting {
                detail: BoundedText::new(&hostile),
            },
            holders: 1,
            subscriptions: vec![WireSubscriptionDiagnostic {
                id: SubscriptionId::new("sub"),
                serves: vec![observation(index + 1)],
                stored_events_complete: false,
                closed: Some(BoundedText::new(&hostile)),
            }],
            reconnect_attempts: 1,
        });
    }

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.relays.len(), 2);
    assert_eq!(snapshot.dropped_facts.relays, 1);
    for relay in &snapshot.relays {
        let RelaySessionState::Reconnecting { detail } = &relay.state else {
            panic!("expected a reconnecting session");
        };
        assert!(detail.as_str().len() <= BoundedText::MAX_BYTES);
        assert!(detail.truncated_bytes() > 0);
        let closed = relay.subscriptions[0]
            .closed
            .as_ref()
            .expect("CLOSED text is retained");
        assert!(closed.as_str().len() <= BoundedText::MAX_BYTES);
        assert!(closed.truncated_bytes() > 0);
    }

    // Republishing the same session replaces rather than evicting a peer.
    let held = snapshot.relays[1].session.clone();
    diagnostics.relay(RelayDiagnostic {
        session: held.clone(),
        generation: OperationGeneration(9),
        state: RelaySessionState::Closed,
        holders: 0,
        subscriptions: Vec::new(),
        reconnect_attempts: 0,
    });
    let replaced = diagnostics.snapshot();
    assert_eq!(replaced.relays.len(), 2);
    assert_eq!(replaced.dropped_facts.relays, 1);
    let current = replaced
        .relays
        .iter()
        .find(|fact| fact.session == held)
        .expect("republished session is retained once");
    assert_eq!(current.generation, OperationGeneration(9));
    assert_eq!(current.state, RelaySessionState::Closed);
}

/// A limit shortfall is attributable to one scope, and the bound-kind and scope
/// pair is the identity, so a repeat shortfall does not fill retention.
#[test]
fn limit_shortfalls_are_attributed_and_deduplicated_by_scope() {
    let diagnostics = Diagnostics::default();
    let relay = session("busy");

    diagnostics.limit(LimitDiagnostic {
        bound: BoundKind::WireSubscriptions,
        limit: 10,
        required: 12,
        scope: LimitScope::Relay {
            session: relay.clone(),
        },
    });
    diagnostics.limit(LimitDiagnostic {
        bound: BoundKind::WireSubscriptions,
        limit: 10,
        required: 14,
        scope: LimitScope::Relay {
            session: relay.clone(),
        },
    });
    diagnostics.limit(LimitDiagnostic {
        bound: BoundKind::WireSubscriptions,
        limit: 10,
        required: 11,
        scope: LimitScope::Observation {
            observation: observation(5),
        },
    });

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.limits.len(), 2);
    let scoped = snapshot
        .limits
        .iter()
        .find(|fact| {
            fact.scope
                == LimitScope::Relay {
                    session: relay.clone(),
                }
        })
        .expect("relay-scoped shortfall is retained");
    assert_eq!(scoped.required, 14);
    assert_eq!(snapshot.dropped_facts.limits, 0);
}

#[test]
fn diagnostics_are_publishable_from_every_owner() {
    const fn assert_shared<T: Send + Sync>() {}
    assert_shared::<Diagnostics>();
}
