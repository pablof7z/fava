//! Real-relay M2 scenarios driven through the public Fava facade.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{DiagnosticsSnapshot, Fava, Observation, Query, RelayUrl, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::message::SubscriptionId;
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::{ProcessFact, RelaySupervisor};
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, command_output, repository_root,
    reserve_port, wire,
};

struct ScenarioResult {
    event_id: Option<EventId>,
    diagnostics: DiagnosticsSnapshot,
    subscription: SubscriptionId,
}

struct CompletedLive {
    relay_version: String,
    proxy_url: String,
    processes: Vec<ProcessFact>,
    result: ScenarioResult,
}

/// Run one M2 scenario against a disposable third-party relay and wire proxy.
///
/// # Errors
///
/// Returns an error when relay setup, public Fava behavior, independent wire
/// evidence, cleanup, or evidence persistence fails.
pub async fn run_live_scenario(id: &str, options: SmokeOptions) -> CanaryResult<PathBuf> {
    if !matches!(
        id,
        "explicit-read-eose" | "explicit-read-live-after-eose" | "explicit-read-cancel"
    ) {
        return Err(CanaryError::new(format!("unknown M2 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {id}"))?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": id, "seed": options.seed }),
    )?;
    let started_at = unix_ms()?;
    let relay_port = reserve_port().await?;
    let supervisor =
        RelaySupervisor::prepare(&options.relay_binary, &artifacts.relay_dir(), relay_port)?;
    let relay_version = supervisor.version().await?;
    let process = supervisor.spawn(1).await?;
    let mut processes = vec![process.fact("ready")];
    artifacts.record("relay_ready", process.fact("ready"))?;
    artifacts.record_resource(process.pid(), 1)?;
    let proxy = WireProxy::start(supervisor.address(), &artifacts.wire_log()).await?;
    let proxy_url = proxy.url();

    let scenario_result = execute(id, &options.seed, &proxy_url).await;
    let stopped = process.graceful_stop().await;
    if let Ok(fact) = stopped.as_ref() {
        processes.push(*fact);
        artifacts.record("relay_gracefully_stopped", fact)?;
    }
    let proxy_result = proxy.shutdown().await;

    let result = match scenario_result {
        Ok(result) => result,
        Err(error) => {
            record_failure(&mut artifacts, id, &error)?;
            let _ = stopped;
            let _ = proxy_result;
            return Err(error);
        }
    };
    stopped?;
    proxy_result?;
    verify_wire(
        &artifacts.wire_log(),
        &result.subscription,
        id != "explicit-read-cancel",
    )?;
    finish(
        artifacts,
        id,
        &options,
        started_at,
        &CompletedLive {
            relay_version,
            proxy_url,
            processes,
            result,
        },
    )
}

async fn execute(id: &str, seed: &str, proxy_url: &str) -> CanaryResult<ScenarioResult> {
    if id == "explicit-read-eose" {
        crate::hostile::refuse_forged_event(seed).await?;
    }
    let keys = crate::deterministic_keys(&format!("{id}\0{seed}"))?;
    let relay = RelayUrl::parse(proxy_url).map_err(error)?;
    let cache = Arc::new(MemoryEventCache::default());
    let fava = Fava::builder()
        .event_cache(Arc::clone(&cache))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .build()
        .map_err(error)?;
    let initial = event(&keys, seed, 0)?;
    if id != "explicit-read-cancel" {
        wire::publish(proxy_url, &initial).await?;
    }
    let query = Query::events()
        .authors([keys.public_key()])
        .kind(Kind::TextNote)
        .only_from_relays([relay.clone()])
        .map_err(error)?;
    let mut observation = fava.observe(query).await.map_err(error)?;
    let (session, generation, subscription) = wait_subscription(&fava).await?;
    if session.relay != relay {
        return Err(CanaryError::new("diagnostics named the wrong relay"));
    }

    let event_id = match id {
        "explicit-read-eose" => {
            wait_event(&mut observation, initial.id, 1).await?;
            wait_eose(&fava, &subscription).await?;
            Some(initial.id)
        }
        "explicit-read-live-after-eose" => {
            wait_event(&mut observation, initial.id, 1).await?;
            wait_eose(&fava, &subscription).await?;
            let live = event(&keys, seed, 1)?;
            wire::publish(proxy_url, &live).await?;
            wait_event(&mut observation, live.id, 2).await?;
            Some(live.id)
        }
        "explicit-read-cancel" => {
            observation.close();
            observation.close();
            wait_withdrawal(&fava, &subscription).await?;
            let late = event(&keys, seed, 2)?;
            wire::publish(proxy_url, &late).await?;
            if observation.changed().await.is_ok() {
                return Err(CanaryError::new(
                    "closed observation delivered a later application update",
                ));
            }
            if !cache.is_empty().map_err(error)? {
                return Err(CanaryError::new(
                    "cancelled subscription admitted a later relay event",
                ));
            }
            Some(late.id)
        }
        _ => unreachable!("validated scenario"),
    };
    observation.close();
    wait_withdrawal(&fava, &subscription).await?;
    let diagnostics = fava.diagnostics();
    if diagnostics
        .relays
        .iter()
        .any(|entry| entry.session == session && entry.generation == generation)
    {
        return Err(CanaryError::new(
            "the owner still holds a relay session no observation demands",
        ));
    }
    Ok(ScenarioResult {
        event_id,
        diagnostics,
        subscription,
    })
}

/// The first wire subscription the observation owner reports as installed.
async fn wait_subscription(
    fava: &Fava,
) -> CanaryResult<(
    fava_relay::RelaySessionKey,
    fava::OperationGeneration,
    SubscriptionId,
)> {
    wait(Duration::from_secs(5), || {
        fava.diagnostics().relays.iter().find_map(|relay| {
            relay
                .subscriptions
                .first()
                .map(|wire| (relay.session.clone(), relay.generation, wire.id.clone()))
        })
    })
    .await
}

/// The owner records EOSE on the exact wire subscription it installed.
async fn wait_eose(fava: &Fava, subscription: &SubscriptionId) -> CanaryResult<()> {
    wait(Duration::from_secs(5), || {
        fava.diagnostics()
            .relays
            .iter()
            .flat_map(|relay| relay.subscriptions.iter())
            .any(|wire| &wire.id == subscription && wire.stored_events_complete)
            .then_some(())
    })
    .await
}

/// The subscription leaves the owner's installed set when its last demand goes.
async fn wait_withdrawal(fava: &Fava, subscription: &SubscriptionId) -> CanaryResult<()> {
    wait(Duration::from_secs(5), || {
        (!fava
            .diagnostics()
            .relays
            .iter()
            .flat_map(|relay| relay.subscriptions.iter())
            .any(|wire| &wire.id == subscription))
        .then_some(())
    })
    .await
}

async fn wait_event(
    observation: &mut Observation,
    event_id: EventId,
    minimum_count: usize,
) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let current = observation.current();
            if current.events.len() >= minimum_count
                && current.events.iter().any(|record| record.id() == event_id)
                && current.events.iter().all(|record| {
                    record
                        .relay_occurrences()
                        .occurrences()
                        .all(|evidence| !evidence.session.relay.as_str().is_empty())
                })
            {
                return Ok(());
            }
            observation.changed().await.map_err(error)?;
        }
    })
    .await
    .map_err(|_| CanaryError::new("application event deadline elapsed"))?
}

