//! Bounded exact relay diagnostic evidence.

use std::num::NonZeroUsize;

use fava_diagnostics::Diagnostics;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use nostr::message::SubscriptionId;

fn session(name: &str) -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL"),
        RelayAccess::public(),
    )
}

#[test]
fn eose_closed_auth_failure_and_withdrawal_remain_distinct_and_bounded() {
    let diagnostics = Diagnostics::bounded(NonZeroUsize::new(2).expect("non-zero"));
    let first = session("first");
    let second = session("second");
    let third = session("third");
    let id = SubscriptionId::new("sub");

    diagnostics.session_opened(first.clone(), 1);
    diagnostics.session_opened(second.clone(), 1);
    diagnostics.session_opened(third.clone(), 1);
    diagnostics.subscription_opened(third.clone(), 1, id.clone());
    diagnostics.eose(third.clone(), 1, id.clone());
    diagnostics.closed(third.clone(), 1, id.clone(), "blocked".to_owned());
    diagnostics.authentication_required(third.clone(), 1);
    diagnostics.failed(third.clone(), 1, "disconnected".to_owned());
    diagnostics.withdrawn(third.clone(), 1, id.clone());

    let snapshot = diagnostics.snapshot();
    assert_eq!(snapshot.sessions.len(), 2);
    assert!(!snapshot.sessions.iter().any(|(key, _)| key == &first));
    assert_eq!(snapshot.eose, vec![(third.clone(), 1, id.clone())]);
    assert_eq!(snapshot.closed.len(), 1);
    assert_eq!(snapshot.authentication_required, vec![(third.clone(), 1)]);
    assert_eq!(snapshot.failures.len(), 1);
    assert_eq!(snapshot.withdrawn, vec![(third, 1, id)]);
}
