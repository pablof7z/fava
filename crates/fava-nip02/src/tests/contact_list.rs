use fava_write::{EventBuilder as FavaEventBuilder, EventValue, Kind, Timestamp};
use nostr::key::Keys;

use crate::{ContactList, ContactListError, ContactListRowEvidence};

use super::{source, tag};

#[test]
fn contact_list_fixtures_are_finalized() {
    let author = Keys::generate();
    let target = Keys::generate().public_key();
    let event = source(
        &author,
        Kind::ContactList,
        7,
        "legacy",
        vec![tag(&[
            "p",
            &target.to_hex(),
            "wss://relay.example",
            "alice",
        ])],
    );

    event
        .verify()
        .expect("fixture has a valid id and signature");
    assert!(EventValue::Signed(event).id().is_some());
}

#[test]
fn valid_empty_and_ordered_contact_lists_decode() {
    let author = Keys::generate();
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let empty = source(
        &author,
        Kind::ContactList,
        1,
        "legacy",
        vec![tag(&["t", "nostr"]), tag(&["something-something"])],
    );
    let empty = ContactList::from_event(&EventValue::Signed(empty)).expect("valid empty list");
    assert_eq!(empty.author(), author.public_key());
    assert!(empty.follows().is_empty());
    assert!(empty.evidence().is_empty());

    let event = source(
        &author,
        Kind::ContactList,
        2,
        "legacy",
        vec![
            tag(&["x", "before"]),
            tag(&["p", &alice.to_hex()]),
            tag(&["p", &bob.to_hex(), "wss://relay.example", "bob"]),
        ],
    );
    let list = ContactList::from_event(&EventValue::Signed(event)).expect("ordered list");
    assert_eq!(
        list.follows()
            .iter()
            .map(|follow| (follow.source_index(), follow.pubkey()))
            .collect::<Vec<_>>(),
        vec![(1, alice), (2, bob)]
    );
    assert_eq!(
        list.follows()[1].relay().map(ToString::to_string),
        Some("wss://relay.example".to_owned())
    );
    assert_eq!(list.follows()[1].petname(), Some("bob"));

    let older = source(&author, Kind::ContactList, 3, "", Vec::new());
    let newer = source(&author, Kind::ContactList, 4, "", Vec::new());
    let older = ContactList::from_event(&EventValue::Signed(older)).expect("older");
    let newer = ContactList::from_event(&EventValue::Signed(newer)).expect("newer");
    assert_eq!(newer.author(), older.author());
}

#[test]
fn nip02_accounts_for_every_p_row() {
    let author = Keys::generate();
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let rows = vec![
        tag(&["x", "not-a-contact"]),
        tag(&["p"]),
        tag(&["p", "not-a-public-key"]),
        tag(&["p", &alice.to_hex(), "https://not-a-relay.example"]),
        tag(&["p", &alice.to_hex(), "", "ali"]),
        tag(&["p", &alice.to_hex()]),
        tag(&["p", &bob.to_hex(), "", "bob", "future-column"]),
        tag(&["p", &bob.to_hex()]),
    ];
    let event = source(&author, Kind::ContactList, 7, "legacy", rows.clone());
    let list = ContactList::from_event(&EventValue::Signed(event)).expect("mixed list");

    assert_eq!(
        list.follows()
            .iter()
            .map(crate::Follow::source_index)
            .collect::<Vec<_>>(),
        vec![4, 7]
    );
    assert_eq!(
        list.evidence()
            .iter()
            .map(ContactListRowEvidence::source_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 5, 6]
    );
    let mut accounted = list
        .follows()
        .iter()
        .map(crate::Follow::source_index)
        .chain(
            list.evidence()
                .iter()
                .map(ContactListRowEvidence::source_index),
        )
        .collect::<Vec<_>>();
    accounted.sort_unstable();
    assert_eq!(accounted, vec![1, 2, 3, 4, 5, 6, 7]);

    assert!(matches!(
        &list.evidence()[0],
        ContactListRowEvidence::MissingTarget { .. }
    ));
    assert!(matches!(
        &list.evidence()[1],
        ContactListRowEvidence::InvalidPublicKey { .. }
    ));
    assert!(matches!(
        &list.evidence()[2],
        ContactListRowEvidence::InvalidRelayHint { .. }
    ));
    assert!(matches!(
        &list.evidence()[3],
        ContactListRowEvidence::DuplicateTarget { pubkey, .. } if *pubkey == alice
    ));
    assert!(matches!(
        &list.evidence()[4],
        ContactListRowEvidence::UninterpretedExtraColumns { .. }
    ));
    for (evidence, source_index) in list.evidence().iter().zip([1, 2, 3, 5, 6]) {
        assert_eq!(evidence.raw_row(), rows[source_index].as_slice());
    }
}

