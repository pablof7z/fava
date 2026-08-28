//! Real-relay M3 multi-relay and reconnect scenarios through the public facade.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{Fava, Observation, Query, RelayUrl, Timestamp};
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind};
use nostr::message::{RelayMessage, SubscriptionId};
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::{ProcessFact, RelaySupervisor};
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, command_output, repository_root,
    reserve_port, wire,
};

/// Run one M3 relay scenario against disposable third-party relay processes.
///
/// # Errors
///
/// Returns an error when relay setup, public Fava behavior, independent wire
/// evidence, or evidence persistence fails.
pub async fn run_m3_live_scenario(id: &str, options: SmokeOptions) -> CanaryResult<PathBuf> {
    if !matches!(id, "multi-relay-dedup-provenance" | "reconnect-generation") {
        return Err(CanaryError::new(format!("unknown M3 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {id}"))?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": id, "seed": options.seed }),
    )?;
    let started_at = unix_ms()?;
    let result = if id == "multi-relay-dedup-provenance" {
        multi_relay(&mut artifacts, &options).await
    } else {
        reconnect(&mut artifacts, &options).await
    };
    match result {
        Ok(completed) => finish(artifacts, id, &options, started_at, &completed),
        Err(error) => {
            artifacts.append_app_stderr(&error.to_string())?;
            artifacts.record(
                "scenario_failed",
                json!({ "scenario": id, "error": error.to_string() }),
            )?;
            artifacts.write_report(&format!(
                "# Canary report\n\n- Scenario: {id}\n- Result: failed\n- Error: {error}\n"
            ))?;
            Err(error)
        }
    }
}

struct Completed {
    relay_version: String,
    proxy_urls: Vec<String>,
    processes: Vec<ProcessFact>,
    event_id: EventId,
    subscriptions: Vec<SubscriptionId>,
    summary: String,
}

async fn multi_relay(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
) -> CanaryResult<Completed> {
    let mut active = Vec::new();
    let mut proxy_urls = Vec::new();
    let mut relay_version = None;
    let mut processes = Vec::new();
    for index in 0..3 {
        let port = reserve_port().await?;
        let relay_dir = artifacts
            .root()
            .join(format!("relays/nostr-rs-relay-{index}"));
        let supervisor = RelaySupervisor::prepare(&options.relay_binary, &relay_dir, port)?;
        relay_version.get_or_insert(supervisor.version().await?);
        let process = supervisor.spawn(1).await?;
        processes.push(process.fact("ready"));
        artifacts.record("relay_ready", process.fact("ready"))?;
        artifacts.record_resource(process.pid(), 1)?;
        let proxy = WireProxy::start(
            supervisor.address(),
            &artifacts.root().join(format!("wire/proxy-{index}.jsonl")),
        )
        .await?;
        proxy_urls.push(proxy.url());
        active.push((process, proxy));
    }

    let event = event(&options.seed, "multi")?;
    wire::publish(&proxy_urls[0], &event).await?;
    wire::publish(&proxy_urls[1], &event).await?;
    let relays = parse_relays(&proxy_urls)?;
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache));
    let mut observation = fava
        .observe(
            Query::events()
                .ids([event.id])
                .map_err(error)?
                .only_from_relays(relays.clone())
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    wait_events(&mut observation, event.id, 2).await?;
    wait(Duration::from_secs(5), || {
        (completed(&fava) == 3).then_some(())
    })
    .await?;
    let current = observation.current();
    if current.events.len() != 1 {
        return Err(CanaryError::new("same event was not deduplicated"));
    }
    let serving: BTreeSet<_> = current.events[0]
        .relay_occurrences()
        .occurrences()
        .map(|evidence| evidence.session.relay.clone())
        .collect();
    if serving.len() != 2
        || !serving.contains(&relays[0])
        || !serving.contains(&relays[1])
        || serving.contains(&relays[2])
    {
        return Err(CanaryError::new(format!(
            "actual serving relay evidence mismatch: {serving:?}"
        )));
    }
    let subscriptions: Vec<SubscriptionId> = installed(&fava).into_iter().collect();
    observation.close();
    wait(Duration::from_secs(5), || {
        installed(&fava).is_empty().then_some(())
    })
    .await?;

    for (process, proxy) in active {
        processes.push(process.graceful_stop().await?);
        proxy.shutdown().await?;
    }
    Ok(Completed {
        relay_version: relay_version.expect("three relays supplied a version"),
        proxy_urls,
        processes,
        event_id: event.id,
        subscriptions,
        summary: "one record; two actual serving relays; third relay not credited".to_owned(),
    })
}

