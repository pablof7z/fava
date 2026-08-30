//! Gate 1: NIP-42 AUTH handshake end-to-end.
//!
//! Proves: relay demands AUTH → `Nip42Publisher` signs a kind-22242 challenge
//! response and resends the EVENT → relay acknowledges → `Acknowledged` outcome.
//! Wire transcript is inspected to confirm AUTH and EVENT frames both appear.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fava_publisher::{PublishAttempt, PublishOutcome, Publisher};
use fava_publisher_nip01::Nip42Publisher;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_session::Session;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_transport::{OpenRelaySession, Transport, TransportBounds, TransportDeadlines};
use fava_transport_testkit::FakeTransport;
use fava_wire::{ClientMessage, RelayMessage};
use fava_write::{
    Event, EventBuilder, Kind, PublicKey, ReceiptId, RevisionId, UnsignedEvent, WriteId,
};
use nostr::event::FinalizeEvent;
use nostr::key::{Keys, SecretKey};
use nostr::types::RelayUrl;
use tokio::sync::watch;

fn author_keys() -> Keys {
    Keys::new(SecretKey::from_slice(&[42_u8; 32]).expect("constant key"))
}

fn relay_url() -> RelayUrl {
    RelayUrl::parse("wss://auth.example").expect("relay URL")
}

fn session_key() -> RelaySessionKey {
    RelaySessionKey {
        relay: relay_url(),
        access: RelayAccess::Public,
    }
}

fn open_request() -> OpenRelaySession {
    let nonzero = |n: usize| std::num::NonZeroUsize::new(n).expect("non-zero");
    OpenRelaySession {
        key: session_key(),
        deadlines: TransportDeadlines {
            establish: Duration::from_millis(200),
            write: Duration::from_millis(200),
            idle: Duration::from_secs(10),
            close: Duration::from_millis(200),
        },
        bounds: TransportBounds {
            inbound_frames: nonzero(64),
            outbound_frames: nonzero(16),
            max_frame_bytes: nonzero(1_048_576),
        },
        reconnect_attempts: None,
    }
}

/// Inline signer that signs immediately with a fixed key.
struct ImmediateSigner {
    keys: Keys,
}

impl Signer for ImmediateSigner {
    fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    fn availability(&self) -> SignerAvailability {
        SignerAvailability::Available
    }

    fn sign_event(
        self: Arc<Self>,
        event: UnsignedEvent,
        _cancel: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<Event, SignerError>> + Send + 'static>> {
        Box::pin(async move {
            event
                .finalize(&self.keys)
                .map_err(|e| SignerError::InvalidOutput(e.to_string()))
        })
    }
}

fn signed_note() -> Event {
    let keys = author_keys();
    EventBuilder::new(Kind::TextNote)
        .content("nip42 test")
        .by(keys.public_key())
        .build()
        .expect("note builds")
        .finalize(&keys)
        .expect("note signs")
}

fn attempt(event: Event) -> PublishAttempt {
    PublishAttempt {
        write_id: WriteId::try_from(1).expect("nonzero write identity"),
        receipt_id: ReceiptId::try_from(1).expect("nonzero receipt identity"),
        revision_id: RevisionId::FIRST,
        number: 1,
        session: session_key(),
        event,
        timeout: Duration::from_secs(2),
    }
}

fn client_messages(frames: Vec<Vec<u8>>) -> Vec<ClientMessage<'static>> {
    frames
        .into_iter()
        .map(|f| {
            serde_json::from_slice::<ClientMessage<'static>>(&f).expect("client message decodes")
        })
        .collect()
}

/// AUTH challenge from relay → AUTH response signed → resent EVENT → OK accepted.
#[tokio::test(flavor = "multi_thread")]
async fn nip42_auth_handshake_completes_and_event_is_acknowledged() {
    let keys = author_keys();
    let session = Session::new([Arc::new(ImmediateSigner { keys }) as Arc<dyn Signer>])
        .expect("session with one signer");

    let publisher = Nip42Publisher::new(session);
    let transport = Arc::new(FakeTransport::new());

    // Establish the session in the transport before publish so we can get a handle.
    let lease = transport
        .as_ref()
        .acquire_session(open_request())
        .await
        .expect("session established");
    let peer = transport
        .relay(&session_key())
        .expect("peer registered after acquire");

    let event = signed_note();
    let event_id = event.id;
    let attempt_val = attempt(event);

    // Run publisher concurrently while we script the relay side.
    let transport_ref = Arc::clone(&transport);
    let publish_fut =
        tokio::spawn(async move { publisher.publish(attempt_val, &*transport_ref).await });

    // Give the publisher a moment to send the initial EVENT.
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Relay side: demand AUTH.
    let challenge = "test-challenge-string";
    peer.push_frame(
        serde_json::to_vec(&RelayMessage::Auth {
            challenge: std::borrow::Cow::Borrowed(challenge),
        })
        .expect("AUTH encodes"),
    );

    // Give the publisher time to sign and resend EVENT.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Relay side: acknowledge the (re)sent EVENT.
    peer.push_frame(
        serde_json::to_vec(&RelayMessage::Ok {
            event_id,
            status: true,
            message: std::borrow::Cow::Borrowed(""),
        })
        .expect("OK encodes"),
    );

    let outcome = publish_fut.await.expect("publisher task completed");

    assert_eq!(
        outcome,
        PublishOutcome::Acknowledged {
            message: String::new()
        },
        "relay acknowledged after AUTH handshake"
    );

    // Verify wire transcript: initial EVENT, AUTH response, resent EVENT.
    let frames = peer.delivered_frames();
    let messages = client_messages(frames);

    let has_auth = messages.iter().any(|m| matches!(m, ClientMessage::Auth(_)));
    assert!(has_auth, "AUTH response was sent; frames: {messages:?}");

    let events_sent = messages
        .iter()
        .filter(|m| matches!(m, ClientMessage::Event(_)))
        .count();
    assert_eq!(
        events_sent, 2,
        "initial EVENT plus resent EVENT after AUTH; frames: {messages:?}"
    );

    drop(lease);
}

/// No signer for the challenged pubkey → `AuthenticationRequired` without crash.
#[tokio::test(flavor = "multi_thread")]
async fn nip42_without_signer_returns_authentication_required() {
    // Session with no signers attached.
    let session = Session::new([]).expect("empty session");
    let publisher = Nip42Publisher::new(session);
    let transport = Arc::new(FakeTransport::new());

    let lease = transport
        .as_ref()
        .acquire_session(open_request())
        .await
        .expect("session established");
    let peer = transport.relay(&session_key()).expect("peer registered");

    let event = signed_note();
    let event_id = event.id;
    let attempt_val = attempt(event);

    let transport_ref = Arc::clone(&transport);
    let publish_fut =
        tokio::spawn(async move { publisher.publish(attempt_val, &*transport_ref).await });

    tokio::time::sleep(Duration::from_millis(20)).await;

    peer.push_frame(
        serde_json::to_vec(&RelayMessage::Auth {
            challenge: std::borrow::Cow::Borrowed("challenge"),
        })
        .expect("AUTH encodes"),
    );

    let outcome = publish_fut.await.expect("publisher task completed");
    assert_eq!(
        outcome,
        PublishOutcome::AuthenticationRequired,
        "no signer yields AuthenticationRequired"
    );
    drop(lease);
    let _ = event_id; // suppress unused warning
}
