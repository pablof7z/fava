//! Public query identity and construction refusal evidence.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fava_query::{Query, QueryError, RelayUrl};

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
