//! Declared NIP-11 limits reach planning and publication before any bytes move.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use fava::{Fava, Query, RelayDeliveryOutcome, WriteIntent, WriteRouting};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_nip11::{
    RelayInformation, RelayInformationError, RelayInformationFetcher, RelayLimitation,
};
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer_local::LocalSigner;
use fava_state::{RelaySessionKey, RelayUrl};
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{HandoffOutcome, RelaySession, Transport, TransportError};
use fava_write::{EventBuilder, Kind};
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;

/// A relay-information service that answers with one fixed document.
struct FixedRelayInformation {
    limitation: RelayLimitation,
}

impl RelayInformationFetcher for FixedRelayInformation {
    fn get(
        &self,
        _relay: RelayUrl,
    ) -> Pin<Box<dyn Future<Output = Result<RelayInformation, RelayInformationError>> + Send + '_>>
    {
        Box::pin(async move {
            Ok(RelayInformation {
                limitation: self.limitation,
                ..RelayInformation::default()
            })
        })
    }
}

/// A relay-information service that is always unreachable.
struct UnreachableRelayInformation;

impl RelayInformationFetcher for UnreachableRelayInformation {
    fn get(
        &self,
        _relay: RelayUrl,
    ) -> Pin<Box<dyn Future<Output = Result<RelayInformation, RelayInformationError>> + Send + '_>>
    {
        Box::pin(async move {
            Err(RelayInformationError::Unreachable(
                "no route to relay information".to_owned(),
            ))
        })
    }
}

/// A transport that records every connection attempt and answers nothing.
#[derive(Default)]
struct CountingTransport {
    opened: AtomicUsize,
}

impl Transport for CountingTransport {
    fn open_session(
        &self,
        key: RelaySessionKey,
    ) -> Pin<Box<dyn Future<Output = Result<Arc<dyn RelaySession>, TransportError>> + Send + '_>>
    {
        self.opened.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            Ok(Arc::new(SilentSession {
                key,
                closed: AtomicBool::new(false),
            }) as Arc<dyn RelaySession>)
        })
    }
}

struct SilentSession {
    key: RelaySessionKey,
    closed: AtomicBool,
}

impl RelaySession for SilentSession {
    fn key(&self) -> &RelaySessionKey {
        &self.key
    }

    fn generation(&self) -> u64 {
        1
    }

    fn send(&self, _frame: String) -> Pin<Box<dyn Future<Output = HandoffOutcome> + Send + '_>> {
        Box::pin(async move { HandoffOutcome::HandedOff })
    }

    fn next_message(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, TransportError>> + Send + '_>> {
        Box::pin(async move {
            std::future::pending::<()>().await;
            Err(TransportError::Closed)
        })
    }

    fn close(&self) -> Pin<Box<dyn Future<Output = Result<(), TransportError>> + Send + '_>> {
        Box::pin(async move {
            self.closed.store(true, Ordering::SeqCst);
            Ok(())
        })
    }
}

fn relay_url() -> RelayUrl {
    RelayUrl::parse("ws://127.0.0.1:9").expect("relay URL")
}

fn query() -> Query {
    Query::events()
        .authors([Keys::generate().public_key()])
        .only_from_relays([relay_url()])
        .expect("explicit relay is valid")
}

#[tokio::test]
async fn a_declared_message_limit_refuses_the_query_before_any_connection() {
    let transport = Arc::new(CountingTransport::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::clone(&transport))
        .relay_information(Arc::new(FixedRelayInformation {
            limitation: RelayLimitation {
                max_message_length: Some(32),
                ..RelayLimitation::default()
            },
        }))
        .build()
        .expect("assembly builds");

    let Err(error) = fava.observe(query()).await else {
        panic!("the relay declares it cannot carry this REQ");
    };

    assert!(
        error.to_string().contains("relay allows 32"),
        "the refusal names the exact declared bound, got {error}"
    );
    assert_eq!(
        transport.opened.load(Ordering::SeqCst),
        0,
        "no connection is opened for work the relay has refused in advance"
    );
    let diagnostics = fava.diagnostics();
    assert!(
        diagnostics
            .relay_limit_shortfalls
            .iter()
            .any(|(session, reason)| session.relay == relay_url()
                && reason.contains("relay allows 32")),
        "the exact shortfall is a reported fact"
    );
    assert!(
        diagnostics
            .relay_limits
            .iter()
            .any(|(_, declared)| declared.contains("max_message_length=32"))
    );
}

