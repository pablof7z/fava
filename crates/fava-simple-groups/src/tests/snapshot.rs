use fava_query::{EventRecord, QuerySnapshot};
use fava_state::{RelayAccess, RelayEvidence, RelaySessionKey, RelayUrl};
use fava_write::{Event, EventValue, Kind, Tag, Timestamp};
use nostr::event::{EventBuilder, FinalizeEvent};
use nostr::key::Keys;

use crate::{SimpleGroup, SimpleGroupError, SimpleGroupSnapshot};

fn host(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

fn simple_group(names: &[&str]) -> SimpleGroup {
    SimpleGroup::on(names.iter().map(|name| host(name)), "photos").expect("group")
}

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("test tag")
}

fn signed(keys: &Keys, kind: u16, created_at: u64, tags: Vec<Tag>, content: &str) -> Event {
    EventBuilder::new(Kind::from_u16(kind), content)
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .finalize(keys)
        .expect("test event signs")
}

fn evidence(relays: &[RelayUrl]) -> RelayEvidence {
    let mut evidence = RelayEvidence::default();
    for (index, relay) in relays.iter().enumerate() {
        evidence.merge(&RelayEvidence::one(
            RelaySessionKey::new(relay.clone(), RelayAccess::public()),
            Timestamp::from(u64::try_from(index + 1).expect("small index")),
        ));
    }
    evidence
}

fn record(event: Event, relays: &[RelayUrl]) -> EventRecord {
    EventRecord::new(EventValue::Signed(event), evidence(relays), None).expect("stable event id")
}

fn snapshot(events: Vec<EventRecord>) -> QuerySnapshot {
    QuerySnapshot::evaluated(events, &[])
}

trait ProjectionOutcome {
    fn bounded_refusal(self) -> Option<(usize, usize)>;
}

impl ProjectionOutcome for SimpleGroupSnapshot {
    fn bounded_refusal(self) -> Option<(usize, usize)> {
        None
    }
}

impl ProjectionOutcome for Result<SimpleGroupSnapshot, SimpleGroupError> {
    fn bounded_refusal(self) -> Option<(usize, usize)> {
        match self {
            Err(SimpleGroupError::TooManyDiscoveryItems { actual, maximum }) => Some((actual, maximum)),
            Ok(_) | Err(_) => None,
        }
    }
}

#[test]
fn snapshot_projection_refuses_bound_plus_one() {
    let keys = Keys::generate();
    let a = host("a");
    let event = record(
        signed(
            &keys,
            39_000,
            1,
            vec![tag(&["d", "photos"]), tag(&["name", "bounded"])],
            "",
        ),
        std::slice::from_ref(&a),
    );
    let input = snapshot(vec![event; 4_097]);

    assert_eq!(
        simple_group(&["a"]).project(&input).bounded_refusal(),
        Some((4_097, 4_096))
    );
}

#[test]
fn empty_snapshot_is_empty_positive_evidence() {
    let simple_group = simple_group(&["b", "a"]);
    let projected = simple_group
        .project(&snapshot(Vec::new()))
        .expect("empty input is bounded");

    assert!(projected.events().is_empty());
    assert_eq!(
        projected.hosts().cloned().collect::<Vec<_>>(),
        [host("b"), host("a")]
    );
    assert!(projected.metadata().next().is_none());
    assert!(projected.admins().next().is_none());
    assert!(projected.members().next().is_none());
    assert!(projected.at(&host("b")).is_some());
    assert!(projected.at(&host("a")).is_some());
    assert!(!projected.metadata_differ());
    assert!(!projected.admins_differ());
    assert!(!projected.members_differ());
    assert!(!projected.roles_differ());
    assert!(!projected.participants_differ());
    assert!(!projected.pins_differ());
}

