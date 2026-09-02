//! External compile-surface proof for the authentication-owned API.

use std::collections::BTreeSet;

use fava_auth::{AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy, Challenge};
use fava_transport::{RelayConnection, RelaySessionIdentity};
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("valid relay url")
}

fn demand(url: &str, generation: u64) -> AuthenticationDemand {
    AuthenticationDemand {
        session: RelaySessionIdentity {
            relay: relay(url),
            connection: RelayConnection::new(generation).expect("non-zero connection"),
        },
        challenge: Challenge::new("opaque-nonce").expect("bounded challenge"),
    }
}

#[test]
fn a_demand_names_the_exact_relay_and_generation_it_arrived_on() {
    let demand = demand("wss://relay.example.com", 7);

    assert_eq!(demand.session.relay, relay("wss://relay.example.com"));
    assert_eq!(demand.session.connection.get(), 7);
    assert_eq!(demand.challenge.as_str(), "opaque-nonce");
}

/// A policy that carries state answers through the trait.
struct ApprovedRelays {
    approved: BTreeSet<RelayUrl>,
    account: PublicKey,
}

impl AuthenticationPolicy for ApprovedRelays {
    fn decide(&self, demand: &AuthenticationDemand) -> AuthenticationDecision {
        if self.approved.contains(&demand.session.relay) {
            AuthenticationDecision::Authenticate {
                as_of: self.account,
            }
        } else {
            AuthenticationDecision::Decline
        }
    }
}

#[test]
fn a_stateful_policy_and_a_bare_closure_are_both_policies() {
    let account = Keys::generate().public_key();
    let approved = demand("wss://approved.example.com", 1);
    let unknown = demand("wss://unknown.example.com", 1);

    let stateful = ApprovedRelays {
        approved: BTreeSet::from([relay("wss://approved.example.com")]),
        account,
    };
    assert_eq!(
        stateful.decide(&approved),
        AuthenticationDecision::Authenticate { as_of: account }
    );
    assert_eq!(stateful.decide(&unknown), AuthenticationDecision::Decline);

    // No adapter between a closure and the trait: the blanket impl is the seam.
    let closure = |demand: &AuthenticationDemand| {
        if demand.session.relay == relay("wss://approved.example.com") {
            AuthenticationDecision::Authenticate { as_of: account }
        } else {
            AuthenticationDecision::Defer
        }
    };
    let closure: &dyn AuthenticationPolicy = &closure;
    assert_eq!(
        closure.decide(&approved),
        AuthenticationDecision::Authenticate { as_of: account }
    );
    assert_eq!(closure.decide(&unknown), AuthenticationDecision::Defer);
}

#[test]
fn a_demand_is_answered_by_the_connection_it_arrived_on_not_a_minted_identity() {
    let first = demand("wss://relay.example.com", 3);
    let reconnected = demand("wss://relay.example.com", 4);

    // `demand.session` is a `RelaySessionIdentity`: the same value `pending`
    // returns and `answer` takes. Nothing else names a demand.
    assert_eq!(first.session.relay, reconnected.session.relay, "same relay");
    assert_ne!(
        first.session, reconnected.session,
        "a later generation is a different connection, and answers nothing signed for the earlier one"
    );
}
