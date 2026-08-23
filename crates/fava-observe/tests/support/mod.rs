//! Scripted providers for owner-level live-query evidence.

// Each integration test binary compiles this module and uses part of it.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use fava_diagnostics::Diagnostics;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_observe::{Observation, ObserveError, Observer};
use fava_query::{ObservationId, QueryEvaluator, QuerySource, RelayQueryEvidence};
use fava_query_standard::StandardQueryEvaluator;
use fava_routing::Router;
use fava_runtime::{Runtime, RuntimeConfig};
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl};
use fava_subscriptions::{
    InstalledSubscriptions, PlanRevision, RelayDemand, RelayReadConstraints, SubscriptionPlan,
    SubscriptionPlanError, SubscriptionPlanner,
};
use fava_transport::{Transport, TransportDeadlines};
use fava_transport_testkit::{FakeRelay, FakeTransport};
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId};
use fava_write_store_memory::MemoryWriteStore;

/// Complete owner assembly plus the scripted providers it was given.
pub(crate) struct Assembly {
    pub observer: Observer,
    pub transport: Arc<FakeTransport>,
    pub planner: Arc<RecordingPlanner>,
    pub diagnostics: Arc<Diagnostics>,
    pub cache: Arc<MemoryEventCache>,
    pub runtime: Runtime,
}

/// Assemble the observation owner over the neutral transport fake.
#[must_use]
pub fn assemble() -> Assembly {
    assemble_with(Vec::new())
}

/// Assemble the owner with an ordered automatic router chain.
#[must_use]
pub fn assemble_with(routers: Vec<Arc<dyn Router>>) -> Assembly {
    let cache = Arc::new(MemoryEventCache::default());
    let writes = Arc::new(MemoryWriteStore::default());
    let transport = Arc::new(FakeTransport::new());
    let planner = Arc::new(RecordingPlanner::default());
    let diagnostics = Arc::new(Diagnostics::default());
    let runtime = Runtime::new(RuntimeConfig {
        default_channel_depth: nonzero(1_024),
        max_tasks: nonzero(65_536),
        max_provider_operations: nonzero(4_096),
    });
    let event_source: Arc<dyn QuerySource> = cache.clone();
    let write_source: Arc<dyn QuerySource> = writes;
    let evaluator: Arc<dyn QueryEvaluator> = Arc::new(StandardQueryEvaluator);
    let observer = Observer::new(event_source, write_source, evaluator)
        .with_transport(Arc::clone(&transport) as Arc<dyn Transport>)
        .with_subscription_planner(Arc::clone(&planner) as Arc<dyn SubscriptionPlanner>)
        .with_event_cache(Arc::clone(&cache) as Arc<dyn EventCache>)
        .with_diagnostics(Arc::clone(&diagnostics))
        .with_routers(routers)
        .with_runtime(runtime.clone())
        .with_deadlines(TransportDeadlines {
            establish: Duration::from_millis(200),
            write: Duration::from_millis(200),
            idle: Duration::from_secs(60),
            close: Duration::from_millis(200),
        });
    Assembly {
        observer,
        transport,
        planner,
        diagnostics,
        cache,
        runtime,
    }
}

impl Assembly {
    /// Rebuild the owner over a replacement local source and evaluator.
    #[must_use]
    pub fn with_local(
        &self,
        event_source: Arc<dyn QuerySource>,
        evaluator: Arc<dyn QueryEvaluator>,
    ) -> Observer {
        Observer::new(
            event_source,
            Arc::new(MemoryWriteStore::default()),
            evaluator,
        )
        .with_transport(Arc::clone(&self.transport) as Arc<dyn Transport>)
        .with_subscription_planner(Arc::clone(&self.planner) as Arc<dyn SubscriptionPlanner>)
        .with_event_cache(Arc::clone(&self.cache) as Arc<dyn EventCache>)
        .with_diagnostics(Arc::clone(&self.diagnostics))
        .with_runtime(self.runtime.clone())
    }

    /// The session the fake registered for one relay, if it dialed one.
    #[must_use]
    pub fn peer(&self, relay: &RelayUrl) -> Option<FakeRelay> {
        self.transport.relay(&session_key(relay))
    }

    /// The session the fake registered for one relay.
    ///
    /// # Panics
    ///
    /// If no session was established for `relay`.
    #[must_use]
    pub fn established(&self, relay: &RelayUrl) -> FakeRelay {
        self.peer(relay).expect("the relay session established")
    }
}

/// Extract the refusal from an open that must not produce a handle.
#[must_use]
pub fn refusal(result: Result<Observation, ObserveError>) -> ObserveError {
    match result {
        Ok(_) => panic!("open must refuse"),
        Err(error) => error,
    }
}

