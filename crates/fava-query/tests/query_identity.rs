//! Public query identity and construction refusal evidence.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fava_query::{Query, QueryError, RelayUrl, SingleLetterTag};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn hash(query: &Query) -> u64 {
    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn equivalent_relay_construction_has_one_query_identity() {
    let relay_a = relay("wss://a.example");
    let relay_b = relay("wss://b.example");
    let left = Query::events()
        .from_relays([relay_a.clone(), relay_b.clone()])
        .expect("relay set is non-empty");
    let right = Query::events()
        .from_relays([relay_b, relay_a])
        .expect("relay set is non-empty");

    assert_eq!(left, right);
    assert_eq!(hash(&left), hash(&right));
}

#[test]
fn invalid_query_inputs_are_refused_during_construction() {
    assert_eq!(
        Query::events().from_relays([]),
        Err(QueryError::EmptyExplicitRelays)
    );
    assert_eq!(Query::events().limit(0), Err(QueryError::ZeroLimit));
}

#[test]
fn all_ascii_letter_tag_axes_are_case_sensitive() {
    let mut keys = std::collections::BTreeSet::new();

    for lowercase in 'a'..='z' {
        let uppercase = lowercase.to_ascii_uppercase();
        let lowercase = SingleLetterTag::from_char(lowercase).expect("lowercase ASCII tag key");
        let uppercase = SingleLetterTag::from_char(uppercase).expect("uppercase ASCII tag key");
        let lowercase_query = Query::events().tag_values(lowercase, ["exact"]);
        let uppercase_query = Query::events().tag_values(uppercase, ["exact"]);

        assert_ne!(lowercase_query, uppercase_query);
        assert_eq!(
            lowercase_query.selection().tag_values.get(&lowercase),
            Some(&std::collections::BTreeSet::from(["exact".to_owned()]))
        );
        assert_eq!(
            uppercase_query.selection().tag_values.get(&uppercase),
            Some(&std::collections::BTreeSet::from(["exact".to_owned()]))
        );
        keys.insert(lowercase);
        keys.insert(uppercase);
    }

    assert_eq!(keys.len(), 52);
}

#[test]
fn literal_tag_values_have_canonical_query_identity() {
    let e = SingleLetterTag::from_char('e').expect("tag key");
    let upper_p = SingleLetterTag::from_char('P').expect("tag key");
    let left = Query::events()
        .tag_values(e, ["café", "alpha", "café"])
        .tag_values(upper_p, ["東京"])
        .tag_values(e, ["omega", "alpha"]);
    let right = Query::events()
        .tag_values(e, ["omega"])
        .tag_values(upper_p, ["東京", "東京"])
        .tag_values(e, ["alpha", "café"]);

    assert_eq!(left, right);
    assert_eq!(hash(&left), hash(&right));
    assert_eq!(
        left.selection().tag_values.get(&e),
        Some(&std::collections::BTreeSet::from([
            "alpha".to_owned(),
            "café".to_owned(),
            "omega".to_owned(),
        ]))
    );
}

#[test]
fn absent_and_present_empty_tag_axes_are_distinct() {
    let e = SingleLetterTag::from_char('e').expect("tag key");
    let absent = Query::events();
    let present_empty = Query::events().tag_values(e, std::iter::empty::<String>());

    assert_ne!(absent, present_empty);
    assert_eq!(
        present_empty.selection().tag_values.get(&e),
        Some(&std::collections::BTreeSet::new())
    );
}
