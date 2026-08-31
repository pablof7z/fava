//! An assembled engine authenticates to a relay through its public API alone.
//!
//! Every part of the NIP-42 lifecycle existed before this: the owner answered
//! challenges, bounded them, deferred them to a person, and voided them on
//! reconnect. None of it was reachable, because nothing assembled the owner
//! into an engine an application builds. These tests drive the capability the
//! way an application does -- `Fava::builder()`, `observe`, `authentication()`
//! -- and never call a provider directly.

use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query};
use fava_auth::{AuthenticationDecision, AuthenticationDemand, AuthenticationPolicy};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{AuthenticationState, RelayAccess, RelaySessionKey};
use fava_signer_local::LocalSigner;
use fava_subscriptions_no_grouping::planner;
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use nostr::types::RelayUrl;
use serde_json::{Value, json};

/// A relay `AUTH` challenge frame.
fn challenge_frame(challenge: &str) -> Vec<u8> {
    json!(["AUTH", challenge]).to_string().into_bytes()
}

/// Every client `AUTH` frame the relay actually received.
fn auth_frames(relay: &FakeRelay) -> Vec<Value> {
    relay
        .delivered_frames()
        .into_iter()
        .filter_map(|frame| serde_json::from_slice::<Value>(&frame).ok())
        .filter(|message| message.get(0).and_then(Value::as_str) == Some("AUTH"))
        .collect()
}

/// Let every owner-held task reach quiescence without advancing wall time.
async fn settle() {
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(10)).await;
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
}

/// A publisher these tests never exercise: they are about the challenge
/// handshake on a read session, not about delivery.
struct SilentPublisher;

impl fava_publisher::Publisher for SilentPublisher {
    fn publish<'a>(
        &'a self,
        _attempt: fava_publisher::PublishAttempt,
        _transport: &'a dyn fava_transport::Transport,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = fava_publisher::PublishOutcome> + Send + 'a>,
    > {
        Box::pin(async {
            fava_publisher::PublishOutcome::OutcomeUnknown {
                reason: "these tests do not publish".to_owned(),
            }
        })
    }
}

/// A policy that answers every challenge the same way.
struct Always(AuthenticationDecision);

impl AuthenticationPolicy for Always {
    fn decide(&self, _demand: &AuthenticationDemand) -> AuthenticationDecision {
        self.0
    }
}

struct Rig {
    fava: Fava,
    transport: Arc<FakeTransport>,
    key: RelaySessionKey,
    relay: RelayUrl,
    account: nostr::key::PublicKey,
}

impl Rig {
    /// Assemble one engine the way an application does.
    fn build(policy: Option<Always>, attach_signer: bool) -> Self {
        let keys = Keys::generate();
        let account = keys.public_key();
        let relay = RelayUrl::parse("wss://authenticated.example").expect("relay URL");
        let key = RelaySessionKey {
            relay: relay.clone(),
            access: RelayAccess::Authenticated(account),
        };
        let transport = Arc::new(FakeTransport::new());

        let mut builder = Fava::builder()
            .event_cache(Arc::new(MemoryEventCache::default()))
            .write_store(Arc::new(MemoryWriteStore::default()))
            .query_evaluator(Arc::new(StandardQueryEvaluator))
            .subscription_planner(Arc::new(planner()))
            .transport(Arc::clone(&transport));
        if attach_signer {
            // A signer turns on publication assembly, which then wants its own
            // providers. Authentication needs the signer; the rest is the price
            // of an assembly that can also publish.
            builder = builder
                .signer(Arc::new(LocalSigner::new(keys)))
                .publisher(Arc::new(SilentPublisher))
                .delivery_policy(Arc::new(StandardDeliveryPolicy::default()));
        }
        if let Some(policy) = policy {
            builder = builder.authentication_policy(Arc::new(policy));
        }
        let fava = builder.build().expect("assembly is complete");

        Self {
            fava,
            transport,
            key,
            relay,
            account,
        }
    }

    /// One live query under this rig's authenticated account.
    fn query(&self) -> Query {
        Query::events()
            .only_from_relays([self.relay.clone()])
            .expect("relay selection")
            .with_relay_access(RelayAccess::Authenticated(self.account))
    }

    fn peer(&self) -> Option<FakeRelay> {
        self.transport.relay(&self.key)
    }
}

