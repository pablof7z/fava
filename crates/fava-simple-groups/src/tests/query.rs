use std::collections::BTreeSet;

use fava_write::{Kind, PublicKey};

use crate::{SimpleGroupStateEventKind, saved_group_lists};

use super::public_key;

#[test]
fn all_state_event_kinds_are_exact_and_complete() {
    assert_eq!(
        SimpleGroupStateEventKind::ALL
            .into_iter()
            .map(Kind::from)
            .collect::<BTreeSet<_>>(),
        (39_000..=39_005).map(Kind::from_u16).collect()
    );
}

#[test]
fn generic_kind_converts_only_exact_state_event_kinds() {
    let cases = [
        (39_000, SimpleGroupStateEventKind::Metadata),
        (39_001, SimpleGroupStateEventKind::Admins),
        (39_002, SimpleGroupStateEventKind::Members),
        (39_003, SimpleGroupStateEventKind::Roles),
        (39_004, SimpleGroupStateEventKind::LivekitParticipants),
        (39_005, SimpleGroupStateEventKind::Pins),
    ];
    for (number, expected) in cases {
        assert_eq!(
            SimpleGroupStateEventKind::try_from(Kind::from_u16(number)),
            Ok(expected)
        );
    }

    let refusal = Kind::from_u16(39_006);
    assert_eq!(SimpleGroupStateEventKind::try_from(refusal), Err(refusal));
}

#[test]
fn saved_group_lists_has_bounded_input_and_no_result_limit() {
    let author = public_key();
    let query = saved_group_lists([author, author]).expect("bounded authors");
    assert_eq!(
        query.selection().kinds,
        Some(BTreeSet::from([Kind::from_u16(10_009)]))
    );
    assert_eq!(query.selection().authors, Some(BTreeSet::from([author])));
    assert_eq!(query.result_limit(), None);

    let empty = saved_group_lists(Vec::<PublicKey>::new()).expect("empty matches nothing");
    assert_eq!(empty.selection().authors, Some(BTreeSet::new()));
}
