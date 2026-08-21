//! Generation-scoped, relay-access-isolated NIP-42 behavior.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use fava_auth::{
    Authentication, AuthenticationOutcome, AuthenticationPolicy, AuthorizationDecision,
    RelayChallenge,
};
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_transport::{HandoffOutcome, RelaySession, TransportError};
use fava_wire::{ClientMessage, RelayMessage};
use fava_write::PublicKey;
use nostr::key::Keys;

/// One scripted relay session that answers exactly one client AUTH frame.
struct ScriptedSession {
    key: RelaySessionKey,
    generation: u64,
    status: bool,
    message: String,
    sent: Mutex<Vec<String>>,
    inbound: Mutex<Vec<String>>,
}

impl ScriptedSession {
    fn new(key: RelaySessionKey, generation: u64, status: bool, message: &str) -> Self {
        Self {
            key,
            generation,
            status,
            message: message.to_owned(),
            sent: Mutex::new(Vec::new()),
            inbound: Mutex::new(Vec::new()),
        }
    }

    fn sent_frames(&self) -> Vec<String> {
        self.sent.lock().expect("test lock").clone()
    }
}

impl RelaySession for ScriptedSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if let Ok(ClientMessage::Auth(event)) =
                serde_json::from_str::<ClientMessage<'static>>(&frame)
            {
                let answer = RelayMessage::ok(event.id, self.status, self.message.clone());
                self.inbound
                    .lock()
                    .expect("test lock")
                    .push(encode_client_answer(&answer));
            }
            self.sent.lock().expect("test lock").push(frame);
            HandoffOutcome::HandedOff
        })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            let frame = self.inbound.lock().expect("test lock").pop();
            frame.ok_or(TransportError::Closed)
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move { Ok(()) })
    }
}

fn encode_client_answer(message: &RelayMessage<'static>) -> String {
    serde_json::to_string(message).expect("relay message encodes")
}

struct FixedPolicy(AuthorizationDecision);

impl AuthenticationPolicy for FixedPolicy {
    fn authorize<'a>(
        &'a self,
        _challenge: &'a RelayChallenge,
    ) -> Pin<Box<dyn Future<Output = AuthorizationDecision> + Send + 'a>> {
        Box::pin(async move { self.0.clone() })
    }
}

/// Authorize only the exact relay access the application trusts.
struct PerAccessPolicy {
    permitted: String,
    identity: PublicKey,
}

impl AuthenticationPolicy for PerAccessPolicy {
    fn authorize<'a>(
        &'a self,
        challenge: &'a RelayChallenge,
    ) -> Pin<Box<dyn Future<Output = AuthorizationDecision> + Send + 'a>> {
        Box::pin(async move {
            if challenge.session().access.as_str() == self.permitted {
                AuthorizationDecision::Authorize(self.identity)
            } else {
                AuthorizationDecision::Decline(format!(
                    "relay access {} is not authorized",
                    challenge.session().access.as_str()
                ))
            }
        })
    }
}

fn session_key(access: &str) -> RelaySessionKey {
    RelaySessionKey::new(
        RelayUrl::parse("ws://127.0.0.1:9").expect("test relay parses"),
        RelayAccess::named(access),
    )
}

#[tokio::test]
async fn declining_one_relay_access_leaves_another_account_authenticated() {
    let permitted = Keys::generate();
    let denied = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(PerAccessPolicy {
            permitted: "permitted".to_owned(),
            identity: permitted.public_key(),
        }),
        [
            Arc::new(LocalSigner::new(permitted.clone())) as Arc<dyn Signer>,
            Arc::new(LocalSigner::new(denied)) as Arc<dyn Signer>,
        ],
    );

    let allowed = ScriptedSession::new(session_key("permitted"), 1, true, "");
    let refused = ScriptedSession::new(session_key("denied"), 1, true, "");
    let allowed_challenge =
        RelayChallenge::new(session_key("permitted"), 1, "nonce-a").expect("bounded challenge");
    let refused_challenge =
        RelayChallenge::new(session_key("denied"), 1, "nonce-b").expect("bounded challenge");

    let refused_outcome = authentication.answer(&refused_challenge, &refused).await;
    let allowed_outcome = authentication.answer(&allowed_challenge, &allowed).await;

    assert!(matches!(
        refused_outcome,
        AuthenticationOutcome::Declined { .. }
    ));
    assert_eq!(
        allowed_outcome,
        AuthenticationOutcome::Accepted {
            identity: permitted.public_key(),
            message: String::new(),
        }
    );
    assert!(
        refused.sent_frames().is_empty(),
        "a declined relay access must not hand off an AUTH frame"
    );
    assert_eq!(allowed.sent_frames().len(), 1);
}