/// The whole point: an application builds an engine, opens a query, and the
/// relay's challenge is answered without the application doing anything else.
#[tokio::test(flavor = "current_thread")]
async fn an_assembled_engine_answers_a_challenge_through_its_public_api() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Authenticate)), true);
    let _observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    let peer = rig.peer().expect("the authenticated session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert_eq!(
        auth_frames(&peer).len(),
        1,
        "one approved challenge is answered exactly once"
    );
    assert_eq!(
        rig.fava
            .authentication()
            .expect("a policy was selected")
            .state(&rig.key),
        Some(AuthenticationState::Attempted),
        "the owner records that it answered"
    );
}

/// An assembly that selects no policy authenticates to nothing. Silence is the
/// answer, not a default.
#[tokio::test(flavor = "current_thread")]
async fn an_engine_without_a_policy_authenticates_to_nothing() {
    let rig = Rig::build(None, true);
    let _observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    let peer = rig.peer().expect("the session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert!(
        auth_frames(&peer).is_empty(),
        "no policy, no signature, no frame"
    );
    assert!(
        rig.fava.authentication().is_none(),
        "an engine with no policy exposes no authentication owner"
    );
}

/// A declining policy signs nothing, and says so.
#[tokio::test(flavor = "current_thread")]
async fn a_declining_policy_signs_nothing() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Decline)), true);
    let _observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    let peer = rig.peer().expect("the session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert!(auth_frames(&peer).is_empty(), "a decline sends no frame");
    assert_eq!(
        rig.fava
            .authentication()
            .expect("a policy was selected")
            .state(&rig.key),
        Some(AuthenticationState::Declined)
    );
}

/// A deferred challenge is enumerable, signals its arrival, and is answered out
/// of band -- the seam an application needs when a person owns the decision.
#[tokio::test(flavor = "current_thread")]
async fn a_deferred_challenge_reaches_the_application_and_is_answered_out_of_band() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Defer)), true);
    let _observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    let authentication = rig.fava.authentication().expect("a policy was selected");
    let changes = authentication.subscribe();
    let peer = rig.peer().expect("the session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert!(
        auth_frames(&peer).is_empty(),
        "a deferred challenge signs nothing until a person answers"
    );
    assert!(changes.has_changed().unwrap_or(false), "the set changed");

    let pending = authentication.pending();
    assert_eq!(pending.len(), 1, "exactly one demand awaits a person");
    assert_eq!(pending[0].session.key, rig.key);

    authentication
        .answer(pending[0].id, AuthenticationDecision::Authenticate)
        .await
        .expect("the demand is answered");
    settle().await;

    assert_eq!(
        auth_frames(&peer).len(),
        1,
        "approving the demand sends exactly one AUTH frame"
    );
    assert!(
        authentication.pending().is_empty(),
        "an answered demand no longer awaits anyone"
    );
}

/// A public query never authenticates, and never asks the policy.
#[tokio::test(flavor = "current_thread")]
async fn a_public_query_authenticates_nothing() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Authenticate)), true);
    let public = Query::events()
        .only_from_relays([rig.relay.clone()])
        .expect("relay selection");
    let _observation = rig.fava.observe(public).await.expect("live query opens");
    settle().await;

    let public_key = RelaySessionKey {
        relay: rig.relay.clone(),
        access: RelayAccess::Public,
    };
    let peer = rig
        .transport
        .relay(&public_key)
        .expect("the public session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert!(
        auth_frames(&peer).is_empty(),
        "public access is never authenticated"
    );
}

/// The relay's demand reaches the observation's own evidence, sourced from the
/// owner that determined it rather than decoded a second time from the wire.
#[tokio::test(flavor = "current_thread")]
async fn an_observation_reports_the_relays_demand_from_the_owners_conclusion() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Decline)), true);
    let observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    let peer = rig.peer().expect("the session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    // Anything from the relay refreshes this relay's evidence; a closure is the
    // ordinary way a relay ends a subscription it will not serve.
    let requests: Vec<String> = peer
        .delivered_frames()
        .into_iter()
        .filter_map(|frame| serde_json::from_slice::<Value>(&frame).ok())
        .filter(|message| message.get(0).and_then(Value::as_str) == Some("REQ"))
        .filter_map(|message| {
            message
                .get(1)
                .and_then(Value::as_str)
                .map(std::borrow::ToOwned::to_owned)
        })
        .collect();
    let subscription = requests.first().expect("a REQ was sent").clone();
    peer.push_frame(
        json!([
            "CLOSED",
            subscription,
            "auth-required: we only serve authenticated users"
        ])
        .to_string()
        .as_bytes(),
    );
    settle().await;

    let snapshot = observation.current();
    let state = snapshot
        .evidence
        .relay(&rig.key)
        .map(|occurrence| occurrence.state.clone())
        .expect("the observation carries evidence for this relay");
    assert!(
        matches!(
            state,
            fava_query::RelaySourceState::AuthenticationRequired {
                state: AuthenticationState::Declined,
                ..
            }
        ),
        "the observation reports what the authentication owner concluded, got {state:?}"
    );
}

