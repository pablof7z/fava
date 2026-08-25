use std::collections::BTreeSet;

use fava_query::QueryError;
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

#[test]
fn saved_group_list_author_input_is_bounded_by_the_query_owner() {
    assert_eq!(
        saved_group_lists(std::iter::repeat(public_key())),
        Err(QueryError::TooManyAuthors {
            actual: 4_097,
            maximum: 4_096,
        })
    );
}
