//! Component evidence that scoped query evidence keeps relay facts distinct.
//!
//! Authority: `GOALS:403-418` (QUERY-009), `GOALS:420-428` (QUERY-010),
//! `GOALS:393-401` (QUERY-008).

use std::num::NonZeroU64;

use fava_query::{
    BoundedText, DesiredPlanEvidence, ObservationId, Progress, QueryBranchId, QueryEvidence,
    QueryShortfall, RelayDeadline, RelayQueryEvidence, RelayShortfall, RelaySourceState,
    RelayWithdrawal, RoundIssuer, RouteOrigin, SourceKind,
};
use nostr::types::{RelayUrl, Timestamp};

fn session(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay url parses")
}

fn observation(value: u64) -> ObservationId {
    ObservationId::new(NonZeroU64::new(value).expect("non-zero observation id"))
}

fn relay(session: RelayUrl, state: RelaySourceState) -> RelayQueryEvidence {
    let generation = RoundIssuer::new()
        .expect("generation authority")
        .allocate()
        .expect("generation");
    RelayQueryEvidence {
        session,
        generation: Some(generation),
        plan_revision: 7,
        branches: vec![QueryBranchId::ROOT],
        state,
        shared_with: vec![observation(1)],
        shortfall: None,
        route: RouteOrigin::Explicit,
    }
}

/// An empty result carrying EOSE from one relay must never be confused with an
/// empty result caused by a relay that was never reached.
#[test]
fn empty_with_eose_is_distinguishable_from_unreachable_relay() {
    let answered = session("wss://answered.example");
    let never = session("wss://never.example");

    let evidence = QueryEvidence {
        sources: Vec::new(),
        relays: vec![
            relay(
                answered.clone(),
                RelaySourceState::StoredEventsComplete {
                    at: Timestamp::from(1_700_000_000),
                },
            ),
            relay(
                never.clone(),
                RelaySourceState::Unreachable {
                    attempts: 3,
                    detail: BoundedText::new("connection refused"),
                },
            ),
        ],
        plan: Some(DesiredPlanEvidence {
            revision: 7,
            relays: vec![answered.clone(), never.clone()],
            installed: 1,
        }),
        shortfalls: Vec::new(),
    };

    let answered_evidence = evidence.relay(&answered).expect("answered relay present");
    let never_evidence = evidence.relay(&never).expect("unreached relay present");

    assert!(
        answered_evidence.stored_events_complete(),
        "a relay that sent EOSE must report stored events complete"
    );
    assert!(
        !never_evidence.stored_events_complete(),
        "a relay that never answered must never report stored events complete"
    );
    assert!(
        answered_evidence.is_live(),
        "a relay holding an installed request after EOSE is still live"
    );
    assert!(
        !never_evidence.is_live(),
        "an unreachable relay cannot deliver new events"
    );
    assert_ne!(
        answered_evidence.state, never_evidence.state,
        "empty-with-EOSE and unreachable must not share one state value"
    );
    assert!(
        !evidence.all_relays_stored_events_complete(),
        "one unreached relay must prevent a whole-query completeness claim"
    );
}

/// A relay refusal, an authentication demand, and a withdrawal are three
/// different facts, none of which is a timeout or an unreachable relay.
#[test]
fn every_relay_failure_mode_is_a_distinct_typed_fact() {
    let states = [
        RelaySourceState::Planned,
        RelaySourceState::Connecting,
        RelaySourceState::Open {
            requested_at: Timestamp::from(1),
        },
        RelaySourceState::StoredEventsComplete {
            at: Timestamp::from(2),
        },
        RelaySourceState::Refused {
            message: BoundedText::new("blocked: pubkey not allowed"),
            at: Timestamp::from(3),
        },
        RelaySourceState::AuthenticationRequired {
            progress: Progress::Requested {
                challenge: "nonce-one".to_owned(),
            },
            at: Timestamp::from(4),
        },
        RelaySourceState::TimedOut {
            deadline: RelayDeadline::Idle,
            after_ms: 30_000,
        },
        RelaySourceState::Disconnected {
            detail: BoundedText::new("socket closed"),
        },
        RelaySourceState::Unreachable {
            attempts: 5,
            detail: BoundedText::new("dns failure"),
        },
        RelaySourceState::Withdrawn {
            reason: RelayWithdrawal::RouteWithdrawn,
        },
    ];

    for (left_index, left) in states.iter().enumerate() {
        for (right_index, right) in states.iter().enumerate() {
            if left_index != right_index {
                assert_ne!(
                    left, right,
                    "relay state variants must stay mutually exclusive"
                );
            }
        }
    }

    let complete = states
        .iter()
        .filter(|state| matches!(state, RelaySourceState::StoredEventsComplete { .. }))
        .count();
    assert_eq!(
        complete, 1,
        "exactly one relay state may mean the relay actually sent EOSE"
    );
}