/// OWN-07's `auth_denied_for_one_access_context_leaves_another_running`.
///
/// A relay is one host and several session authorities. Denying one account
/// must not disturb another account's work, nor public-access work, on that
/// same host: they are different connections with different identities.
#[tokio::test(flavor = "current_thread")]
async fn auth_denied_for_one_access_context_leaves_another_running() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Decline)), true);

    let denied = rig
        .fava
        .observe(rig.query())
        .await
        .expect("the authenticated query opens");
    let public_query = Query::events()
        .only_from_relays([rig.relay.clone()])
        .expect("relay selection");
    let public = rig
        .fava
        .observe(public_query)
        .await
        .expect("the public query opens");
    settle().await;

    let public_key = RelaySessionKey {
        relay: rig.relay.clone(),
        access: RelayAccess::Public,
    };
    let denied_peer = rig.peer().expect("the authenticated session");
    let public_peer = rig
        .transport
        .relay(&public_key)
        .expect("the public session");

    denied_peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert_eq!(
        rig.fava
            .authentication()
            .expect("a policy was selected")
            .state(&rig.key),
        Some(AuthenticationState::Declined),
        "the authenticated context was denied"
    );
    assert!(
        rig.fava
            .authentication()
            .expect("a policy was selected")
            .state(&public_key)
            .is_none(),
        "public access is never authenticated, so it has no verdict"
    );
    assert!(
        public.current().evidence.relay(&public_key).is_some(),
        "the public observation still carries evidence for its own session"
    );
    assert!(
        denied.current().evidence.relay(&rig.key).is_some(),
        "the denied observation stays open and reports its own relay"
    );

    // The public session keeps serving: its own subscription still completes.
    let public_subscription = requests_of(&public_peer)
        .first()
        .cloned()
        .expect("the public query sent a REQ");
    public_peer.push_frame(json!(["EOSE", public_subscription]).to_string().as_bytes());
    settle().await;

    let snapshot = public.current();
    assert!(
        snapshot
            .evidence
            .relay(&public_key)
            .is_some_and(fava_query::RelayQueryEvidence::stored_events_complete),
        "public work on the same host completes while another account is denied"
    );
}

/// Every wire subscription identifier the relay was sent.
fn requests_of(peer: &FakeRelay) -> Vec<String> {
    peer.delivered_frames()
        .into_iter()
        .filter_map(|frame| serde_json::from_slice::<Value>(&frame).ok())
        .filter(|message| message.get(0).and_then(Value::as_str) == Some("REQ"))
        .filter_map(|message| {
            message
                .get(1)
                .and_then(Value::as_str)
                .map(std::borrow::ToOwned::to_owned)
        })
        .collect()
}

/// Watching for challenges must not hold a relay session open by itself.
///
/// The owner takes its own lease so that an unsolicited challenge is seen with
/// no query attached. That lease would otherwise keep the socket alive forever,
/// because a lease is exactly what keeps a session from closing.
#[tokio::test(flavor = "current_thread")]
async fn the_watch_releases_its_lease_when_no_authenticated_work_remains() {
    let rig = Rig::build(Some(Always(AuthenticationDecision::Authenticate)), true);
    let observation = rig
        .fava
        .observe(rig.query())
        .await
        .expect("live query opens");
    settle().await;

    assert!(
        fava_transport::Transport::holders(rig.transport.as_ref(), &rig.key)
            .is_some_and(|holders| holders.get() >= 2),
        "the observation and the watch each hold the session"
    );

    drop(observation);
    // The watch checks periodically whether it is the last holder; nothing
    // announces that the last observation ended.
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        settle().await;
        if fava_transport::Transport::holders(rig.transport.as_ref(), &rig.key).is_none() {
            break;
        }
    }

    assert!(
        fava_transport::Transport::holders(rig.transport.as_ref(), &rig.key).is_none(),
        "the watch let go once nothing was left to serve, got {:?}",
        fava_transport::Transport::holders(rig.transport.as_ref(), &rig.key)
    );
}