async fn reconnect(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
) -> CanaryResult<Completed> {
    let port = reserve_port().await?;
    let supervisor = RelaySupervisor::prepare(&options.relay_binary, &artifacts.relay_dir(), port)?;
    let relay_version = supervisor.version().await?;
    let first = supervisor.spawn(1).await?;
    let mut processes = vec![first.fact("ready")];
    artifacts.record("relay_ready", first.fact("ready"))?;
    let proxy = WireProxy::start(supervisor.address(), &artifacts.wire_log()).await?;
    let proxy_url = proxy.url();
    let relay = RelayUrl::parse(&proxy_url).map_err(error)?;
    let cache = Arc::new(MemoryEventCache::default());
    let fava = assembly(Arc::clone(&cache));
    let mut observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .map_err(error)?
                .only_from_relays([relay.clone()])
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let (old_generation, old_subscription) = wait_subscription(&fava, None).await?;
    wait(Duration::from_secs(5), || {
        fava.diagnostics()
            .relays
            .iter()
            .flat_map(|relay| relay.subscriptions.iter())
            .any(|wire| wire.id == old_subscription && wire.stored_events_complete)
            .then_some(())
    })
    .await?;
    processes.push(first.hard_kill().await?);
    let second = supervisor.spawn(2).await?;
    processes.push(second.fact("ready"));
    artifacts.record("relay_restarted", second.fact("ready"))?;

    let (generation, subscription) = wait_subscription(&fava, Some(old_generation)).await?;
    if generation <= old_generation {
        return Err(CanaryError::new(
            "reconnect did not mint a fresh session generation",
        ));
    }
    if subscription == old_subscription {
        return Err(CanaryError::new(
            "reconnect reused the retired wire subscription id: a straggler frame for the \
             closed request can settle the fresh one (GOALS:426, QUERY-010)",
        ));
    }
    let event = event(&options.seed, "reconnect")?;
    let stale = serde_json::to_string(&RelayMessage::event(
        old_subscription.clone(),
        event.clone(),
    ))?;
    proxy.inject_relay_text(stale)?;
    wait(Duration::from_secs(5), || {
        cache.len().is_ok_and(|count| count > 0).then_some(())
    })
    .await
    .err()
    .map_or(Ok(()), |_| Ok::<(), CanaryError>(()))?;
    if !cache.is_empty().map_err(error)? {
        return Err(CanaryError::new(
            "stale subscription frame entered the event cache",
        ));
    }
    proxy.inject_relay_text(serde_json::to_string(&RelayMessage::event(
        subscription.clone(),
        event.clone(),
    ))?)?;
    wait_events(&mut observation, event.id, 1).await?;
    observation.close();
    wait(Duration::from_secs(5), || {
        (!installed(&fava).contains(&subscription)).then_some(())
    })
    .await?;
    processes.push(second.graceful_stop().await?);
    proxy.shutdown().await?;
    verify_reconnect_wire(&artifacts.wire_log(), &old_subscription, &subscription)?;
    Ok(Completed {
        relay_version,
        proxy_urls: vec![proxy_url],
        processes,
        event_id: event.id,
        subscriptions: vec![old_subscription, subscription],
        summary: "fresh reconnect identity; stale frame refused; current frame admitted".to_owned(),
    })
}

fn assembly(cache: Arc<MemoryEventCache>) -> Fava {
    Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .build()
        .expect("complete canary assembly")
}

fn parse_relays(urls: &[String]) -> CanaryResult<Vec<RelayUrl>> {
    urls.iter()
        .map(|url| RelayUrl::parse(url).map_err(error))
        .collect()
}

