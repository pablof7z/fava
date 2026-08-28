//! Real-relay M4 routing scenarios through the public Fava facade.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{Fava, Observation, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_query_standard::StandardQueryEvaluator;
use fava_relay::{RelayAccess, RelaySessionKey};
use fava_router_app_relays::AppRelayRouter;
use fava_router_fallback_relays::FallbackRelayRouter;
use fava_router_testkit::DelayedRouter;
use fava_routing::{CoverageState, RouteContribution, RouteDestination, RouteTarget};
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind};
use nostr::types::{RelayUrl, Timestamp};
use serde_json::json;

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::{ProcessFact, RelayProcess, RelaySupervisor};
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, command_output, repository_root,
    reserve_port, wire,
};

/// Run one M4 automatic or explicit routing scenario against real relays.
///
/// # Errors
///
/// Returns an error when routing, relay execution, wire evidence, or evidence
/// persistence fails.
pub async fn run_routing_scenario(id: &str, options: SmokeOptions) -> CanaryResult<PathBuf> {
    if !matches!(
        id,
        "async-route-partial-read" | "explicit-route-bypass" | "fallback-reacts"
    ) {
        return Err(CanaryError::new(format!("unknown M4 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {id}"))?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": id, "seed": options.seed }),
    )?;
    let started = unix_ms()?;
    let count = match id {
        "explicit-route-bypass" => 1,
        "async-route-partial-read" => 2,
        "fallback-reacts" => 3,
        _ => unreachable!("validated scenario"),
    };
    let (version, mut relays, mut processes) =
        start_relays(&mut artifacts, &options, count).await?;
    let result = match id {
        "async-route-partial-read" => partial_read(&options.seed, &relays).await,
        "explicit-route-bypass" => explicit_bypass(&options.seed, &relays).await,
        "fallback-reacts" => fallback_reacts(&options.seed, &relays).await,
        _ => unreachable!("validated scenario"),
    };
    for relay in relays.drain(..) {
        processes.push(relay.stop().await?);
    }
    let completed = match result {
        Ok(completed) => completed,
        Err(error) => {
            artifacts.append_app_stderr(&error.to_string())?;
            artifacts.record(
                "scenario_failed",
                json!({ "scenario": id, "error": error.to_string() }),
            )?;
            return Err(error);
        }
    };
    finish(
        artifacts, id, &options, started, &version, &processes, &completed,
    )
}

struct LabRelay {
    process: RelayProcess,
    proxy: WireProxy,
    url: String,
    log: PathBuf,
}

impl LabRelay {
    async fn stop(self) -> CanaryResult<ProcessFact> {
        let fact = self.process.graceful_stop().await?;
        self.proxy.shutdown().await?;
        Ok(fact)
    }
}

struct Completed {
    event_ids: Vec<EventId>,
    summary: String,
}

async fn start_relays(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
    count: usize,
) -> CanaryResult<(String, Vec<LabRelay>, Vec<ProcessFact>)> {
    let mut version = None;
    let mut relays = Vec::new();
    let mut processes = Vec::new();
    for index in 0..count {
        let directory = artifacts
            .root()
            .join(format!("relays/nostr-rs-relay-{index}"));
        let supervisor =
            RelaySupervisor::prepare(&options.relay_binary, &directory, reserve_port().await?)?;
        version.get_or_insert(supervisor.version().await?);
        let process = supervisor.spawn(1).await?;
        processes.push(process.fact("ready"));
        artifacts.record("relay_ready", process.fact("ready"))?;
        let log = artifacts.root().join(format!("wire/proxy-{index}.jsonl"));
        let proxy = WireProxy::start(supervisor.address(), &log).await?;
        relays.push(LabRelay {
            url: proxy.url(),
            process,
            proxy,
            log,
        });
    }
    Ok((
        version.expect("at least one relay has a version"),
        relays,
        processes,
    ))
}

async fn partial_read(seed: &str, relays: &[LabRelay]) -> CanaryResult<Completed> {
    let events = seed_events(seed, relays).await?;
    let urls = parse_urls(relays)?;
    let delayed = Arc::new(DelayedRouter::new("delayed", RouteContribution::default()));
    let fava = assembly()
        .router(Arc::new(AppRelayRouter::new(
            "app-relays",
            [urls[0].clone()],
        )))
        .router(Arc::clone(&delayed))
        .build()
        .map_err(error)?;
    let mut observation = open(&fava, Query::events()).await?;
    wait_wire(&relays[0].log, "REQ", 1).await?;
    wait_events(&mut observation, 1).await?;
    if wire_count(&relays[1].log, "REQ")? != 0 {
        return Err(CanaryError::new(
            "delayed relay started before its contribution",
        ));
    }
    delayed.replace(contribution(&urls[1]));
    wait_wire(&relays[1].log, "REQ", 1).await?;
    wait_events(&mut observation, 2).await?;
    if wire_count(&relays[0].log, "REQ")? != 1 {
        return Err(CanaryError::new("unchanged app relay was restarted"));
    }
    observation.close();
    wait_wire(&relays[0].log, "CLOSE", 1).await?;
    wait_wire(&relays[1].log, "CLOSE", 1).await?;
    Ok(Completed {
        event_ids: events.into_iter().map(|event| event.id).collect(),
        summary: "app relay began immediately; delayed relay joined without restarting it"
            .to_owned(),
    })
}

async fn explicit_bypass(seed: &str, relays: &[LabRelay]) -> CanaryResult<Completed> {
    let events = seed_events(seed, relays).await?;
    let relay = RelayUrl::parse(&relays[0].url).map_err(error)?;
    let delayed = Arc::new(DelayedRouter::new("must-not-open", contribution(&relay)));
    let fava = assembly()
        .router(Arc::clone(&delayed))
        .build()
        .map_err(error)?;
    let mut observation = fava
        .observe(Query::events().from_relays([relay]).map_err(error)?)
        .await
        .map_err(error)?;
    wait_events(&mut observation, 1).await?;
    if delayed.open_count() != 0 || !fava.diagnostics().relays.is_empty() {
        return Err(CanaryError::new("explicit query opened automatic routing"));
    }
    observation.close();
    wait_wire(&relays[0].log, "CLOSE", 1).await?;
    Ok(Completed {
        event_ids: vec![events[0].id],
        summary: "explicit relay opened with zero automatic router sessions".to_owned(),
    })
}

async fn fallback_reacts(seed: &str, relays: &[LabRelay]) -> CanaryResult<Completed> {
    let events = seed_events(seed, relays).await?;
    let urls = parse_urls(relays)?;
    let delayed = Arc::new(DelayedRouter::new(
        "later-coverage",
        RouteContribution::default(),
    ));
    let fava = assembly()
        .router(Arc::new(AppRelayRouter::new(
            "app-relays",
            [urls[0].clone()],
        )))
        .router(Arc::clone(&delayed))
        .router(Arc::new(FallbackRelayRouter::new(
            "fallback",
            [urls[2].clone()],
            NonZeroUsize::new(2).expect("non-zero"),
        )))
        .build()
        .map_err(error)?;
    let mut observation = open(&fava, Query::events()).await?;
    wait_wire(&relays[0].log, "REQ", 1).await?;
    wait_wire(&relays[2].log, "REQ", 1).await?;
    delayed.replace(contribution(&urls[1]));
    wait_wire(&relays[1].log, "REQ", 1).await?;
    wait_wire(&relays[2].log, "CLOSE", 1).await?;
    if wire_count(&relays[0].log, "REQ")? != 1 || wire_count(&relays[0].log, "CLOSE")? != 0 {
        return Err(CanaryError::new("unrelated app relay was interrupted"));
    }
    wait_events(&mut observation, 3).await?;
    observation.close();
    wait_wire(&relays[0].log, "CLOSE", 1).await?;
    wait_wire(&relays[1].log, "CLOSE", 1).await?;
    Ok(Completed {
        event_ids: events.into_iter().map(|event| event.id).collect(),
        summary: "fallback withdrew after adequate upstream coverage; app relay stayed live"
            .to_owned(),
    })
}

fn assembly() -> fava::FavaBuilder {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
}

fn contribution(relay: &RelayUrl) -> RouteContribution {
    let target = RouteTarget::WholeRequest;
    let session = RelaySessionKey {
        relay: relay.clone(),
        access: RelayAccess::Public,
    };
    RouteContribution {
        destinations: vec![RouteDestination::new(
            session.clone(),
            BTreeSet::from([target.clone()]),
            "delayed test coverage",
        )],
        coverage: BTreeMap::from([(target, CoverageState::Covered(BTreeSet::from([session])))]),
        unresolved: BTreeSet::new(),
        shortfalls: Vec::new(),
    }
}

async fn seed_events(seed: &str, relays: &[LabRelay]) -> CanaryResult<Vec<Event>> {
    let mut events = Vec::new();
    for (index, relay) in relays.iter().enumerate() {
        let event = event(seed, index)?;
        wire::publish(&relay.url, &event).await?;
        events.push(event);
    }
    Ok(events)
}

fn event(seed: &str, index: usize) -> CanaryResult<Event> {
    let keys = crate::deterministic_keys(&format!("routing\0{seed}\0{index}"))?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(
        EventBuilder::new(Kind::TextNote, format!("Fava M4 routing {seed} {index}"))
            .custom_created_at(Timestamp::from(now.saturating_add(index as u64)))
            .finalize(&keys)?,
    )
}

fn parse_urls(relays: &[LabRelay]) -> CanaryResult<Vec<RelayUrl>> {
    relays
        .iter()
        .map(|relay| RelayUrl::parse(&relay.url).map_err(error))
        .collect()
}

async fn wait_events(observation: &mut Observation, count: usize) -> CanaryResult<()> {
    observation
        .wait_until(Duration::from_secs(5), |snapshot| snapshot.events.len() >= count)
        .await
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("routing observation deadline elapsed"))?;
    Ok(())
}

