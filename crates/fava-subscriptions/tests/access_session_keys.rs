//! Access identity remains part of the session key around compiled demand.

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use fava_query::{ObservationId, Query, QueryBranchId};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_subscriptions::demand_for_query;
use nostr::key::Keys;
use nostr::types::RelayUrl;

#[test]
fn same_query_and_url_compile_into_distinct_access_session_buckets()
-> Result<(), Box<dyn std::error::Error>> {
    let relay = RelayUrl::parse("wss://relay.example")?;
    let alice = Keys::generate().public_key();
    let public_query = Query::events().with_relay_access(RelayAccess::Public);
    let private_query = Query::events().with_relay_access(RelayAccess::Authenticated(alice));
    let public = RelaySessionKey {
        relay: relay.clone(),
        access: public_query.access().clone(),
    };
    let private = RelaySessionKey {
        relay,
        access: private_query.access().clone(),
    };
    let mut by_session = BTreeMap::new();
    by_session.insert(
        public.clone(),
        vec![demand_for_query(
            ObservationId::new(NonZeroU64::new(1).unwrap()),
            QueryBranchId::ROOT,
            &public_query,
        )],
    );
    by_session.insert(
        private.clone(),
        vec![demand_for_query(
            ObservationId::new(NonZeroU64::new(2).unwrap()),
            QueryBranchId::ROOT,
            &private_query,
        )],
    );
    assert_eq!(by_session.len(), 2);
    assert_eq!(by_session[&public].len(), 1);
    assert_eq!(by_session[&private].len(), 1);
    Ok(())
}