#[tokio::test]
async fn an_unreachable_relay_information_document_leaves_limits_unknown() {
    let transport = Arc::new(CountingTransport::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::clone(&transport))
        .relay_information(Arc::new(UnreachableRelayInformation))
        .build()
        .expect("assembly builds");

    let Ok(observation) = fava.observe(query()).await else {
        panic!("an unknown claim invents no bound");
    };

    assert_eq!(transport.opened.load(Ordering::SeqCst), 1);
    assert!(
        fava.diagnostics()
            .relay_limits
            .iter()
            .any(|(_, declared)| declared.starts_with("unknown: ")),
        "why the limits are unknown stays an exact reported fact"
    );
    assert!(fava.diagnostics().relay_limit_shortfalls.is_empty());
    observation.close();
}

#[tokio::test]
async fn a_declared_content_limit_refuses_publication_before_any_connection() {
    let keys = Keys::generate();
    let transport = Arc::new(CountingTransport::default());
    let information = Arc::new(FixedRelayInformation {
        limitation: RelayLimitation {
            max_content_length: Some(4),
            ..RelayLimitation::default()
        },
    });
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::clone(&transport))
        .signer(Arc::new(LocalSigner::new(keys.clone())))
        .publisher(Arc::new(
            Nip01Publisher::new().with_relay_information(Arc::clone(&information)),
        ))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .relay_information(information)
        .build()
        .expect("assembly builds");

    let accepted = fava
        .publish(
            WriteIntent::event(
                EventBuilder::new(keys.public_key(), Kind::TextNote)
                    .content("far longer than four bytes")
                    .build()
                    .expect("event builds"),
                WriteRouting::Explicit([relay_url()].into_iter().collect()),
            )
            .expect("intent is valid"),
        )
        .expect("write is accepted");

    let receipt = tokio::time::timeout(
        Duration::from_secs(5),
        fava.wait_terminal(accepted.receipt_id),
    )
    .await
    .expect("receipt settles")
    .expect("receipt is readable");

    let outcome = receipt
        .destinations()
        .values()
        .next()
        .expect("one exact destination fact");
    let RelayDeliveryOutcome::RefusedByLimit { reason } = outcome else {
        panic!("expected a declared-limit refusal, got {outcome:?}");
    };
    assert!(
        reason.contains("max_content_length=4"),
        "the refusal names the exact declared bound, got {reason}"
    );
    assert_eq!(
        transport.opened.load(Ordering::SeqCst),
        0,
        "knowingly invalid work never reaches a connection"
    );
}

#[tokio::test]
async fn an_undeclared_content_limit_refuses_nothing() {
    let keys = Keys::generate();
    let transport = Arc::new(CountingTransport::default());
    let fava = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner::default()))
        .transport(Arc::clone(&transport))
        .signer(Arc::new(LocalSigner::new(keys.clone())))
        .publisher(Arc::new(Nip01Publisher::new().with_relay_information(
            Arc::new(FixedRelayInformation {
                limitation: RelayLimitation::default(),
            }),
        )))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .build()
        .expect("assembly builds");

    fava.publish(
        WriteIntent::event(
            EventBuilder::new(keys.public_key(), Kind::TextNote)
                .content("far longer than four bytes")
                .build()
                .expect("event builds"),
            WriteRouting::Explicit([relay_url()].into_iter().collect()),
        )
        .expect("intent is valid"),
    )
    .expect("write is accepted");

    let mut opened = 0;
    for _ in 0..500 {
        opened = transport.opened.load(Ordering::SeqCst);
        if opened > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert_eq!(
        opened, 1,
        "a relay that declares no content limit still receives the attempt"
    );
}
