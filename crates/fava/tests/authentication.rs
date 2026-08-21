//! Public-facade NIP-42 evidence: explicit access, generation scope, isolation.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava::{
    Authentication, AuthenticationPolicy, AuthorizationDecision, Fava, Query, RelayChallenge,
    RelayDeliveryOutcome, WriteIntent, WriteRouting,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::Signer;
use fava_signer_local::LocalSigner;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions_no_grouping::planner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_wire::{ClientMessage, RelayMessage};
use fava_write::{EventBuilder as WriteEventBuilder, Kind as WriteKind};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::{Keys, PublicKey};
use tokio::sync::Notify;

/// One relay that requires NIP-42 before serving or accepting anything.
struct AuthenticatingRelay {
    challenge: String,
    stored: Vec<Event>,
    sessions: Mutex<Vec<Arc<SessionState>>>,
    next_generation: AtomicU64,
}

#[derive(Default)]
struct SessionState {
    key: Mutex<Option<RelaySessionKey>>,
    authenticated: AtomicBool,
    identity: Mutex<Option<PublicKey>>,
    sent: Mutex<Vec<String>>,
    inbound: Mutex<Vec<String>>,
    notify: Notify,
    closed: AtomicBool,
}

impl SessionState {
    fn push(&self, message: &RelayMessage<'_>) {
        self.inbound
            .lock()
            .expect("test lock")
            .push(serde_json::to_string(message).expect("relay message encodes"));
        self.notify.notify_waiters();
    }
}

impl AuthenticatingRelay {
    fn new(challenge: &str, stored: Vec<Event>) -> Self {
        Self {
            challenge: challenge.to_owned(),
            stored,
            sessions: Mutex::new(Vec::new()),
            next_generation: AtomicU64::new(0),
        }
    }

    fn authenticated_identities(&self) -> Vec<PublicKey> {
        self.sessions
            .lock()
            .expect("test lock")
            .iter()
            .filter_map(|session| *session.identity.lock().expect("test lock"))
            .collect()
    }

    fn handle(&self, state: &SessionState, frame: &str) {
        let Ok(message) = serde_json::from_str::<ClientMessage<'static>>(frame) else {
            return;
        };
        match message {
            ClientMessage::Req {
                subscription_id, ..
            } => {
                let id = subscription_id.into_owned();
                if state.authenticated.load(Ordering::SeqCst) {
                    for event in &self.stored {
                        state.push(&RelayMessage::event(id.clone(), event.clone()));
                    }
                    state.push(&RelayMessage::eose(id));
                } else {
                    state.push(&RelayMessage::closed(
                        id,
                        "auth-required: this relay serves authenticated sessions",
                    ));
                    state.push(&RelayMessage::auth(self.challenge.clone()));
                }
            }
            ClientMessage::Auth(event) => {
                let answers = event.kind.as_u16() == 22242
                    && event.verify().is_ok()
                    && event.tags.iter().any(|tag| {
                        tag.clone().to_vec() == vec!["challenge".to_owned(), self.challenge.clone()]
                    });
                if answers {
                    state.authenticated.store(true, Ordering::SeqCst);
                    *state.identity.lock().expect("test lock") = Some(event.pubkey);
                }
                state.push(&RelayMessage::ok(event.id, answers, ""));
            }
            ClientMessage::Event(event) => {
                if state.authenticated.load(Ordering::SeqCst) {
                    state.push(&RelayMessage::ok(event.id, true, ""));
                } else {
                    state.push(&RelayMessage::auth(self.challenge.clone()));
                }
            }
            _ => {}
        }
    }
}

struct RelayFleet {
    relays: BTreeMap<RelayUrl, Arc<AuthenticatingRelay>>,
}

