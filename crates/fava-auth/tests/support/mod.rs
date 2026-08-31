//! One authenticated session, a fake relay, and a policy under test.

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;

use fava_auth::{
    AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy, Authenticator,
};
use fava_relay::{AuthenticationState, RelayAccess, RelaySessionKey};
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
    key: RelaySessionKey,
}

impl Rig {
    async fn build(decision: AuthenticationDecision, attach_signer: bool) -> Self {
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
        let authenticator = Authenticator::new(
            Arc::clone(&transport) as Arc<dyn fava_transport::Transport>,
            signers,
            Arc::new(Fixed(decision)),
            runtime,
        );
        let key = RelaySessionKey {
            relay: RelayUrl::parse("wss://relay.example.com").expect("valid relay url"),
            access: RelayAccess::Authenticated(account),
        };
        authenticator
            .watch_session(key.clone())
            .await
            .expect("the watch begins");
        let rig = Self {
            authenticator,
            transport,
            key,
        };
        rig.settle().await;
        rig
    }

    pub async fn approving() -> Self {
        Self::build(AuthenticationDecision::Authenticate, true).await
    }

    pub async fn approving_without_signer() -> Self {
        Self::build(AuthenticationDecision::Authenticate, false).await
    }

    pub async fn declining() -> Self {
        Self::build(AuthenticationDecision::Decline, true).await
    }

    pub async fn deferring() -> Self {
        Self::build(AuthenticationDecision::Defer, true).await
    }

    pub const fn authenticator(&self) -> &Authenticator {
        &self.authenticator
    }

    pub const fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    pub fn relay(&self) -> FakeRelay {
        self.transport
            .relay(&self.key)
            .expect("the watch acquired this session")
    }

    pub fn state(&self) -> Option<AuthenticationState> {
        self.authenticator.state(&self.key)
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
