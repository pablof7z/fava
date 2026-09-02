//! ARCH-6b.6: answering one relay must not delay answering another.
//!
//! `answer_requests` reads every relay's ask off one broadcast. A signer can
//! be remote and slow; if reading the next ask waited on the current one
//! being answered, one slow signer would starve every other relay's demand,
//! and a full 64-slot backlog would drop it outright.

use std::collections::BTreeMap;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy, Authenticator,
};
use fava_relay::Authority;
use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::Session;
use fava_signer::{Signer, SignerAvailability, SignerError};
use fava_signer_local::LocalSigner;
use fava_transport::{OpenRelaySession, Transport, TransportBounds, TransportDeadlines};
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_write::{Event, PublicKey, UnsignedEvent};
use nostr::event::FinalizeEvent;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use serde_json::{Value, json};
use tokio::sync::{Notify, watch};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("constant is non-zero")
}

fn challenge_frame(challenge: &str) -> Vec<u8> {
    json!(["AUTH", challenge]).to_string().into_bytes()
}

/// Every client `AUTH` frame one relay actually received.
fn auth_frames(relay: &FakeRelay) -> Vec<Value> {
    relay
        .delivered_frames()
        .into_iter()
        .filter_map(|frame| serde_json::from_slice::<Value>(&frame).ok())
        .filter(|value| value.get(0).and_then(Value::as_str) == Some("AUTH"))
        .collect()
}

/// Let the watch and answer tasks drain what the relay pushed.
async fn settle() {
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
}

/// A signer that will not finalize until a test releases its gate, standing
/// in for a remote signer a person has not yet approved on.
struct GatedSigner {
    keys: Keys,
    gate: Arc<Notify>,
}

impl Signer for GatedSigner {
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
            self.gate.notified().await;
            event
                .finalize(&self.keys)
                .map_err(|error| SignerError::InvalidOutput(error.to_string()))
        })
    }
}

/// Authenticates each relay as the account this test assigned it.
struct PerRelay(BTreeMap<RelayUrl, PublicKey>);

impl AuthenticationPolicy for PerRelay {
    fn decide(&self, demand: &AuthenticationDemand) -> AuthenticationDecision {
        let as_of = *self
            .0
            .get(&demand.session.relay)
            .expect("this test names every relay it challenges");
        AuthenticationDecision::Authenticate { as_of }
    }
}

async fn open(
    transport: &FakeTransport,
    relay: &RelayUrl,
    authority: Authority,
) -> fava_transport::RelaySessionLease {
    transport
        .acquire_session(OpenRelaySession {
            relay: relay.clone(),
            authority,
            deadlines: TransportDeadlines {
                establish: Duration::from_secs(1),
                write: Duration::from_secs(1),
                idle: Duration::from_secs(1),
                close: Duration::from_secs(1),
            },
            bounds: TransportBounds {
                inbound_frames: nonzero(64),
                outbound_frames: nonzero(4),
                max_frame_bytes: nonzero(1_048_576),
            },
            reconnect_attempts: None,
        })
        .await
        .expect("the test opens the session under test")
}

#[tokio::test]
async fn a_slow_signer_on_one_relay_does_not_delay_answering_another() {
    let slow_keys = Keys::generate();
    let fast_keys = Keys::generate();
    let gate = Arc::new(Notify::new());

    let slow_relay = RelayUrl::parse("wss://slow.example.com").expect("valid relay url");
    let fast_relay = RelayUrl::parse("wss://fast.example.com").expect("valid relay url");

    let signers = Session::new([
        Arc::new(GatedSigner {
            keys: slow_keys.clone(),
            gate: Arc::clone(&gate),
        }) as Arc<dyn Signer>,
        Arc::new(LocalSigner::new(fast_keys.clone())) as Arc<dyn Signer>,
    ])
    .expect("two distinct signers");

    let accounts = BTreeMap::from([
        (slow_relay.clone(), slow_keys.public_key()),
        (fast_relay.clone(), fast_keys.public_key()),
    ]);

    let runtime = Runtime::new(RuntimeConfig {
        default_channel_depth: nonzero(1_024),
        max_tasks: nonzero(1_024),
        max_provider_operations: nonzero(256),
    });
    let authenticator = Authenticator::new(signers, Arc::new(PerRelay(accounts)), runtime);

    let transport = FakeTransport::new();
    authenticator
        .answer_requests(
            &(std::sync::Arc::new(transport.clone())
                as std::sync::Arc<dyn fava_transport::Transport>),
        )
        .expect("the owner begins answering");

    let slow_authority = Authority::As(slow_keys.public_key());
    let fast_authority = Authority::As(fast_keys.public_key());
    let _slow_lease = open(&transport, &slow_relay, slow_authority).await;
    let _fast_lease = open(&transport, &fast_relay, fast_authority).await;
    let slow_relay_handle = transport
        .relay(&slow_relay, &slow_authority)
        .expect("acquired above");
    let fast_relay_handle = transport
        .relay(&fast_relay, &fast_authority)
        .expect("acquired above");

    // The slow relay's challenge arrives first; its signer blocks on the gate.
    slow_relay_handle.push_frame(&challenge_frame("slow-nonce"));
    settle().await;
    assert!(
        auth_frames(&slow_relay_handle).is_empty(),
        "the slow relay's signer has not been released yet"
    );

    // The fast relay's challenge arrives while the slow one is still stuck.
    fast_relay_handle.push_frame(&challenge_frame("fast-nonce"));
    settle().await;

    // Proof: the fast relay was answered without waiting for the slow
    // signer, which is still gated and has not returned.
    assert_eq!(
        auth_frames(&fast_relay_handle).len(),
        1,
        "a slow signer on one relay must not delay answering another"
    );
    assert!(
        auth_frames(&slow_relay_handle).is_empty(),
        "the slow relay is still gated, so it must still be unanswered"
    );

    // Release the slow signer and confirm its answer still arrives.
    gate.notify_one();
    settle().await;
    assert_eq!(
        auth_frames(&slow_relay_handle).len(),
        1,
        "the slow relay is answered once its signer returns"
    );
}