#[test]
fn invalid_contact_rows_do_not_reserve_duplicate_targets() {
    let author = Keys::generate();
    let target = Keys::generate().public_key();
    let rows = vec![
        tag(&["p", &target.to_hex(), "https://invalid.example"]),
        tag(&["p", &target.to_hex(), "wss://relay.example", "alice"]),
        tag(&["p", &target.to_hex()]),
    ];
    let event = source(&author, Kind::ContactList, 9, "", rows.clone());
    let list = ContactList::from_event(&EventValue::Signed(event)).expect("mixed list");

    assert_eq!(list.follows().len(), 1);
    assert_eq!(list.follows()[0].source_index(), 1);
    assert_eq!(list.follows()[0].pubkey(), target);
    assert_eq!(list.evidence().len(), 2);
    assert!(matches!(
        &list.evidence()[0],
        ContactListRowEvidence::InvalidRelayHint {
            source_index: 0,
            ..
        }
    ));
    assert!(matches!(
        &list.evidence()[1],
        ContactListRowEvidence::DuplicateTarget {
            source_index: 2,
            pubkey,
            ..
        } if *pubkey == target
    ));
    assert_eq!(list.evidence()[0].raw_row(), rows[0].as_slice());
    assert_eq!(list.evidence()[1].raw_row(), rows[2].as_slice());
}

#[test]
fn petname_presence_and_utf8_remain_exact() {
    let author = Keys::generate();
    let targets = (0..4)
        .map(|_| Keys::generate().public_key())
        .collect::<Vec<_>>();
    let decomposed = "A\u{0301}";
    let event = source(
        &author,
        Kind::ContactList,
        11,
        "",
        vec![
            tag(&["p", &targets[0].to_hex()]),
            tag(&["p", &targets[1].to_hex(), ""]),
            tag(&["p", &targets[2].to_hex(), "", ""]),
            tag(&["p", &targets[3].to_hex(), "", decomposed]),
        ],
    );
    let list = ContactList::from_event(&EventValue::Signed(event)).expect("valid list");

    assert_eq!(list.follows()[0].petname(), None);
    assert_eq!(list.follows()[1].petname(), None);
    assert_eq!(list.follows()[2].petname(), Some(""));
    assert_eq!(list.follows()[3].petname(), Some(decomposed));
    assert!(list.follows().iter().all(|follow| follow.relay().is_none()));
    assert_eq!(
        list.follows()[3].petname().unwrap().as_bytes(),
        decomposed.as_bytes()
    );
}

#[test]
fn invalid_contact_list_events_are_refused_before_rows() {
    let author = Keys::generate();
    let wrong_kind = source(&author, Kind::Metadata, 1, "", Vec::new());
    assert!(matches!(
        ContactList::from_event(&EventValue::Signed(wrong_kind)),
        Err(ContactListError::WrongKind(0))
    ));

    let mut missing_id = FavaEventBuilder::new(Kind::ContactList)
        .created_at(Timestamp::from(2))
        .by(author.public_key())
        .build()
        .expect("bounded unsigned fixture");
    missing_id.id = None;
    assert!(matches!(
        ContactList::from_event(&EventValue::Unsigned(missing_id)),
        Err(ContactListError::MissingEventId)
    ));

    let mut tampered = source(&author, Kind::ContactList, 3, "original", Vec::new());
    tampered.content = "tampered".to_owned();
    assert!(matches!(
        ContactList::from_event(&EventValue::Signed(tampered)),
        Err(ContactListError::InvalidEvent(_))
    ));

    let too_large = source(
        &author,
        Kind::ContactList,
        5,
        &"x".repeat(140_000),
        Vec::new(),
    );
    assert!(matches!(
        ContactList::from_event(&EventValue::Signed(too_large)),
        Err(ContactListError::TooLarge { .. })
    ));
}

#[test]
fn signed_contact_list_input_is_not_subject_to_the_local_write_builder_tag_bound() {
    let author = Keys::generate();
    let signed = source(
        &author,
        Kind::ContactList,
        4,
        "",
        (0..2_001)
            .map(|index| tag(&["x", &index.to_string()]))
            .collect(),
    );

    let decoded = ContactList::from_event(&EventValue::Signed(signed))
        .expect("valid signed input is decoded under its owning byte bound");
    assert!(decoded.follows().is_empty());
    assert!(decoded.evidence().is_empty());
}

/// A duplicate relay in the publication route is a routing defect. Reporting
/// it as a malformed contact list sends the caller to fix the wrong thing.
#[test]
fn a_repeated_publication_relay_is_reported_as_a_route_defect() {
    let relay = nostr::types::RelayUrl::parse("wss://relay.example").expect("relay url");

    let mapped = crate::contact_list::map_write_error(
        fava_write::WriteIntentError::DuplicateExplicitRelay {
            relay: relay.clone(),
        },
    );

    assert_eq!(mapped, ContactListError::DuplicateRelay { relay });
    assert!(
        !matches!(mapped, ContactListError::InvalidEvent(_)),
        "a bad route is never a malformed event"
    );
}

#[test]
fn an_empty_or_oversized_publication_route_is_reported_as_a_route_defect() {
    let mapped =
        crate::contact_list::map_write_error(fava_write::WriteIntentError::EmptyExplicitRelays);
    assert!(matches!(mapped, ContactListError::InvalidRoute(_)));

    let mapped =
        crate::contact_list::map_write_error(fava_write::WriteIntentError::TooManyExplicitRelays {
            actual: 100,
            maximum: 32,
        });
    assert!(matches!(mapped, ContactListError::InvalidRoute(_)));
}