impl Transport for RelayFleet {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        Box::pin(async move {
            let relay = self
                .relays
                .get(&key.relay)
                .cloned()
                .ok_or_else(|| TransportError::ConnectionRefused("unknown relay".to_owned()))?;
            let generation = relay.next_generation.fetch_add(1, Ordering::SeqCst) + 1;
            let state = Arc::new(SessionState::default());
            *state.key.lock().expect("test lock") = Some(key.clone());
            relay
                .sessions
                .lock()
                .expect("test lock")
                .push(Arc::clone(&state));
            Ok(Arc::new(FleetSession {
                key,
                generation,
                relay,
                state,
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct FleetSession {
    key: RelaySessionKey,
    generation: u64,
    relay: Arc<AuthenticatingRelay>,
    state: Arc<SessionState>,
}

impl RelaySession for FleetSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn send(&self, frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move {
            if self.state.closed.load(Ordering::SeqCst) {
                return HandoffOutcome::NotHandedOff {
                    reason: "session is closed".to_owned(),
                };
            }
            self.relay.handle(&self.state, &frame);
            self.state.sent.lock().expect("test lock").push(frame);
            HandoffOutcome::HandedOff
        })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            loop {
                if self.state.closed.load(Ordering::SeqCst) {
                    return Err(TransportError::Closed);
                }
                let next = {
                    let mut inbound = self.state.inbound.lock().expect("test lock");
                    if inbound.is_empty() {
                        None
                    } else {
                        Some(inbound.remove(0))
                    }
                };
                if let Some(frame) = next {
                    return Ok(frame);
                }
                self.state.notify.notified().await;
            }
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move {
            self.state.closed.store(true, Ordering::SeqCst);
            self.state.notify.notify_waiters();
            Ok(())
        })
    }
}

/// Authorize exactly the relay-access names the application trusts.
struct AccessPolicy {
    permitted: BTreeMap<String, PublicKey>,
}

impl AuthenticationPolicy for AccessPolicy {
    fn authorize<'a>(
        &'a self,
        challenge: &'a RelayChallenge,
    ) -> Pin<Box<dyn Future<Output = AuthorizationDecision> + Send + 'a>> {
        Box::pin(async move {
            self.permitted
                .get(challenge.session().access.as_str())
                .map_or_else(
                    || {
                        AuthorizationDecision::Decline(format!(
                            "relay access {} is not authorized",
                            challenge.session().access.as_str()
                        ))
                    },
                    |identity| AuthorizationDecision::Authorize(*identity),
                )
        })
    }
}

async fn wait_until(limit: Duration, mut ready: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + limit;
    while !ready() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "condition was not reached before the deadline"
        );
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

#[tokio::test]
async fn an_authenticated_relay_serves_the_query_after_demand_is_restored() {
    let relay_url = RelayUrl::parse("wss://auth.example").expect("relay URL");
    let author = Keys::generate();
    let account = Keys::generate();
    let stored = EventBuilder::new(Kind::TextNote, "authenticated read")
        .custom_created_at(Timestamp::from(10))
        .finalize(&author)
        .expect("event signs");
    let relay = Arc::new(AuthenticatingRelay::new("read-nonce", vec![stored.clone()]));
    let transport = Arc::new(RelayFleet {
        relays: BTreeMap::from([(relay_url.clone(), Arc::clone(&relay))]),
    });
    let authentication = Arc::new(Authentication::new(
        Arc::new(AccessPolicy {
            permitted: BTreeMap::from([("reader".to_owned(), account.public_key())]),
        }),
        [Arc::new(LocalSigner::new(account.clone())) as Arc<dyn Signer>],
    ));
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(transport)
        .authentication(Arc::clone(&authentication))
        .build()
        .expect("assembly builds");

    let query = Query::events()
        .authors([author.public_key()])
        .with_relay_access(RelayAccess::named("reader"))
        .only_from_relays([relay_url.clone()])
        .expect("explicit relay is valid");
    let observation = fava.observe(query).await.expect("live query opens");

    wait_until(Duration::from_secs(2), || {
        !observation.current().events.is_empty()
    })
    .await;
    let snapshot = observation.current();
    assert_eq!(snapshot.events.len(), 1);
    assert_eq!(snapshot.events[0].id(), stored.id);
    assert_eq!(
        relay.authenticated_identities(),
        vec![account.public_key()],
        "the relay authenticated exactly the identity the policy authorized"
    );
    let diagnostics = fava.diagnostics();
    assert!(
        diagnostics
            .authenticated
            .iter()
            .any(|(key, _, identity)| key.relay == relay_url
                && identity == &account.public_key().to_hex())
    );
    assert!(
        !diagnostics.authentication_required.is_empty(),
        "the AUTH challenge remains an exact reported fact"
    );
    observation.close();
}

#[tokio::test]
async fn declining_one_account_leaves_the_other_account_publishing() {
    let relay_url = RelayUrl::parse("wss://auth.example").expect("relay URL");
    let alice = Keys::generate();
    let bob = Keys::generate();
    let relay = Arc::new(AuthenticatingRelay::new("write-nonce", Vec::new()));
    let transport = Arc::new(RelayFleet {
        relays: BTreeMap::from([(relay_url.clone(), Arc::clone(&relay))]),
    });
    let authentication = Arc::new(Authentication::new(
        Arc::new(AccessPolicy {
            permitted: BTreeMap::from([("alice".to_owned(), alice.public_key())]),
        }),
        [Arc::new(LocalSigner::new(alice.clone())) as Arc<dyn Signer>],
    ));
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::clone(&transport))
        .signer(Arc::new(LocalSigner::new(alice.clone())))
        .signer(Arc::new(LocalSigner::new(bob.clone())))
        .publisher(Arc::new(Nip01Publisher::authenticated(Arc::clone(
            &authentication,
        ))))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .authentication(Arc::clone(&authentication))
        .build()
        .expect("assembly builds");

