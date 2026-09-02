//! Exact relay access is part of public query identity.

use fava_query::Query;
use fava_relay::Authority;
use nostr::key::Keys;
use std::collections::HashSet;

#[test]
fn same_selection_with_different_access_is_two_query_keys() {
    let authenticated = Authority::As(Keys::generate().public_key());
    let public = Query::events().with_relay_access(Authority::Unauthenticated);
    let private = Query::events().with_relay_access(authenticated);
    assert_ne!(public, private);
    assert_eq!(public.access(), &Authority::Unauthenticated);
    assert_eq!(private.access(), &authenticated);
    assert_eq!(HashSet::from([public, private]).len(), 2);
}
