use std::collections::BTreeSet;
use std::sync::Arc;

use fava_query::{
    EventRecord, QueryEvaluator, QuerySnapshot, SingleLetterTag, SourceEvent, SourceKind,
    SourceRevision, SourceSnapshot, SourceStatus,
};
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{CachedEvent, RelayEvidence};
use fava_write::{EventValue, Kind};
use nostr::key::Keys;

use crate::{IntoContactAuthors, contact_list, followers_of, follows_of};

use super::{source, tag};

fn snapshot(events: Vec<fava_write::Event>) -> QuerySnapshot {
    QuerySnapshot::evaluated(
        events
            .into_iter()
            .map(|event| {
                EventRecord::new(EventValue::Signed(event), RelayEvidence::default(), None)
                    .expect("finalized event record")
            })
            .collect(),
        &[],
    )
}

fn accepts_sealed_trait(
    authors: impl IntoContactAuthors,
) -> Result<fava_query::Query, fava_query::QueryError> {
    contact_list(authors)
}

#[test]
fn one_many_and_empty_contact_list_queries_keep_a_concrete_author_axis() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();

    let one = contact_list(alice).expect("one author is bounded");
    assert_eq!(
        one.selection().kinds,
        Some(BTreeSet::from([Kind::ContactList]))
    );
    assert_eq!(one.selection().authors, Some(BTreeSet::from([alice])));
    assert!(one.selection().tag_values.is_empty());
    assert_eq!(one.result_limit(), None);

    let authors = vec![alice, bob, alice];
    let many = contact_list(authors.as_slice()).expect("three author inputs are bounded");
    assert_eq!(many.selection().authors, Some(BTreeSet::from([alice, bob])));
    assert_eq!(many.result_limit(), None);

    let empty: &[fava_write::PublicKey] = &[];
    let none = contact_list(empty).expect("empty author input is bounded");
    assert_eq!(none.selection().authors, Some(BTreeSet::new()));
    assert_ne!(
        none,
        fava_query::Query::events()
            .kinds([Kind::ContactList])
            .expect("one kind is bounded")
    );

    assert_eq!(accepts_sealed_trait(&authors), Ok(many));
}

#[test]
fn ordinary_evaluation_keeps_each_authors_newest_contact_list() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let alice_old = source(&alice, Kind::ContactList, 2, "old", Vec::new());
    let alice_new = source(&alice, Kind::ContactList, 9, "new", Vec::new());
    let bob_only = source(&bob, Kind::ContactList, 5, "bob", Vec::new());
    let sources = [SourceSnapshot {
        kind: SourceKind::EventCache,
        revision: SourceRevision(1),
        status: SourceStatus::Open,
        retractions: Vec::new(),
        events: vec![alice_old, bob_only.clone(), alice_new.clone()]
            .into_iter()
            .map(|event| SourceEvent::Cached(CachedEvent::new(event, RelayEvidence::default())))
            .collect(),
    }];

    let result = StandardQueryEvaluator
        .evaluate(
            &contact_list([alice.public_key(), bob.public_key()]).expect("two authors are bounded"),
            &sources,
        )
        .expect("ordinary evaluation succeeds");

    assert_eq!(result.events.len(), 2);
    assert_eq!(result.events[0].id(), alice_new.id);
    assert_eq!(result.events[1].id(), bob_only.id);
}

#[test]
fn follower_discovery_uses_only_exact_lowercase_p_and_canonical_hex() {
    let subject = Keys::generate().public_key();
    let query = followers_of(subject).expect("one tag value is bounded");
    let lower_p = SingleLetterTag::LOWERCASE_P;
    let upper_p = SingleLetterTag::UPPERCASE_P;

    assert_eq!(
        query.selection().kinds,
        Some(BTreeSet::from([Kind::ContactList]))
    );
    assert_eq!(query.selection().authors, None);
    assert_eq!(
        query.selection().tag_values.get(&lower_p),
        Some(&BTreeSet::from([subject.to_hex()]))
    );
    assert!(!query.selection().tag_values.contains_key(&upper_p));
}

#[test]
fn follow_projection_is_ordered_repeatable_and_safe_for_two_hop_concurrency() {
    let alice = Keys::generate();
    let bob = Keys::generate();
    let carol = Keys::generate();
    let dave = Keys::generate();
    let erin = Keys::generate();
    let first = source(
        &alice,
        Kind::ContactList,
        8,
        "",
        vec![
            tag(&["p", &bob.public_key().to_hex()]),
            tag(&["p", "malformed"]),
            tag(&["p", &carol.public_key().to_hex()]),
        ],
    );
    let second = source(
        &dave,
        Kind::ContactList,
        7,
        "",
        vec![tag(&["p", &erin.public_key().to_hex()])],
    );
    let first_hop_snapshot = Arc::new(snapshot(vec![first, second]));
    let expected = vec![bob.public_key(), carol.public_key(), erin.public_key()];

    assert_eq!(follows_of(first_hop_snapshot.as_ref()), expected);
    assert_eq!(follows_of(first_hop_snapshot.as_ref()), expected);
    assert!(follows_of(&snapshot(Vec::new())).is_empty());

    let first_hop = follows_of(first_hop_snapshot.as_ref());
    assert_eq!(
        contact_list(first_hop.as_slice())
            .expect("projected authors are bounded")
            .selection()
            .authors,
        Some(first_hop.iter().copied().collect())
    );
    let bob_list = source(
        &bob,
        Kind::ContactList,
        10,
        "",
        vec![tag(&["p", &dave.public_key().to_hex()])],
    );
    assert_eq!(
        follows_of(&snapshot(vec![bob_list])),
        vec![dave.public_key()]
    );

    let left_snapshot = Arc::clone(&first_hop_snapshot);
    let right_snapshot = Arc::clone(&first_hop_snapshot);
    let left = std::thread::spawn(move || follows_of(left_snapshot.as_ref()));
    let right = std::thread::spawn(move || follows_of(right_snapshot.as_ref()));
    assert_eq!(left.join().expect("left projection"), expected);
    assert_eq!(right.join().expect("right projection"), expected);
}
