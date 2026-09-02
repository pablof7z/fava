//! One authenticated session, a fake relay, and a policy under test.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy, Authenticator,
};
use fava_relay::Authority;
use fava_runtime::{Runtime, RuntimeConfig};
use fava_session::Session;
use fava_signer_local::LocalSigner;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_write::EventId;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use serde_json::{Value, json};

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).expect("constant is non-zero")
}

/// A relay `AUTH` challenge frame.
pub fn challenge_frame(challenge: &str) -> Vec<u8> {
    json!(["AUTH", challenge]).to_string().into_bytes()
}

/// A relay `OK` frame for one event id.
pub fn ok_frame(event_id: &EventId, status: bool, message: &str) -> Vec<u8> {
    json!(["OK", event_id.to_hex(), status, message])
        .to_string()
        .into_bytes()
}

/// Every client `AUTH` frame the relay actually received.
pub fn auth_frames(relay: &FakeRelay) -> Vec<Value> {
    relay
        .delivered_frames()
        .into_iter()
        .filter_map(|frame| serde_json::from_slice::<Value>(&frame).ok())
        .filter(|value| value.get(0).and_then(Value::as_str) == Some("AUTH"))
        .collect()
}

/// A policy that always answers the same way.
struct Fixed(AuthenticationDecision);

impl AuthenticationPolicy for Fixed {
    fn decide(&self, _demand: &AuthenticationDemand) -> AuthenticationDecision {
        self.0
    }
}

pub struct Rig {
    authenticator: Authenticator,
    transport: Arc<FakeTransport>,
    relay: RelayUrl,
    authority: Authority,
    account: fava_write::PublicKey,
    /// Somebody has to want this connection. Nothing authenticates a relay no
    /// component is talking to.
    lease: fava_transport::RelaySessionLease,
}

impl Rig {
    async fn build(
        decision: impl FnOnce(fava_write::PublicKey) -> AuthenticationDecision,
        attach_signer: bool,
    ) -> Self {
        let keys = Keys::generate();
        let account = keys.public_key();
        let signers = if attach_signer {
            Session::new([Arc::new(LocalSigner::new(keys)) as Arc<dyn fava_signer::Signer>])
                .expect("one signer")
        } else {
            Session::new([]).expect("no signers")
        };
        let transport = Arc::new(FakeTransport::new());
        let runtime = Runtime::new(RuntimeConfig {
            default_channel_depth: nonzero(1_024),
            max_tasks: nonzero(1_024),
            max_provider_operations: nonzero(256),
        });
        let authenticator =
            Authenticator::new(signers, Arc::new(Fixed(decision(account))), runtime);
        let relay = RelayUrl::parse("wss://relay.example.com").expect("valid relay url");
        let authority = Authority::As(account);
        authenticator
            .answer_requests(transport.as_ref())
            .expect("the owner begins answering");
        // Nothing authenticates a relay nobody has connected to, so the rig
        // opens the session the test will drive.
        let lease = fava_transport::Transport::acquire_session(
            transport.as_ref(),
            fava_transport::OpenRelaySession {
                relay: relay.clone(),
                authority,
                deadlines: fava_transport::TransportDeadlines {
                    establish: std::time::Duration::from_secs(1),
                    write: std::time::Duration::from_secs(1),
                    idle: std::time::Duration::from_secs(1),
                    close: std::time::Duration::from_secs(1),
                },
                bounds: fava_transport::TransportBounds {
                    inbound_frames: std::num::NonZeroUsize::new(64).expect("non-zero"),
                    outbound_frames: std::num::NonZeroUsize::new(4).expect("non-zero"),
                    max_frame_bytes: std::num::NonZeroUsize::new(1_048_576).expect("non-zero"),
                },
                reconnect_attempts: None,
            },
        )
        .await
        .expect("the rig opens the session under test");
        let rig = Self {
            authenticator,
            transport,
            relay,
            authority,
            account,
            lease,
        };
        rig.settle().await;
        rig
    }

    pub async fn approving() -> Self {
        Self::build(|as_of| AuthenticationDecision::Authenticate { as_of }, true).await
    }

    pub async fn approving_without_signer() -> Self {
        Self::build(
            |as_of| AuthenticationDecision::Authenticate { as_of },
            false,
        )
        .await
    }

    pub async fn declining() -> Self {
        Self::build(|_| AuthenticationDecision::Decline, true).await
    }

    pub async fn deferring() -> Self {
        Self::build(|_| AuthenticationDecision::Defer, true).await
    }

    pub const fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }

    pub const fn relay_url(&self) -> &RelayUrl {
        &self.relay
    }

    pub const fn account(&self) -> fava_write::PublicKey {
        self.account
    }

    pub fn relay(&self) -> FakeRelay {
        self.transport
            .relay(&self.relay, &self.authority)
            .expect("the watch acquired this session")
    }

    /// How far authentication has got, read from the connection that holds it.
    pub fn state(&self) -> fava_relay::Authentication {
        fava_transport::RelaySessionExt::connection(&self.session())
            .borrow()
            .authentication
            .clone()
    }

    /// The session under test.
    pub fn session(&self) -> std::sync::Arc<dyn fava_transport::RelaySession> {
        std::sync::Arc::clone(self.lease.session())
    }

    /// Let the watch task drain what the relay pushed.
    pub async fn settle(&self) {
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
    }

    /// Event id of the most recent client `AUTH` frame.
    pub fn last_auth_event_id(&self) -> EventId {
        let frames = auth_frames(&self.relay());
        let last = frames.last().expect("an AUTH frame was sent");
        let hex = last
            .get(1)
            .and_then(|event| event.get("id"))
            .and_then(Value::as_str)
            .expect("the AUTH frame carries a signed event");
        EventId::from_hex(hex).expect("valid event id")
    }
}
