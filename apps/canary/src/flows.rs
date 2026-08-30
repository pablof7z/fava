//! Ten ordinary-application flows driven through the public Fava facade.
//!
//! This module is the canary's consumer surface. It may depend only on the
//! `fava` facade and on provider crates an application would select. It must
//! never reach into a Fava internal crate, never install a stub transport,
//! publisher, or write store, never construct a second engine to feed the
//! first, and never hand-feed data the library should have acquired.
//!
//! When a flow cannot be written that way, the flow is recorded as a wall and
//! left failing. A wall is the deliverable, not a defect in this file.

use std::fmt::Write as _;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use fava::{
    EventBuilder, Fava, Kind, Observation, PublicKey, Query, RelaySessionState, RelayUrl,
    all_acknowledged, all_terminal,
};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_nip02::Nip02;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_router_app_relays::AppRelayRouter;
use fava_signer_local::LocalSigner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::key::Keys;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::artifacts::RunArtifacts;
use crate::proxy::WireProxy;
use crate::{CanaryError, CanaryResult, deterministic_keys, wire};

/// A relay address that accepts TCP but is not routable: connections hang.
const BLACKHOLE_RELAY: &str = "ws://192.0.2.1:8080";

/// Every flow gets this much wall-clock budget before it is called frozen.
const RESPONSIVE_BUDGET: Duration = Duration::from_secs(5);

/// Inputs for the consumer flow suite.
#[derive(Clone, Debug)]
pub struct FlowOptions {
    /// A reachable real relay, for example `ws://127.0.0.1:7447`.
    pub relay_url: String,
    /// Caller-selected seed used to derive disposable identities.
    pub seed: String,
    /// Parent directory for preserved evidence bundles.
    pub runs_directory: PathBuf,
}

/// One flow result as an outside developer would report it.
#[derive(Clone, Debug)]
struct FlowRecord {
    id: &'static str,
    intent: &'static str,
    status: &'static str,
    severity: &'static str,
    conclusion: String,
    detail: Value,
}

impl FlowRecord {
    fn passed(id: &'static str, intent: &'static str, detail: Value) -> Self {
        Self {
            id,
            intent,
            status: "passed",
            severity: "none",
            conclusion: String::new(),
            detail,
        }
    }

    fn wall(
        id: &'static str,
        intent: &'static str,
        severity: &'static str,
        conclusion: impl Into<String>,
        detail: Value,
    ) -> Self {
        Self {
            id,
            intent,
            status: "wall",
            severity,
            conclusion: conclusion.into(),
            detail,
        }
    }

    fn json(&self) -> Value {
        json!({
            "flow": self.id,
            "intent": self.intent,
            "status": self.status,
            "severity": self.severity,
            "conclusion": self.conclusion,
            "detail": self.detail,
        })
    }
}

/// Run the ten consumer flows against one reachable real relay.
///
/// # Errors
///
/// Returns an error when evidence cannot be persisted, or when at least one
/// flow hit a wall. A wall is a reported defect, so the run exits nonzero.
pub async fn run_flows_scenario(options: FlowOptions) -> CanaryResult<PathBuf> {
    let mut artifacts = RunArtifacts::create(&options.runs_directory, "dx-flows", &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": "dx-flows", "relay": options.relay_url }),
    )?;

    let upstream = crate::relay_socket_address(&options.relay_url)?;
    let proxy = WireProxy::start(upstream, &artifacts.root().join("wire/flows.jsonl")).await?;
    let live = RelayUrl::parse(&proxy.url()).map_err(error)?;
    let unreachable =
        RelayUrl::parse(&format!("ws://127.0.0.1:{}", closed_port().await?)).map_err(error)?;
    let blackhole = RelayUrl::parse(BLACKHOLE_RELAY).map_err(error)?;

    let mut records = Vec::new();
    records.push(flow_01_engine_before_account(&live).await);
    records.push(flow_02_offline_local_view(&unreachable, &blackhole).await);
    records.push(flow_03_runtime_signer(&live, &options.seed));
    records.push(flow_04_two_accounts(&live, &options.seed).await);
    records.push(flow_05_profile_and_contacts(&live, &options.seed).await);
    records.push(flow_06_automatic_note(&live, &options.seed).await);
    records.push(flow_07_two_observations_one_connection(&live, &proxy).await);
    records.push(flow_08_mixed_relay_health(&live, &unreachable).await);
    records.push(flow_09_cancel_before_delivery(&unreachable, &options.seed));
    records.push(flow_10_clean_close(&options.relay_url).await);

    proxy.shutdown().await?;

    let table: Vec<Value> = records.iter().map(FlowRecord::json).collect();
    artifacts.write_json("flows.json", &json!({ "flows": table }))?;
    let mut report = String::from("# Consumer flow results\n\n");
    for record in &records {
        let note = if record.conclusion.is_empty() {
            String::new()
        } else {
            format!(" -- {}", record.conclusion)
        };
        let _ = writeln!(
            report,
            "- {} [{}] {}{note}",
            record.id, record.status, record.intent
        );
    }
    artifacts.write_report(&report)?;
    artifacts.record("scenario_finished", json!({ "flows": table }))?;

    let walls: Vec<&str> = records
        .iter()
        .filter(|record| record.status == "wall")
        .map(|record| record.id)
        .collect();
    if walls.is_empty() {
        Ok(artifacts.root().to_path_buf())
    } else {
        Err(CanaryError::new(format!(
            "consumer flows hit walls: {} (evidence: {})",
            walls.join(", "),
            artifacts.root().display()
        )))
    }
}