/// The wire subscription the owner currently has installed at any relay,
/// optionally waiting for a generation newer than one already seen.
async fn wait_subscription(
    fava: &Fava,
    after: Option<fava::OperationGeneration>,
) -> CanaryResult<(fava::OperationGeneration, SubscriptionId)> {
    wait(Duration::from_secs(10), || {
        fava.diagnostics().relays.iter().find_map(|relay| {
            let generation = relay.generation?;
            if after.is_some_and(|seen| generation <= seen) {
                return None;
            }
            relay
                .subscriptions
                .first()
                .map(|wire| (generation, wire.id.clone()))
        })
    })
    .await
}

/// Every wire subscription the observation owner currently has installed.
fn installed(fava: &Fava) -> BTreeSet<SubscriptionId> {
    fava.diagnostics()
        .relays
        .iter()
        .flat_map(|relay| relay.subscriptions.iter())
        .map(|wire| wire.id.clone())
        .collect()
}

/// Installed wire subscriptions the relay has finished its stored replay for.
fn completed(fava: &Fava) -> usize {
    fava.diagnostics()
        .relays
        .iter()
        .flat_map(|relay| relay.subscriptions.iter())
        .filter(|wire| wire.stored_events_complete)
        .count()
}

async fn wait_events(
    observation: &mut Observation,
    event_id: EventId,
    evidence_count: usize,
) -> CanaryResult<()> {
    observation
        .wait_until(Duration::from_secs(5), |snapshot| {
            snapshot.events.iter().any(|record| {
                record.id() == event_id && record.relay_occurrences().len() == evidence_count
            })
        })
        .await
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("multi-relay event observation deadline elapsed"))?;
    Ok(())
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

fn event(seed: &str, label: &str) -> CanaryResult<Event> {
    let keys = crate::deterministic_keys(&format!("{label}\0{seed}"))?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(
        EventBuilder::new(Kind::TextNote, format!("Fava M3 {label} {seed}"))
            .custom_created_at(Timestamp::from(now))
            .finalize(&keys)?,
    )
}

fn verify_reconnect_wire(
    path: &std::path::Path,
    old: &SubscriptionId,
    current: &SubscriptionId,
) -> CanaryResult<()> {
    let mut old_req = false;
    let mut current_req = false;
    let mut injected = 0;
    for line in fs::read_to_string(path)?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        let direction = entry.get("direction").and_then(Value::as_str);
        let payload = entry.get("payload").and_then(Value::as_str).unwrap_or("");
        old_req |= direction == Some("client_to_relay")
            && payload.contains("\"REQ\"")
            && payload.contains(old.as_str());
        current_req |= direction == Some("client_to_relay")
            && payload.contains("\"REQ\"")
            && payload.contains(current.as_str());
        if direction == Some("proxy_to_client") {
            injected += 1;
        }
    }
    if !old_req || !current_req || injected != 2 {
        return Err(CanaryError::new(format!(
            "wire reconnect witness mismatch: old={old_req}, current={current_req}, injected={injected}"
        )));
    }
    Ok(())
}

fn finish(
    mut artifacts: RunArtifacts,
    scenario: &str,
    options: &SmokeOptions,
    started_at: u128,
    completed: &Completed,
) -> CanaryResult<PathBuf> {
    artifacts.record(
        "scenario_passed",
        json!({
            "scenario": scenario,
            "event_id": completed.event_id.to_hex(),
            "subscriptions": completed.subscriptions.iter().map(SubscriptionId::as_str).collect::<Vec<_>>(),
            "summary": completed.summary,
        }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &completed.processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {}\n- Event: {}\n- Evidence: {}\n",
        completed.relay_version, completed.event_id, completed.summary
    ))?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let run_id = artifacts.run_id()?;
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
            "proxy_urls": completed.proxy_urls,
            "started_unix_ms": started_at,
            "ended_unix_ms": unix_ms()?,
            "artifact_sha256": hashes,
        }),
    )?;
    artifacts.append_app_stdout(&format!("passed {scenario}"))?;
    Ok(artifacts.root().to_owned())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