#[tokio::test]
async fn a_challenge_from_a_retired_generation_produces_no_relay_work() {
    let keys = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(FixedPolicy(AuthorizationDecision::Authorize(
            keys.public_key(),
        ))),
        [Arc::new(LocalSigner::new(keys)) as Arc<dyn Signer>],
    );
    let session = ScriptedSession::new(session_key("account"), 2, true, "");
    let stale = RelayChallenge::new(session_key("account"), 1, "nonce").expect("bounded challenge");

    let outcome = authentication.answer(&stale, &session).await;

    assert!(matches!(outcome, AuthenticationOutcome::Failed { .. }));
    assert!(session.sent_frames().is_empty());
}

#[tokio::test]
async fn the_authenticated_identity_is_the_policy_choice_not_the_signer_registry_order() {
    let first = Keys::generate();
    let second = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(FixedPolicy(AuthorizationDecision::Authorize(
            second.public_key(),
        ))),
        [
            Arc::new(LocalSigner::new(first)) as Arc<dyn Signer>,
            Arc::new(LocalSigner::new(second.clone())) as Arc<dyn Signer>,
        ],
    );
    let session = ScriptedSession::new(session_key("account"), 3, true, "");
    let challenge =
        RelayChallenge::new(session_key("account"), 3, "nonce").expect("bounded challenge");

    let outcome = authentication.answer(&challenge, &session).await;

    assert_eq!(
        outcome,
        AuthenticationOutcome::Accepted {
            identity: second.public_key(),
            message: String::new(),
        }
    );
    let frame = session.sent_frames().pop().expect("one AUTH frame");
    let ClientMessage::Auth(event) =
        serde_json::from_str::<ClientMessage<'static>>(&frame).expect("AUTH frame decodes")
    else {
        panic!("expected an AUTH frame");
    };
    assert_eq!(event.pubkey, second.public_key());
    assert_eq!(event.kind.as_u16(), 22242);
}

#[tokio::test]
async fn an_unregistered_authorized_identity_fails_before_handoff() {
    let registered = Keys::generate();
    let absent = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(FixedPolicy(AuthorizationDecision::Authorize(
            absent.public_key(),
        ))),
        [Arc::new(LocalSigner::new(registered)) as Arc<dyn Signer>],
    );
    let session = ScriptedSession::new(session_key("account"), 1, true, "");
    let challenge =
        RelayChallenge::new(session_key("account"), 1, "nonce").expect("bounded challenge");

    let outcome = authentication.answer(&challenge, &session).await;

    assert!(matches!(outcome, AuthenticationOutcome::Failed { .. }));
    assert!(session.sent_frames().is_empty());
}

#[tokio::test]
async fn relay_refusal_is_never_reported_as_acceptance() {
    let keys = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(FixedPolicy(AuthorizationDecision::Authorize(
            keys.public_key(),
        ))),
        [Arc::new(LocalSigner::new(keys)) as Arc<dyn Signer>],
    );
    let session = ScriptedSession::new(session_key("account"), 1, false, "restricted: not allowed");
    let challenge =
        RelayChallenge::new(session_key("account"), 1, "nonce").expect("bounded challenge");

    assert_eq!(
        authentication.answer(&challenge, &session).await,
        AuthenticationOutcome::Refused {
            message: "restricted: not allowed".to_owned(),
        }
    );
}

#[tokio::test]
async fn the_answer_frame_is_an_exact_nip42_auth_message() {
    let keys = Keys::generate();
    let authentication = Authentication::new(
        Arc::new(FixedPolicy(AuthorizationDecision::Authorize(
            keys.public_key(),
        ))),
        [Arc::new(LocalSigner::new(keys)) as Arc<dyn Signer>],
    );
    let session = ScriptedSession::new(session_key("account"), 1, true, "");
    let challenge =
        RelayChallenge::new(session_key("account"), 1, "the-nonce").expect("bounded challenge");

    authentication.answer(&challenge, &session).await;

    let frame = session.sent_frames().pop().expect("one AUTH frame");
    assert!(frame.starts_with("[\"AUTH\","));
    let ClientMessage::Auth(event) =
        serde_json::from_str::<ClientMessage<'static>>(&frame).expect("AUTH frame decodes")
    else {
        panic!("expected an AUTH frame");
    };
    event.verify().expect("the AUTH answer is a valid event");
    let tags: Vec<Vec<String>> = event.tags.iter().map(|tag| tag.clone().to_vec()).collect();
    assert!(tags.contains(&vec!["relay".to_owned(), "ws://127.0.0.1:9".to_owned()]));
    assert!(tags.contains(&vec!["challenge".to_owned(), "the-nonce".to_owned()]));
}