async fn open(fava: &Fava, query: Query) -> CanaryResult<Observation> {
    tokio::time::timeout(Duration::from_secs(5), fava.observe(query))
        .await
        .map_err(|_| CanaryError::new("automatic query open deadline elapsed"))?
        .map_err(error)
}

async fn wait_wire(path: &Path, message: &str, count: usize) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if wire_count(path, message)? >= count {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .map_err(|_| CanaryError::new(format!("wire {message} deadline elapsed")))?
}

fn wire_count(path: &Path, message: &str) -> CanaryResult<usize> {
    Ok(fs::read_to_string(path)?
        .lines()
        .filter(|line| {
            line.contains("client_to_relay") && line.contains(&format!("\\\"{message}\\\""))
        })
        .count())
}

fn finish(
    mut artifacts: RunArtifacts,
    scenario: &str,
    options: &SmokeOptions,
    started: u128,
    version: &str,
    processes: &[ProcessFact],
    completed: &Completed,
) -> CanaryResult<PathBuf> {
    artifacts.record(
        "scenario_passed",
        json!({ "scenario": scenario, "event_ids": completed.event_ids.iter().map(EventId::to_hex).collect::<Vec<_>>(), "summary": completed.summary }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {version}\n- Evidence: {}\n",
        completed.summary
    ))?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": artifacts.run_id()?, "scenario": scenario, "scenario_seed": options.seed,
            "selected_profile": "nostr-rs-relay-0.8.12-local-process", "fava_revision": revision,
            "canary_revision": revision, "dirty": dirty, "relay_implementation": "nostr-rs-relay",
            "relay_version": version, "started_unix_ms": started, "ended_unix_ms": unix_ms()?,
            "artifact_sha256": hashes,
        }),
    )?;
    artifacts.append_app_stdout(&format!("passed {scenario}"))?;
    Ok(artifacts.root().to_owned())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
