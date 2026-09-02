//! External compile-surface proof for the authentication-owned API.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationDemandId, AuthenticationPolicy,
    Challenge, PendingAuthentication,
};
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
fn a_deferred_demand_carries_a_stable_identity_and_its_generation() {
    let demand = demand("wss://relay.example.com", 3);
    let id = AuthenticationDemandId::from_nonzero(NonZeroU64::new(1).expect("non-zero"));

    let pending = PendingAuthentication {
        id,
        session: demand.session.clone(),
    };

    assert_eq!(pending.id, id, "an answer names the exact demand");
    assert_eq!(
        pending.session.connection, demand.session.connection,
        "an answer given after this generation is replaced resolves nothing"
    );
    assert_ne!(
        id,
        AuthenticationDemandId::from_nonzero(NonZeroU64::new(2).expect("non-zero")),
        "identities are distinct"
    );
}
