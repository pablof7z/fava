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
use fava_relay::Authority;
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

/// A policy answering every challenge the same way, named before the rig's
/// account exists: `Rig::build` closes it over that account once it does, so
/// `Authenticate` names who to authenticate as without the caller needing to
/// generate the keys first.
#[derive(Clone, Copy)]
enum Always {
    Authenticate,
    Decline,
    Defer,
}

struct AlwaysAs {
    decision: Always,
    account: nostr::key::PublicKey,
}

impl AuthenticationPolicy for AlwaysAs {
    fn decide(&self, _demand: &AuthenticationDemand) -> AuthenticationDecision {
        match self.decision {
            Always::Authenticate => AuthenticationDecision::Authenticate {
                as_of: self.account,
            },
            Always::Decline => AuthenticationDecision::Decline,
            Always::Defer => AuthenticationDecision::Defer,
        }
    }
}

struct Rig {
    fava: Fava,
    transport: Arc<FakeTransport>,
    relay: RelayUrl,
    account: nostr::key::PublicKey,
}

impl Rig {
    /// The authority this rig's own query authenticates as.
    fn authority(&self) -> Authority {
        Authority::As(self.account)
    }

    /// How far authentication has got, read from the connection that holds it.
    fn authentication(&self) -> Option<fava_relay::Authentication> {
        let session = self.transport.session(&self.relay, &self.authority())?;
        Some(
            fava_transport::RelaySessionExt::connection(&session)
                .borrow()
                .authentication
                .clone(),
        )
    }

    /// Assemble one engine the way an application does.
    fn build(policy: Option<Always>, attach_signer: bool) -> Self {
        let keys = Keys::generate();
        let account = keys.public_key();
        let relay = RelayUrl::parse("wss://authenticated.example").expect("relay URL");
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
        if let Some(decision) = policy {
            builder = builder.authentication_policy(Arc::new(AlwaysAs { decision, account }));
        }
        let fava = builder.build().expect("assembly is complete");

        Self {
            fava,
            transport,
            relay,
            account,
        }
    }

    /// One live query under this rig's authenticated account.
    fn query(&self) -> Query {
        Query::events()
            .only_from_relays([self.relay.clone()])
            .expect("relay selection")
            .with_relay_access(self.authority())
    }

    fn peer(&self) -> Option<FakeRelay> {
        self.transport.relay(&self.relay, &self.authority())
    }
}

/// The whole point: an application builds an engine, opens a query, and the
/// relay's challenge is answered without the application doing anything else.
#[tokio::test(flavor = "current_thread")]
async fn an_assembled_engine_answers_a_challenge_through_its_public_api() {
    let rig = Rig::build(Some(Always::Authenticate), true);
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
        rig.authentication(),
        Some(fava_relay::Authentication::Authenticating { as_of: rig.account }),
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
    let rig = Rig::build(Some(Always::Decline), true);
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
        rig.authentication(),
        Some(fava_relay::Authentication::Declined)
    );
}

/// A deferred challenge is enumerable, signals its arrival, and is answered out
/// of band -- the seam an application needs when a person owns the decision.
#[tokio::test(flavor = "current_thread")]
async fn a_deferred_challenge_reaches_the_application_and_is_answered_out_of_band() {
    let rig = Rig::build(Some(Always::Defer), true);
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
    assert_eq!(pending[0].session.relay, rig.relay);

    authentication
        .answer(
            pending[0].id,
            AuthenticationDecision::Authenticate { as_of: rig.account },
        )
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

/// A connection opened for public work still asks the policy when its relay
/// challenges it, and a policy naming an account still answers: deciding
/// whether to authenticate and deciding as whom is the policy's call, not a
/// structural guarantee tied to what the connection was acquired for. Access
/// stopped being identity, so nothing about how a connection was opened
/// exempts it from a later challenge.
#[tokio::test(flavor = "current_thread")]
async fn a_challenge_on_a_public_connection_still_reaches_the_policy() {
    let rig = Rig::build(Some(Always::Authenticate), true);
    let public = Query::events()
        .only_from_relays([rig.relay.clone()])
        .expect("relay selection");
    let _observation = rig.fava.observe(public).await.expect("live query opens");
    settle().await;

    let peer = rig
        .transport
        .relay(&rig.relay, &Authority::Unauthenticated)
        .expect("the public session was acquired");
    peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert_eq!(
        auth_frames(&peer).len(),
        1,
        "the policy named an account, so it answers even a publicly opened connection"
    );
}

/// The relay's demand reaches the observation's own evidence, sourced from the
/// owner that determined it rather than decoded a second time from the wire.
#[tokio::test(flavor = "current_thread")]
async fn an_observation_reports_the_relays_demand_from_the_owners_conclusion() {
    let rig = Rig::build(Some(Always::Decline), true);
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
        .relay(&rig.relay)
        .map(|occurrence| occurrence.state.clone())
        .expect("the observation carries evidence for this relay");
    // The observation reports what the relay said to it, in the relay's own
    // words. How authentication went is a fact about the connection, read
    // from the connection.
    assert!(
        matches!(state, fava_query::RelaySourceState::Refused { .. }),
        "the observation keeps the relay's own refusal, got {state:?}"
    );
    assert_eq!(
        rig.authentication(),
        Some(fava_relay::Authentication::Declined),
        "and the connection says why it was never authenticated"
    );
}

/// OWN-07's `auth_denied_for_one_access_context_leaves_another_running`.
///
/// A relay is one host. Denying one account's authentication must not disturb
/// public work on the same host: a decline still lets a connection carry
/// anonymous work (a connection nobody authenticated can still become
/// anyone's -- and, symmetrically, one the relay refused to authenticate can
/// still carry the work that never asked to be authenticated). Whether the
/// two share the underlying connection or not, the public observation's own
/// evidence and subscription must be unaffected by the other's refusal.
#[tokio::test(flavor = "current_thread")]
async fn auth_denied_for_one_access_context_leaves_another_running() {
    let rig = Rig::build(Some(Always::Decline), true);

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

    let denied_peer = rig.peer().expect("the authenticated session");
    let public_peer = rig
        .transport
        .relay(&rig.relay, &Authority::Unauthenticated)
        .expect("the public session");

    denied_peer.push_frame(&challenge_frame("nonce-one"));
    settle().await;

    assert_eq!(
        rig.authentication(),
        Some(fava_relay::Authentication::Declined),
        "the authenticated context was denied"
    );
    assert!(
        public.current().evidence.relay(&rig.relay).is_some(),
        "the public observation still carries evidence for its own session"
    );
    assert!(
        denied.current().evidence.relay(&rig.relay).is_some(),
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
            .relay(&rig.relay)
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