async fn wait<T>(duration: Duration, mut value: impl FnMut() -> Option<T>) -> CanaryResult<T> {
    tokio::time::timeout(duration, async {
        loop {
            if let Some(value) = value() {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("diagnostic fact deadline elapsed"))
}

fn event(keys: &Keys, seed: &str, offset: u64) -> CanaryResult<Event> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(
        EventBuilder::new(Kind::TextNote, format!("Fava M2 {seed} {offset}"))
            .custom_created_at(Timestamp::from(now.saturating_add(offset)))
            .finalize(keys)?,
    )
}

fn verify_wire(path: &Path, subscription: &SubscriptionId, require_eose: bool) -> CanaryResult<()> {
    let mut req_connections = BTreeSet::new();
    let mut close_connections = BTreeSet::new();
    let mut saw_eose = false;
    for line in fs::read_to_string(path)?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        let Some(payload) = entry.get("payload").and_then(Value::as_str) else {
            continue;
        };
        let Ok(frame) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if frame.get(1).and_then(Value::as_str) != Some(subscription.as_str()) {
            continue;
        }
        let connection = entry
            .get("connection")
            .and_then(Value::as_u64)
            .ok_or_else(|| CanaryError::new("wire entry omitted connection identity"))?;
        match (
            entry.get("direction").and_then(Value::as_str),
            frame.get(0).and_then(Value::as_str),
        ) {
            (Some("client_to_relay"), Some("REQ")) => {
                req_connections.insert(connection);
            }
            (Some("client_to_relay"), Some("CLOSE")) => {
                close_connections.insert(connection);
            }
            (Some("relay_to_client"), Some("EOSE")) => saw_eose = true,
            _ => {}
        }
    }
    if req_connections.len() != 1 || req_connections != close_connections {
        return Err(CanaryError::new(format!(
            "wire REQ/CLOSE session mismatch: req={req_connections:?}, close={close_connections:?}"
        )));
    }
    if require_eose && !saw_eose {
        return Err(CanaryError::new("wire proxy did not witness exact EOSE"));
    }
    Ok(())
}

fn finish(
    mut artifacts: RunArtifacts,
    scenario: &str,
    options: &SmokeOptions,
    started_at: u128,
    completed: &CompletedLive,
) -> CanaryResult<PathBuf> {
    let result = &completed.result;
    artifacts.record(
        "scenario_passed",
        json!({
            "scenario": scenario,
            "event_id": result.event_id.map(|id| id.to_hex()),
            "subscription": result.subscription.as_str(),
            "relay_sessions_held": result.diagnostics.relays.len(),
            "open_observations": result.diagnostics.queries.len(),
        }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &completed.processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {}\n- Proxy: {}\n- Subscription: {}\n- Exact REQ/CLOSE session: verified\n",
        completed.relay_version,
        completed.proxy_url,
        result.subscription
    ))?;
    let run_id = artifacts.run_id()?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let ended_at = unix_ms()?;
    let hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": run_id,
            "scenario": scenario,
            "scenario_seed": options.seed,
            "selected_profile": "nostr-rs-relay-0.8.12-local-process",
            "fava_revision": revision,
            "canary_revision": revision,
            "dirty": dirty,
            "relay_implementation": "nostr-rs-relay",
            "relay_version": completed.relay_version,
            "proxy_url": completed.proxy_url,
            "started_unix_ms": started_at,
            "ended_unix_ms": ended_at,
            "artifact_sha256": hashes,
        }),
    )?;
    artifacts.append_app_stdout(&format!("passed {scenario}"))?;
    Ok(artifacts.root().to_owned())
}

fn record_failure(
    artifacts: &mut RunArtifacts,
    scenario: &str,
    error: &CanaryError,
) -> CanaryResult<()> {
    artifacts.append_app_stderr(&error.to_string())?;
    artifacts.record(
        "scenario_failed",
        json!({ "scenario": scenario, "error": error.to_string() }),
    )?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: failed\n- Error: {error}\n"
    ))
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