/// One relay URL under the standard test naming.
#[must_use]
pub fn relay(name: &str) -> RelayUrl {
    RelayUrl::parse(&format!("wss://{name}.example")).expect("relay URL")
}

/// The public-access session key for one relay.
#[must_use]
pub fn session_key(relay: &RelayUrl) -> RelaySessionKey {
    RelaySessionKey::new(relay.clone(), RelayAccess::public())
}

/// Evidence one observation currently reports for one relay.
#[must_use]
pub fn evidence(observation: &Observation, relay: &RelayUrl) -> Option<RelayQueryEvidence> {
    observation
        .current()
        .evidence
        .relay(&session_key(relay))
        .cloned()
}

/// Evidence one observation currently reports for one relay.
///
/// # Panics
///
/// If the observation reports no evidence for `relay`.
#[must_use]
pub fn relay_evidence(observation: &Observation, relay: &RelayUrl) -> RelayQueryEvidence {
    evidence(observation, relay)
        .expect("the observation reports evidence for every relay it uses")
}

/// Deliver one relay message to every consumer of a session.
pub fn push(peer: &FakeRelay, message: &RelayMessage<'_>) {
    peer.push_frame(
        serde_json::to_string(message)
            .expect("message encodes")
            .into_bytes(),
    );
}

/// Wire subscriptions one session was asked to open, in order.
#[must_use]
pub fn requests(peer: Option<FakeRelay>) -> Vec<(SubscriptionId, Vec<nostr::filter::Filter>)> {
    client_messages(peer)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Req {
                subscription_id,
                filters,
            } => Some((
                subscription_id.into_owned(),
                filters
                    .into_iter()
                    .map(std::borrow::Cow::into_owned)
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// Wire subscriptions one session was asked to withdraw, in order.
#[must_use]
pub fn withdrawals(peer: Option<FakeRelay>) -> Vec<SubscriptionId> {
    client_messages(peer)
        .into_iter()
        .filter_map(|message| match message {
            ClientMessage::Close(id) => Some(id.into_owned()),
            _ => None,
        })
        .collect()
}

fn client_messages(peer: Option<FakeRelay>) -> Vec<ClientMessage<'static>> {
    peer.map(|peer| peer.delivered_frames())
        .unwrap_or_default()
        .into_iter()
        .map(|frame| {
            serde_json::from_slice::<ClientMessage<'static>>(&frame)
                .expect("client message decodes")
        })
        .collect()
}

/// One wire subscription per logical demand, recording every planner input.
#[derive(Default)]
pub(crate) struct RecordingPlanner {
    inputs: Mutex<Vec<(RelaySessionKey, Vec<RelayDemand>)>>,
}

impl RecordingPlanner {
    /// Every `plan` call this planner received, in order.
    #[must_use]
    pub fn inputs(&self) -> Vec<(RelaySessionKey, Vec<RelayDemand>)> {
        self.inputs.lock().expect("planner lock").clone()
    }

    /// The largest demand set the owner ever handed this planner for `relay`.
    #[must_use]
    pub fn widest(&self, relay: &RelaySessionKey) -> Vec<RelayDemand> {
        self.inputs()
            .into_iter()
            .filter(|(key, _)| key == relay)
            .map(|(_, demand)| demand)
            .max_by_key(Vec::len)
            .unwrap_or_default()
    }
}

impl SubscriptionPlanner for RecordingPlanner {
    fn plan(
        &self,
        relay: &RelaySessionKey,
        demand: &[RelayDemand],
        constraints: &RelayReadConstraints,
        installed: &InstalledSubscriptions,
        revision: PlanRevision,
    ) -> Result<SubscriptionPlan, SubscriptionPlanError> {
        self.inputs
            .lock()
            .expect("planner lock")
            .push((relay.clone(), demand.to_vec()));
        fava_subscriptions_no_grouping::planner().plan(
            relay,
            demand,
            constraints,
            installed,
            revision,
        )
    }
}

/// Observations one relay's evidence says share the wire work behind it.
#[must_use]
pub fn shared_with(observation: &Observation, relay: &RelayUrl) -> Vec<ObservationId> {
    relay_evidence(observation, relay).shared_with
}

/// Await a deterministic condition without advancing wall time past a bound.
pub async fn wait_until(predicate: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("condition deadline elapsed");
}

/// Let every owner-held task reach quiescence.
pub async fn settle() {
    for _ in 0..256 {
        tokio::task::yield_now().await;
    }
}

fn nonzero(value: usize) -> std::num::NonZeroUsize {
    std::num::NonZeroUsize::new(value).expect("constant is non-zero")
}