// ---------------------------------------------------------------------------
// Assembly an ordinary application writes once at start-up.
// ---------------------------------------------------------------------------

/// Assemble a read-only engine: no account exists yet, so no signer exists.
fn read_only_engine(relays: &[RelayUrl]) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner))
        .transport(Arc::new(WebSocketTransport::default()));
    if !relays.is_empty() {
        builder = builder.router(Arc::new(AppRelayRouter::new(
            "app-relays",
            relays.iter().cloned(),
        )));
    }
    builder.build().map_err(error)
}

/// Assemble a publishing engine for accounts already known at start-up.
///
/// Every signer must be named here. There is no later door.
fn publishing_engine(relays: &[RelayUrl], accounts: &[Keys]) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(StandardSubscriptionPlanner))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .with_nip02();
    if !relays.is_empty() {
        builder = builder.router(Arc::new(AppRelayRouter::new(
            "app-relays",
            relays.iter().cloned(),
        )));
    }
    for account in accounts {
        builder = builder.signer(Arc::new(LocalSigner::new(account.clone())));
    }
    builder.build().map_err(error)
}

include!("flows/flow_01_to_05.rs");

include!("flows/flow_06_to_10.rs");

// ---------------------------------------------------------------------------
// Shared helpers. None of these substitute for Fava behaviour.
// ---------------------------------------------------------------------------

async fn publish_note(engine: &Fava, author: PublicKey, label: &str) -> CanaryResult<String> {
    let note = EventBuilder::new(Kind::TextNote)
        .content(format!("Fava flow note {label}"))
        .by(author)
        .build()
        .map_err(error)?;
    publish_and_settle(engine, note).await
}

async fn publish_and_settle(engine: &Fava, note: fava::UnsignedEvent) -> CanaryResult<String> {
    settle(engine.publish(note)).await
}

async fn settle(write: Result<fava::Write, fava::PublishError>) -> CanaryResult<String> {
    let write = write.map_err(error)?;
    let receipt = tokio::time::timeout(Duration::from_secs(10), write.settled(all_acknowledged()))
        .await
        .map_err(|_| CanaryError::new("timed out awaiting relay acknowledgements"))?
        .map_err(error)?;
    Ok(receipt.current.id().to_hex())
}

/// Open an observation, wait for at least `expected` records, then close.
async fn read_back(
    engine: &Fava,
    query: Query,
    relay: &RelayUrl,
    expected: usize,
) -> CanaryResult<usize> {
    let query = query.from_relays([relay.clone()]).map_err(error)?;
    let mut observation = tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query))
        .await
        .map_err(|_| CanaryError::new("observe froze"))?
        .map_err(error)?;
    let count = wait_for(&mut observation, expected).await;
    observation.close();
    count
}

/// Read current local state for a query without creating relay demand.
async fn observe_local(engine: &Fava, query: Query) -> CanaryResult<fava::QuerySnapshot> {
    let observation = tokio::time::timeout(RESPONSIVE_BUDGET, engine.observe(query.cache_only()))
        .await
        .map_err(|_| CanaryError::new("cache-only observe froze"))?
        .map_err(error)?;
    let snapshot = observation.current();
    observation.close();
    Ok((*snapshot).clone())
}

async fn wait_for(observation: &mut Observation, expected: usize) -> CanaryResult<usize> {
    let snapshot = observation
        .wait_until(Duration::from_secs(10), |snapshot| {
            snapshot.events.len() >= expected
        })
        .await
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("flow observation deadline elapsed"))?;
    Ok(snapshot.events.len())
}

async fn closed_port() -> CanaryResult<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