/// One EOSE from a relay serving several branches settles every branch whose
/// demand that relay carries, and does not settle a branch it does not carry.
#[test]
fn grouped_eose_settles_every_logical_demand() {
    let grouped = session("wss://grouped.example");
    let separate = session("wss://separate.example");

    let mut evidence = QueryEvidence {
        sources: Vec::new(),
        relays: vec![
            RelayQueryEvidence {
                branches: vec![QueryBranchId::ROOT, QueryBranchId(1)],
                shared_with: vec![observation(1), observation(2)],
                ..relay(
                    grouped.clone(),
                    RelaySourceState::StoredEventsComplete {
                        at: Timestamp::from(1_700_000_100),
                    },
                )
            },
            RelayQueryEvidence {
                branches: vec![QueryBranchId(2)],
                ..relay(
                    separate.clone(),
                    RelaySourceState::Open {
                        requested_at: Timestamp::from(1_700_000_000),
                    },
                )
            },
        ],
        plan: None,
        shortfalls: Vec::new(),
    };

    let grouped_evidence = evidence.relay(&grouped).expect("grouped relay present");
    assert_eq!(
        grouped_evidence.branches,
        vec![QueryBranchId::ROOT, QueryBranchId(1)],
        "one relay's EOSE must remain attributed to every branch it serves"
    );
    assert!(grouped_evidence.stored_events_complete());
    assert_eq!(
        grouped_evidence.shared_with.len(),
        2,
        "shared wire work must name every observation behind it"
    );
    assert!(
        !evidence.all_relays_stored_events_complete(),
        "a branch served only by a still-open relay is not settled"
    );

    evidence.relays[1].state = RelaySourceState::StoredEventsComplete {
        at: Timestamp::from(1_700_000_200),
    };
    assert!(
        evidence.all_relays_stored_events_complete(),
        "every relay having sent EOSE settles the whole query"
    );
}

/// A query with no relays at all never claims relay completeness.
#[test]
fn a_query_without_relays_never_claims_relay_completeness() {
    let evidence = QueryEvidence::default();
    assert!(!evidence.all_relays_stored_events_complete());
    assert!(evidence.relays.is_empty());
    assert!(evidence.plan.is_none());
}

/// Bounded loss is reported inside the snapshot, not through a side channel.
#[test]
fn coalesced_revisions_are_reported_as_a_typed_shortfall() {
    let evidence = QueryEvidence {
        sources: Vec::new(),
        relays: Vec::new(),
        plan: None,
        shortfalls: vec![
            QueryShortfall::CoalescedUpdates { dropped: 4 },
            QueryShortfall::SourceUnavailable {
                kind: SourceKind::WriteStore,
                detail: BoundedText::new("write store closed"),
            },
        ],
    };

    assert!(evidence.shortfalls.iter().any(|shortfall| matches!(
        shortfall,
        QueryShortfall::CoalescedUpdates { dropped } if *dropped == 4
    )));
    assert!(evidence.shortfalls.iter().any(|shortfall| matches!(
        shortfall,
        QueryShortfall::SourceUnavailable {
            kind: SourceKind::WriteStore,
            ..
        }
    )));
}

/// A relay carrying less demand than the query asked for says so on the
/// snapshot rather than looking complete.
#[test]
fn subscription_shortfall_is_visible_on_the_relay_it_belongs_to() {
    let limited = session("wss://limited.example");
    let evidence = QueryEvidence {
        sources: Vec::new(),
        relays: vec![RelayQueryEvidence {
            branches: vec![QueryBranchId::ROOT],
            shortfall: Some(RelayShortfall {
                branches: vec![QueryBranchId(1), QueryBranchId(2)],
                detail: BoundedText::new("relay subscription limit reached"),
            }),
            route: RouteOrigin::Automatic { revision: 3 },
            ..relay(
                limited.clone(),
                RelaySourceState::StoredEventsComplete {
                    at: Timestamp::from(11),
                },
            )
        }],
        plan: None,
        shortfalls: Vec::new(),
    };

    let entry = evidence.relay(&limited).expect("limited relay present");
    let shortfall = entry.shortfall.as_ref().expect("shortfall retained");
    assert_eq!(shortfall.branches.len(), 2);
    assert!(
        entry.stored_events_complete(),
        "EOSE for the demand that was installed remains true"
    );
    assert!(
        matches!(entry.route, RouteOrigin::Automatic { revision: 3 }),
        "the route that contributed this relay stays attributable"
    );
}

/// Hostile relay text is bounded before it is retained.
#[test]
fn relay_supplied_text_is_bounded() {
    let hostile = "x".repeat(BoundedText::MAX_BYTES * 4);
    let bounded = BoundedText::new(&hostile);
    assert_eq!(bounded.as_str().len(), BoundedText::MAX_BYTES);
    assert_eq!(
        bounded.truncated_bytes(),
        hostile.len() - BoundedText::MAX_BYTES
    );

    let multibyte = "é".repeat(BoundedText::MAX_BYTES);
    let bounded = BoundedText::new(&multibyte);
    assert!(bounded.as_str().len() <= BoundedText::MAX_BYTES);
    assert!(multibyte.starts_with(bounded.as_str()));
    assert_eq!(
        bounded.as_str().len() + bounded.truncated_bytes(),
        multibyte.len()
    );
}

/// A late completion from a superseded plan revision or connection generation
/// is comparable, so an owner can reject it.
#[test]
fn stale_completions_are_comparable_against_current_identity() {
    let mut generations = RoundIssuer::new().expect("generation authority");
    let previous = generations.allocate().expect("previous generation");
    let current = generations.allocate().expect("current generation");
    assert!(previous < current);

    let stale = relay(
        session("wss://stale.example"),
        RelaySourceState::StoredEventsComplete {
            at: Timestamp::from(1),
        },
    );
    assert_eq!(stale.plan_revision, 7);
    assert!(stale.generation.is_some());
}
