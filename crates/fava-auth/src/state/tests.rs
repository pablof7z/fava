use fava_relay::{AuthenticationState, BoundedText, RelayAccess, RelaySessionKey};
use fava_transport::RelaySessionGeneration;
use nostr::key::Keys;
use nostr::types::RelayUrl;

use super::SessionAuthentication;
use crate::challenge::Challenge;

fn session() -> RelaySessionKey {
    RelaySessionKey {
        relay: RelayUrl::parse("wss://relay.example.com").expect("valid relay url"),
        access: RelayAccess::Authenticated(Keys::generate().public_key()),
    }
}

fn generation(value: u64) -> RelaySessionGeneration {
    RelaySessionGeneration::new(value).expect("non-zero generation")
}

fn challenge(text: &str) -> Challenge {
    Challenge::new(text).expect("bounded challenge")
}

#[test]
fn a_later_challenge_replaces_an_earlier_one_on_the_same_generation() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("first"));
    authentication.challenged(generation(1), challenge("second"));

    assert_eq!(
        authentication.challenge().map(Challenge::as_str),
        Some("second"),
        "the relay's current challenge is the only one worth answering"
    );
    assert_eq!(
        authentication.attempts(),
        0,
        "replacing a challenge on one generation spends no attempt"
    );
}

#[test]
fn attempts_exhaust_the_bound_on_one_generation() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("c"));

    for spent in 0..SessionAuthentication::MAX_ATTEMPTS {
        assert!(
            authentication.may_attempt(),
            "attempt {spent} is within the bound"
        );
        authentication.resolved(generation(1), AuthenticationState::Attempted);
    }

    assert_eq!(
        authentication.attempts(),
        SessionAuthentication::MAX_ATTEMPTS
    );
    assert!(
        !authentication.may_attempt(),
        "a relay that re-challenges without end stops being answered"
    );
}

#[test]
fn a_reconnect_clears_an_earlier_verdict_and_refills_the_budget() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("c"));
    authentication.resolved(generation(1), AuthenticationState::Attempted);
    authentication.resolved(generation(1), AuthenticationState::Accepted);
    assert!(authentication.authenticated());

    authentication.reconnected(generation(2));

    assert!(
        !authentication.authenticated(),
        "a replaced connection begins unauthenticated"
    );
    assert_eq!(authentication.state(), None);
    assert_eq!(authentication.challenge(), None);
    assert_eq!(authentication.attempts(), 0);
}

#[test]
fn a_verdict_for_a_replaced_generation_is_dropped() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("c"));
    authentication.reconnected(generation(2));

    authentication.resolved(generation(1), AuthenticationState::Accepted);

    assert!(
        !authentication.authenticated(),
        "a late verdict cannot authenticate the connection that replaced it"
    );
    assert_eq!(authentication.state(), None);
}

#[test]
fn a_challenge_on_a_new_generation_resets_the_budget() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("c"));
    authentication.resolved(generation(1), AuthenticationState::Attempted);

    authentication.challenged(generation(2), challenge("c"));

    assert_eq!(authentication.attempts(), 0);
    assert_eq!(authentication.generation(), Some(generation(2)));
}

#[test]
fn a_refusal_retains_the_relays_own_text() {
    let mut authentication = SessionAuthentication::new(session());
    authentication.challenged(generation(1), challenge("c"));
    authentication.resolved(
        generation(1),
        AuthenticationState::AcceptedButStillRefused {
            message: BoundedText::new("restricted: not a member"),
        },
    );

    let Some(AuthenticationState::AcceptedButStillRefused { message }) = authentication.state()
    else {
        panic!("the refusal state is retained");
    };
    assert_eq!(message.as_str(), "restricted: not a member");
    assert!(
        !authentication.authenticated(),
        "authenticated but refused is not authenticated for this work"
    );
}
