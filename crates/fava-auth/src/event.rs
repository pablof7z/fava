//! The kind-22242 challenge response.

use fava_write::{EventBuildError, EventBuilder, Kind, PublicKey, Tag, UnsignedEvent};
use nostr::types::RelayUrl;

use crate::challenge::Challenge;

/// NIP-42 authentication event kind.
const AUTHENTICATION: u16 = 22242;

/// Build the unsigned kind-22242 challenge response.
///
/// The relay verifies the kind, that `created_at` is near its own clock, that
/// the challenge tag matches what it sent, and that the relay tag names it.
/// The challenge is echoed byte-exact, which is why [`Challenge`] refuses
/// oversized text rather than truncating it.
///
/// # Errors
///
/// Returns [`EventBuildError`] when a tag or the event body cannot be built.
pub fn auth_event(
    identity: PublicKey,
    relay: &RelayUrl,
    challenge: &Challenge,
) -> Result<UnsignedEvent, EventBuildError> {
    let relay_tag = Tag::parse(["relay", relay.as_str()])
        .map_err(|error| EventBuildError::Encoding(error.to_string()))?;
    let challenge_tag = Tag::parse(["challenge", challenge.as_str()])
        .map_err(|error| EventBuildError::Encoding(error.to_string()))?;
    EventBuilder::new(Kind::from_u16(AUTHENTICATION))
        .tag(relay_tag)
        .tag(challenge_tag)
        .by(identity)
        .build()
}

#[cfg(test)]
mod tests {
    use fava_write::PublicKey;
    use nostr::key::Keys;
    use nostr::types::RelayUrl;

    use super::{AUTHENTICATION, auth_event};
    use crate::challenge::Challenge;

    fn rows(event: &fava_write::UnsignedEvent, name: &str) -> Vec<Vec<String>> {
        event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .filter(|row| row.first().map(String::as_str) == Some(name))
            .collect()
    }

    #[test]
    fn the_response_names_its_relay_and_echoes_the_challenge_exactly() {
        let identity: PublicKey = Keys::generate().public_key();
        let relay = RelayUrl::parse("wss://relay.example.com").expect("valid relay url");
        let challenge = Challenge::new("opaque-nonce").expect("bounded challenge");

        let event = auth_event(identity, &relay, &challenge).expect("the response builds");

        assert_eq!(event.kind.as_u16(), AUTHENTICATION);
        assert_eq!(event.pubkey, identity);
        assert_eq!(
            rows(&event, "relay"),
            vec![vec!["relay".to_owned(), relay.as_str().to_owned()]]
        );
        assert_eq!(
            rows(&event, "challenge"),
            vec![vec!["challenge".to_owned(), "opaque-nonce".to_owned()]],
            "the challenge is echoed byte-exact or the relay refuses it"
        );
    }
}