#[test]
fn snapshot_projection_is_deterministic() {
    let keys = Keys::generate();
    let a = host("a");
    let b = host("b");
    let first_candidate = signed(&keys, 9, 20, vec![tag(&["h", "photos"])], "shared");
    let second_candidate = signed(&keys, 9, 10, vec![tag(&["h", "photos"])], "unique");
    let (shared, unique) = if first_candidate.id > second_candidate.id {
        (first_candidate, second_candidate)
    } else {
        (second_candidate, first_candidate)
    };
    let input = snapshot(vec![
        record(shared.clone(), std::slice::from_ref(&b)),
        record(shared.clone(), std::slice::from_ref(&a)),
        record(unique.clone(), std::slice::from_ref(&a)),
    ]);
    let simple_group = simple_group(&["b", "a"]);

    let first = simple_group.project(&input).expect("input is bounded");
    let second = simple_group.project(&input).expect("input is bounded");
    assert_eq!(first, second);
    assert_eq!(
        first
            .events()
            .iter()
            .map(EventRecord::id)
            .collect::<Vec<_>>(),
        [shared.id, unique.id]
    );
    assert_eq!(
        first.events()[0]
            .relay_evidence
            .observations()
            .map(|observation| observation.session.relay.clone())
            .collect::<Vec<_>>(),
        [a, b]
    );
}

