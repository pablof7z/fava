use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fava_state::RelayUrl;
use fava_write::{Event, EventBuilder, Kind, PublicKey, Tag, Timestamp, UnsignedEvent};

use crate::{Group, GroupError};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn author() -> PublicKey {
    PublicKey::from_hex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        .expect("generator public key")
}

fn tag(values: &[&str]) -> Tag {
    Tag::parse(values.iter().copied()).expect("test tag")
}

fn unsigned(tags: Vec<Tag>) -> UnsignedEvent {
    EventBuilder::from_parts(
        author(),
        Kind::from_u16(9_007),
        Timestamp::from(7),
        tags,
        "opaque content".to_owned(),
    )
    .build()
    .expect("bounded unsigned event")
}

fn signed(tags: Vec<Tag>) -> Event {
    let mut source = UnsignedEvent::new(
        author(),
        Timestamp::from(11),
        Kind::from_u16(9_008),
        tags,
        "signed opaque content",
    );
    source.ensure_id();
    Event::new(
        source.id.expect("ensured id"),
        source.pubkey,
        source.created_at,
        source.kind,
        source.tags,
        source.content,
        "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
            .parse()
            .expect("shape-valid test signature"),
    )
}

fn contexts(event: &UnsignedEvent) -> Vec<Vec<String>> {
    event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some("h"))
        .map(|tag| tag.as_slice().to_vec())
        .collect()
}

