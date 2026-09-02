//! A relay's outstanding NIP-42 challenge is evidence, not silence — for the
//! demand that actually asked for authentication.
//!
//! `RelaySourceState::AuthenticationRequired` lost its only producer when the
//! component that used to hold a second opinion about a session's
//! authentication was removed — authentication is now a fact kept on the
//! connection itself (`fava_relay::Authentication`). This proves an
//! observation whose own demand needs an account still learns "the relay is
//! asking, and nobody has answered yet" from there, in the relay's own
//! challenge text.

mod support;

use fava_query::{Progress, Query, RelaySourceState};
use fava_relay::Authority;
use fava_wire::RelayMessage;
use nostr::key::Keys;
use support::{assemble, push, relay, relay_evidence, requests, wait_until};

#[tokio::test(flavor = "current_thread")]
async fn an_outstanding_challenge_reaches_the_observation_that_asked_for_it()
-> Result<(), Box<dyn std::error::Error>> {
    let url = relay("authenticating");
    let assembly = assemble();
    let alice = Keys::generate().public_key();

    let observation = assembly.observer.open(
        Query::events()
            .only_from_relays([url.clone()])?
            .with_relay_access(Authority::As(alice)),
    )?;
    wait_until(|| {
        assembly
            .transport
            .relay(&url, &Authority::As(alice))
            .is_some()
    })
    .await;
    let peer = assembly
        .transport
        .relay(&url, &Authority::As(alice))
        .expect("the authenticated session established");
    wait_until(|| requests(Some(peer.clone())).len() == 1).await;

    push(&peer, &RelayMessage::auth("nonce-one"));
    wait_until(|| {
        matches!(
            relay_evidence(&observation, &url).state,
            RelaySourceState::AuthenticationRequired { .. }
        )
    })
    .await;

    let evidence = relay_evidence(&observation, &url);
    assert!(
        matches!(
            &evidence.state,
            RelaySourceState::AuthenticationRequired {
                progress: Progress::Requested { challenge },
                ..
            } if challenge == "nonce-one"
        ),
        "the observation should carry the relay's own challenge, got {:?}",
        evidence.state
    );

    observation.close();
    Ok(())
}