#[test]
fn snapshot_preserves_same_signer_per_host_forks() {
    let keys = Keys::generate();
    let a = host("a");
    let b = host("b");
    let left = signed(
        &keys,
        39_000,
        20,
        vec![
            tag(&["d", "photos"]),
            tag(&["name", "A"]),
            tag(&["about", "left"]),
        ],
        "",
    );
    let right = signed(
        &keys,
        39_000,
        21,
        vec![
            tag(&["d", "photos"]),
            tag(&["name", "B"]),
            tag(&["about", "right"]),
        ],
        "",
    );
    let projected = simple_group(&["a", "b"])
        .project(&snapshot(vec![
            record(left, std::slice::from_ref(&a)),
            record(right, std::slice::from_ref(&b)),
        ]))
        .expect("input is bounded");

    let values = projected
        .metadata()
        .map(|(host, metadata)| {
            (
                host.clone(),
                metadata.name().map(str::to_owned),
                metadata.about().map(str::to_owned),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        [
            (a.clone(), Some("A".to_owned()), Some("left".to_owned())),
            (b.clone(), Some("B".to_owned()), Some("right".to_owned())),
        ]
    );
    assert_eq!(
        projected
            .at(&a)
            .expect("configured A")
            .metadata()
            .next()
            .and_then(|(_, metadata)| metadata.name()),
        Some("A")
    );
    assert_eq!(
        projected
            .at(&b)
            .expect("configured B")
            .metadata()
            .next()
            .and_then(|(_, metadata)| metadata.name()),
        Some("B")
    );
    assert!(projected.metadata_differ());
}

#[test]
fn snapshot_attribution_uses_actual_relay_evidence() {
    let keys = Keys::generate();
    let listed = Keys::generate().public_key();
    let listed_hex = listed.to_hex();
    let a = host("a");
    let admins = signed(
        &keys,
        39_001,
        30,
        vec![tag(&["d", "photos"]), tag(&["p", &listed_hex, "moderator"])],
        "",
    );
    let members = signed(
        &keys,
        39_002,
        31,
        vec![tag(&["d", "photos"]), tag(&["p", &listed_hex])],
        "",
    );
    let projected = simple_group(&["a", "b", "contacted-but-not-serving"])
        .project(&snapshot(vec![
            record(admins, std::slice::from_ref(&a)),
            record(members, std::slice::from_ref(&a)),
        ]))
        .expect("input is bounded");

    assert_eq!(
        projected
            .admins()
            .map(|(host, (key, roles))| (host.clone(), *key, roles.clone()))
            .collect::<Vec<_>>(),
        [(a.clone(), listed, vec!["moderator".to_owned()])]
    );
    assert_eq!(
        projected
            .members()
            .map(|(host, key)| (host.clone(), *key))
            .collect::<Vec<_>>(),
        [(a, listed)]
    );
    for unobserved in [host("b"), host("contacted-but-not-serving")] {
        let view = projected
            .at(&unobserved)
            .expect("configured host remains inspectable");
        assert!(view.admins().next().is_none());
        assert!(view.members().next().is_none());
    }
}

#[test]
#[allow(clippy::too_many_lines)] // One six-family matrix keeps whole-value disagreement causally aligned.
fn snapshot_disagreement_compares_complete_values() {
    let keys = Keys::generate();
    let first = Keys::generate().public_key();
    let second = Keys::generate().public_key();
    let first_hex = first.to_hex();
    let second_hex = second.to_hex();
    let a = host("a");
    let b = host("b");
    let pin_one = "11".repeat(32);
    let pin_two = "22".repeat(32);
    let families = [
        (
            signed(
                &keys,
                39_000,
                10,
                vec![
                    tag(&["d", "photos"]),
                    tag(&["name", "same"]),
                    tag(&["about", "A"]),
                ],
                "",
            ),
            signed(
                &keys,
                39_000,
                11,
                vec![
                    tag(&["d", "photos"]),
                    tag(&["name", "same"]),
                    tag(&["about", "B"]),
                ],
                "",
            ),
        ),
        (
            signed(
                &keys,
                39_001,
                10,
                vec![tag(&["d", "photos"]), tag(&["p", &first_hex, "same", "A"])],
                "",
            ),
            signed(
                &keys,
                39_001,
                11,
                vec![tag(&["d", "photos"]), tag(&["p", &first_hex, "same", "B"])],
                "",
            ),
        ),
        (
            signed(
                &keys,
                39_002,
                10,
                vec![tag(&["d", "photos"]), tag(&["p", &first_hex])],
                "",
            ),
            signed(
                &keys,
                39_002,
                11,
                vec![
                    tag(&["d", "photos"]),
                    tag(&["p", &first_hex]),
                    tag(&["p", &second_hex]),
                ],
                "",
            ),
        ),
        (
            signed(
                &keys,
                39_003,
                10,
                vec![tag(&["d", "photos"]), tag(&["role", "same", "A"])],
                "",
            ),
            signed(
                &keys,
                39_003,
                11,
                vec![tag(&["d", "photos"]), tag(&["role", "same", "B"])],
                "",
            ),
        ),
        (
            signed(
                &keys,
                39_004,
                10,
                vec![tag(&["d", "photos"]), tag(&["participant", &first_hex])],
                "",
            ),
            signed(
                &keys,
                39_004,
                11,
                vec![
                    tag(&["d", "photos"]),
                    tag(&["participant", &first_hex]),
                    tag(&["participant", &second_hex]),
                ],
                "",
            ),
        ),
        (
            signed(
                &keys,
                39_005,
                10,
                vec![tag(&["d", "photos"]), tag(&["e", &pin_one])],
                "",
            ),
            signed(
                &keys,
                39_005,
                11,
                vec![
                    tag(&["d", "photos"]),
                    tag(&["e", &pin_one]),
                    tag(&["e", &pin_two]),
                ],
                "",
            ),
        ),
    ];
    let records = families
        .into_iter()
        .flat_map(|(left, right)| {
            [
                record(left, std::slice::from_ref(&a)),
                record(right, std::slice::from_ref(&b)),
            ]
        })
        .collect();
    let projected = simple_group(&["b", "a"])
        .project(&snapshot(records))
        .expect("input is bounded");

    assert!(projected.metadata_differ());
    assert!(projected.admins_differ());
    assert!(projected.members_differ());
    assert!(projected.roles_differ());
    assert!(projected.participants_differ());
    assert!(projected.pins_differ());
    assert_eq!(projected.hosts().cloned().collect::<Vec<_>>(), [b, a]);
    assert_eq!(projected.admin_records().count(), 2);
    assert_eq!(projected.member_records().count(), 2);
    assert_eq!(projected.role_records().count(), 2);
    assert_eq!(projected.participant_records().count(), 2);
    assert_eq!(projected.pin_records().count(), 2);
}
