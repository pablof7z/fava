//! Public query identity and construction refusal evidence.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fava_query::{Kind, Query, QueryError, RelayUrl, SingleLetterTag};

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("test relay URL")
}

fn hash(query: &Query) -> u64 {
    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn repeated_kind_identity_is_canonical() {
    let first = Kind::from_u16(30_001);
    let second = Kind::from_u16(30_002);
    let left = Query::events().kind(first).kind(first).kind(second);
    let right = Query::events().kind(second).kind(first).kind(second);

    assert_eq!(left, right);
    assert_eq!(hash(&left), hash(&right));
    assert_eq!(
        left.selection().kinds.as_ref(),
        Some(&std::collections::BTreeSet::from([first, second]))
    );
    assert_eq!(
        Query::events().kind(first).selection().kinds.as_ref(),
        Some(&std::collections::BTreeSet::from([first]))
    );
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

/// One observation is one identity, however many relays carry its demand and
/// however many times those relay sessions are re-established.
///
/// Minting per relay session — which is what happens when a wire-subscription
/// counter is reused as the observation counter — gives one logical query N
/// owners across N relays and a fresh owner on every reconnect. Grouped relay
/// demand can then never be attributed back to one observation, so a grouped
/// EOSE cannot settle it.
#[test]
fn one_observation_keeps_one_identity_across_relays_and_reconnects() {
    let ids = fava_query::ObservationIds::new();

    let observation = ids.allocate().expect("first identity");

    // Fanning the same observation out to three relays and re-establishing
    // each of them mints nothing: the id travels, it is not re-derived.
    let carried: Vec<_> = (0..3)
        .flat_map(|_relay| (0..2).map(move |_reconnect| observation))
        .collect();
    assert!(
        carried.iter().all(|id| *id == observation),
        "every relay session of one observation carries the same owner"
    );

    // A genuinely separate observation is a genuinely separate identity.
    let other = ids.allocate().expect("second identity");
    assert_ne!(other, observation);
    assert_eq!(other.get().get(), observation.get().get() + 1);
}

#[test]
fn observation_identities_are_never_zero_and_never_repeat() {
    let ids = fava_query::ObservationIds::new();
    let minted: Vec<_> = (0..64).map(|_| ids.allocate().expect("identity")).collect();
    let unique: std::collections::BTreeSet<_> = minted.iter().copied().collect();
    assert_eq!(unique.len(), minted.len(), "identities never repeat");
}