    let routing = WriteRouting::Explicit([relay_url.clone()].into_iter().collect());
    let alice_receipt = fava
        .publish(
            WriteIntent::event(
                WriteEventBuilder::new(alice.public_key(), WriteKind::TextNote)
                    .content("from alice")
                    .build()
                    .expect("event builds"),
                routing.clone(),
            )
            .expect("intent is valid")
            .with_relay_access(RelayAccess::named("alice")),
        )
        .expect("alice write is accepted")
        .receipt_id;
    let bob_receipt = fava
        .publish(
            WriteIntent::event(
                WriteEventBuilder::new(bob.public_key(), WriteKind::TextNote)
                    .content("from bob")
                    .build()
                    .expect("event builds"),
                routing,
            )
            .expect("intent is valid")
            .with_relay_access(RelayAccess::named("bob")),
        )
        .expect("bob write is accepted")
        .receipt_id;

    let alice_terminal =
        tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(alice_receipt))
            .await
            .expect("alice receipt settles")
            .expect("alice receipt is readable");
    let bob_terminal =
        tokio::time::timeout(Duration::from_secs(5), fava.wait_terminal(bob_receipt))
            .await
            .expect("bob receipt settles")
            .expect("bob receipt is readable");

    let alice_destination = RelaySessionKey::new(relay_url.clone(), RelayAccess::named("alice"));
    let bob_destination = RelaySessionKey::new(relay_url, RelayAccess::named("bob"));
    assert_eq!(
        alice_terminal.destinations().get(&alice_destination),
        Some(&RelayDeliveryOutcome::Acknowledged {
            message: String::new()
        }),
        "the authorized account still reaches the relay"
    );
    let bob_outcome = bob_terminal
        .destinations()
        .get(&bob_destination)
        .expect("bob has an exact destination fact");
    assert!(
        matches!(
            bob_outcome,
            RelayDeliveryOutcome::AuthenticationDenied { .. }
        ),
        "the declined account terminates with auth denial, got {bob_outcome:?}"
    );
    assert_eq!(
        relay.authenticated_identities(),
        vec![alice.public_key()],
        "only the authorized account ever authenticated"
    );
}
