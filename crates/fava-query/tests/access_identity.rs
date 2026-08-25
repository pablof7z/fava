//! Exact relay access is part of public query identity.

use fava_query::Query;
use fava_relay::RelayAccess;
use nostr::key::Keys;
use std::collections::HashSet;

#[test]
fn same_selection_with_different_access_is_two_query_keys() {
    let authenticated = RelayAccess::Authenticated(Keys::generate().public_key());
    let public = Query::events().with_relay_access(RelayAccess::Public);
    let private = Query::events().with_relay_access(authenticated.clone());
    assert_ne!(public, private);
    assert_eq!(public.access(), &RelayAccess::Public);
    assert_eq!(private.access(), &authenticated);
    assert_eq!(HashSet::from([public, private]).len(), 2);
}
