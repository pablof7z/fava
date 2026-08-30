//! Declarative current-account query binding evidence.

use fava_query::{Query, SingleLetterTag};
use nostr::key::Keys;

#[test]
fn author_dependency_binds_exactly_and_empty_matches_nothing() {
    let alice = Keys::generate().public_key();
    let reactive = Query::events().authors_current_account();

    assert!(reactive.depends_on_current_account());
    assert!(
        reactive.matches_nothing(),
        "an unbound dependency cannot widen"
    );

    let bound = reactive.clone().bind_current_account(Some(alice));
    assert!(!bound.depends_on_current_account());
    assert_eq!(
        bound.selection().authors.as_ref().expect("author axis"),
        &[alice].into_iter().collect()
    );
    assert!(!bound.matches_nothing());

    let empty = reactive.bind_current_account(None);
    assert_eq!(
        empty
            .selection()
            .authors
            .as_ref()
            .expect("author axis")
            .len(),
        0
    );
    assert!(empty.matches_nothing());
}

#[test]
fn tag_dependency_binds_canonical_pubkey_without_widening_literals() {
    let alice = Keys::generate().public_key();
    let p = SingleLetterTag::from_char('p').expect("tag key");
    let reactive = Query::events().tag_value_current_account(p);

    let bound = reactive.clone().bind_current_account(Some(alice));
    assert_eq!(
        bound.selection().tag_values.get(&p).expect("p axis"),
        &[alice.to_hex()].into_iter().collect()
    );

    let empty = reactive.bind_current_account(None);
    assert!(
        empty
            .selection()
            .tag_values
            .get(&p)
            .expect("p axis")
            .is_empty()
    );
    assert!(empty.matches_nothing());
}

#[test]
fn current_account_dependency_intersects_existing_literal_constraints() {
    let alice = Keys::generate().public_key();
    let bob = Keys::generate().public_key();
    let query = Query::events()
        .authors([alice, bob])
        .expect("literal authors")
        .authors_current_account();

    let alice_bound = query.clone().bind_current_account(Some(alice));
    assert_eq!(
        alice_bound.selection().authors.as_ref().expect("authors"),
        &[alice].into_iter().collect()
    );
    assert!(query.bind_current_account(None).matches_nothing());
}