#[test]
fn group_construction_refuses_empty_oversized_and_infinite_hosts() {
    let host = relay("wss://groups.example");

    assert_eq!(
        Group::on(Vec::<RelayUrl>::new(), "photos"),
        Err(GroupError::EmptyHosts)
    );

    let duplicate_bound_plus_one = vec![host.clone(); 257];
    assert_eq!(
        Group::on(duplicate_bound_plus_one, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );

    let distinct_bound_plus_one = (0..257)
        .map(|index| relay(&format!("wss://host-{index}.example")))
        .collect::<Vec<_>>();
    assert_eq!(
        Group::on(distinct_bound_plus_one, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );

    let pulls = Arc::new(AtomicUsize::new(0));
    let observed_pulls = Arc::clone(&pulls);
    let infinite = std::iter::repeat(host.clone()).inspect(move |_| {
        let pull = observed_pulls.fetch_add(1, Ordering::SeqCst) + 1;
        assert!(pull <= 257, "group constructor pulled beyond bound+1");
    });
    assert_eq!(
        Group::on(infinite, "photos"),
        Err(GroupError::TooManyHosts {
            actual: 257,
            maximum: 256,
        })
    );
    assert_eq!(pulls.load(Ordering::SeqCst), 257);

    assert_eq!(Group::on([host.clone()], ""), Err(GroupError::EmptyId));
    assert_eq!(
        Group::on([host.clone()], "x".repeat(4_097)),
        Err(GroupError::GroupIdTooLong {
            bytes: 4_097,
            maximum: 4_096,
        })
    );
    let maximum_id = "x".repeat(4_096);
    assert_eq!(
        Group::on([host], maximum_id.clone())
            .expect("maximum-sized opaque id")
            .id(),
        maximum_id
    );
}

#[test]
fn group_construction_preserves_first_occurrence_order() {
    let first = relay("wss://z.example");
    let second = relay("wss://a.example");
    let third = relay("wss://m.example");
    let group = Group::on(
        [
            first.clone(),
            second.clone(),
            first.clone(),
            third.clone(),
            second.clone(),
        ],
        " photos ",
    )
    .expect("bounded hosts normalize");

    assert_eq!(
        group.hosts().collect::<Vec<_>>(),
        vec![first, second, third]
    );
    assert_eq!(group.id(), " photos ");
}

#[test]
fn group_prepare_unsigned_context_is_lossless() {
    let group = Group::on([relay("wss://groups.example")], "photos").expect("group");
    let first = tag(&["x", "first"]);
    let second = tag(&["p", "subject", "relay-hint"]);
    let context = tag(&["h", "photos"]);

    let absent = unsigned(vec![first.clone(), second.clone()]);
    let prepared_absent = group.prepare(absent).expect("absent context is added");
    assert_eq!(
        prepared_absent.tags.as_slice(),
        &[first.clone(), second.clone(), context.clone()]
    );

    let existing = unsigned(vec![first.clone(), context.clone(), second.clone()]);
    let existing_bytes = existing.as_json();
    let prepared_existing = group
        .prepare(existing)
        .expect("matching context stays exact");
    assert_eq!(prepared_existing.as_json(), existing_bytes);
    assert_eq!(
        group
            .prepare(prepared_existing.clone())
            .expect("repeated preparation is inert")
            .as_json(),
        prepared_existing.as_json()
    );

    let duplicate = unsigned(vec![
        first.clone(),
        context.clone(),
        context,
        second.clone(),
    ]);
    let normalized = group
        .prepare(duplicate)
        .expect("matching duplicates normalize");
    assert_eq!(
        contexts(&normalized),
        vec![vec!["h".to_owned(), "photos".to_owned()]]
    );
    assert_eq!(
        normalized.tags.as_slice(),
        &[first, tag(&["h", "photos"]), second]
    );

    assert_eq!(
        group.prepare(unsigned(vec![tag(&["h"])])),
        Err(GroupError::EmptyGroupContext)
    );
    assert_eq!(
        group.prepare(unsigned(vec![tag(&["h", "elsewhere"])])),
        Err(GroupError::ConflictingGroupContext)
    );
}

#[test]
fn group_prepare_signed_is_byte_exact_or_refuses() {
    let group = Group::on([relay("wss://groups.example")], "photos").expect("group");
    let valid = signed(vec![
        tag(&["x", "first"]),
        tag(&["h", "photos"]),
        tag(&["p", "subject", "relay-hint"]),
    ]);
    let valid_bytes = valid.as_json();
    let prepared = group.prepare(valid).expect("valid signed context");
    assert_eq!(prepared.as_json(), valid_bytes);
    assert_eq!(
        group
            .prepare(prepared.clone())
            .expect("repeated signed preparation is inert")
            .as_json(),
        prepared.as_json()
    );

    assert_eq!(
        group.prepare(signed(vec![tag(&["x", "missing"])])),
        Err(GroupError::MissingGroupContext)
    );
    assert_eq!(
        group.prepare(signed(vec![tag(&["h"])])),
        Err(GroupError::EmptyGroupContext)
    );
    assert_eq!(
        group.prepare(signed(vec![tag(&["h", ""])])),
        Err(GroupError::EmptyGroupContext)
    );
    assert_eq!(
        group.prepare(signed(vec![tag(&["h", "photos"]), tag(&["h", "photos"])])),
        Err(GroupError::DuplicateGroupContext)
    );
    assert_eq!(
        group.prepare(signed(vec![tag(&["h", "elsewhere"])])),
        Err(GroupError::ConflictingGroupContext)
    );
    assert_eq!(
        group.prepare(signed(vec![tag(&["h", "photos", "extra"])])),
        Err(GroupError::ConflictingGroupContext)
    );
    let oversized = "x".repeat(4_097);
    assert_eq!(
        group.prepare(signed(vec![
            Tag::parse(["h", oversized.as_str()]).expect("test tag")
        ])),
        Err(GroupError::GroupContextTooLong {
            bytes: 4_097,
            maximum: 4_096,
        })
    );

    let mut over_bound = vec![tag(&["x", "foreign"]); 2_000];
    over_bound.insert(0, tag(&["h", "photos"]));
    assert_eq!(
        group.prepare(signed(over_bound)),
        Err(GroupError::TooManyContextTags {
            actual: 2_001,
            maximum: 2_000,
        })
    );
}

fn prepare_then_custody(
    group: &Group,
    event: Event,
    custody_calls: &AtomicUsize,
) -> Result<Event, GroupError> {
    let prepared = group.prepare(event)?;
    custody_calls.fetch_add(1, Ordering::SeqCst);
    Ok(prepared)
}

#[test]
fn signed_invalid_context_refuses_before_custody() {
    let group = Group::on([relay("wss://groups.example")], "photos").expect("group");
    let custody_calls = AtomicUsize::new(0);
    let invalid = [
        signed(vec![tag(&["x", "missing"])]),
        signed(vec![tag(&["h"])]),
        signed(vec![tag(&["h", "photos"]), tag(&["h", "photos"])]),
        signed(vec![tag(&["h", "elsewhere"])]),
    ];

    for event in invalid {
        assert!(prepare_then_custody(&group, event, &custody_calls).is_err());
    }
    assert_eq!(custody_calls.load(Ordering::SeqCst), 0);

    let oversized = "x".repeat(4_097);
    assert!(
        prepare_then_custody(
            &group,
            signed(vec![
                Tag::parse(["h", oversized.as_str()]).expect("test tag")
            ]),
            &custody_calls,
        )
        .is_err()
    );
    let mut over_bound = vec![tag(&["x", "foreign"]); 2_000];
    over_bound.insert(0, tag(&["h", "photos"]));
    assert!(prepare_then_custody(&group, signed(over_bound), &custody_calls).is_err());
    assert_eq!(custody_calls.load(Ordering::SeqCst), 0);
}
