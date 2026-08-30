//! External compile-surface proof for the authentication-owned API.

use std::collections::BTreeSet;
use std::num::NonZeroU64;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationDemandId, AuthenticationPolicy,
    Challenge, PendingAuthentication,
};
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_transport::{RelaySessionGeneration, RelaySessionIdentity};
use nostr::key::{Keys, PublicKey};
use nostr::types::RelayUrl;

fn relay(url: &str) -> RelayUrl {
    RelayUrl::parse(url).expect("valid relay url")
}

fn demand(url: &str, account: PublicKey, generation: u64) -> AuthenticationDemand {
    AuthenticationDemand {
        session: RelaySessionIdentity {
            key: RelaySessionKey {
                relay: relay(url),
                access: RelayAccess::Authenticated(account),
            },
            generation: RelaySessionGeneration::new(generation).expect("non-zero generation"),
        },
        challenge: Challenge::new("opaque-nonce").expect("bounded challenge"),
    }
}

#[test]
fn a_demand_names_the_exact_session_and_generation_it_arrived_on() {
    let account = Keys::generate().public_key();
    let demand = demand("wss://relay.example.com", account, 7);

    assert_eq!(
        demand.session.key.access,
        RelayAccess::Authenticated(account),
        "the account to authenticate as is part of the session identity"
    );
    assert_eq!(demand.session.generation.get(), 7);
    assert_eq!(demand.challenge.as_str(), "opaque-nonce");
}

/// A policy that carries state answers through the trait.
struct ApprovedRelays {
    approved: BTreeSet<RelayUrl>,
}

impl AuthenticationPolicy for ApprovedRelays {
    fn decide(&self, demand: &AuthenticationDemand) -> AuthenticationDecision {
        if self.approved.contains(&demand.session.key.relay) {
            AuthenticationDecision::Authenticate
        } else {
            AuthenticationDecision::Decline
        }
    }
}

#[test]
fn a_stateful_policy_and_a_bare_closure_are_both_policies() {
    let account = Keys::generate().public_key();
    let approved = demand("wss://approved.example.com", account, 1);
    let unknown = demand("wss://unknown.example.com", account, 1);

    let stateful = ApprovedRelays {
        approved: BTreeSet::from([relay("wss://approved.example.com")]),
    };
    assert_eq!(
        stateful.decide(&approved),
        AuthenticationDecision::Authenticate
    );
    assert_eq!(stateful.decide(&unknown), AuthenticationDecision::Decline);

    // No adapter between a closure and the trait: the blanket impl is the seam.
    let closure = |demand: &AuthenticationDemand| {
        if demand.session.key.relay == relay("wss://approved.example.com") {
            AuthenticationDecision::Authenticate
        } else {
            AuthenticationDecision::Defer
        }
    };
    let closure: &dyn AuthenticationPolicy = &closure;
    assert_eq!(
        closure.decide(&approved),
        AuthenticationDecision::Authenticate
    );
    assert_eq!(closure.decide(&unknown), AuthenticationDecision::Defer);
}

#[test]
fn a_deferred_demand_carries_a_stable_identity_and_its_generation() {
    let account = Keys::generate().public_key();
    let demand = demand("wss://relay.example.com", account, 3);
    let id = AuthenticationDemandId::from_nonzero(NonZeroU64::new(1).expect("non-zero"));

    let pending = PendingAuthentication {
        id,
        session: demand.session.clone(),
    };

    assert_eq!(pending.id, id, "an answer names the exact demand");
    assert_eq!(
        pending.session.generation, demand.session.generation,
        "an answer given after this generation is replaced resolves nothing"
    );
    assert_ne!(
        id,
        AuthenticationDemandId::from_nonzero(NonZeroU64::new(2).expect("non-zero")),
        "identities are distinct"
    );
}
