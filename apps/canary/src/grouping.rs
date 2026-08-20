//! Real-relay M4 subscription-planner equivalence scenario.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fava::{Fava, Query};
use fava_event_cache_memory::MemoryEventCache;
use fava_ingest::admit_subscription_event;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions::{RelayDemand, SubscriptionPlan, SubscriptionPlanner};
use fava_subscriptions_no_grouping::planner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{HandoffOutcome, RelaySession, Transport};
use fava_transport_websocket::WebSocketTransport;
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, decode_relay, encode_client};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind};
use nostr::filter::Filter;
use nostr::key::{Keys, PublicKey};
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::RelaySupervisor;
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, command_output, repository_root,
    reserve_port, wire,
};

/// Compare standard grouping with one-REQ-per-demand against one real relay.
///
/// # Errors
///
/// Returns an error when wire shape, logical results, relay execution, or
/// evidence persistence differs.
pub async fn run_grouping_scenario(options: SmokeOptions) -> CanaryResult<PathBuf> {
    let scenario = "subscription-grouping-equivalence";
    let artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {scenario}"))?;
    let started = unix_ms()?;
    let supervisor = RelaySupervisor::prepare(
        &options.relay_binary,
        &artifacts.relay_dir(),
        reserve_port().await?,
    )?;
    let version = supervisor.version().await?;
    let process = supervisor.spawn(1).await?;
    let mut processes = vec![process.fact("ready")];
    let proxy = WireProxy::start(supervisor.address(), &artifacts.wire_log()).await?;
    let relay = RelayUrl::parse(&proxy.url()).map_err(error)?;
    let events = events(&options.seed)?;
    for event in &events {
        wire::publish(&proxy.url(), event).await?;
    }
    let demand = events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            RelayDemand::new(
                SubscriptionId::new(format!("logical-{index}")),
                Filter::new().author(event.pubkey).kind(Kind::TextNote),
            )
        })
        .collect::<Vec<_>>();
    let standard_cache = Arc::new(MemoryEventCache::default());
    let separate_cache = Arc::new(MemoryEventCache::default());
    let standard_messages = execute_plan(
        &relay,
        &demand,
        &StandardSubscriptionPlanner::default(),
        standard_cache.as_ref(),
    )
    .await?;
    let separate_messages =
        execute_plan(&relay, &demand, &planner(), separate_cache.as_ref()).await?;
    let authors: Vec<_> = events.iter().map(|event| event.pubkey).collect();
    let standard_results = visible(standard_cache, &authors).await?;
    let separate_results = visible(separate_cache, &authors).await?;
    if standard_results != separate_results
        || standard_results.iter().any(|events| events.len() != 1)
    {
        return Err(CanaryError::new(format!(
            "planner substitution changed logical results: grouped={standard_results:?}, separate={separate_results:?}"
        )));
    }
    if standard_messages != 1 || separate_messages != 3 {
        return Err(CanaryError::new(format!(
            "planner wire shape mismatch: grouped={standard_messages}, separate={separate_messages}"
        )));
    }
    verify_wire(&artifacts.wire_log())?;
    processes.push(process.graceful_stop().await?);
    proxy.shutdown().await?;
    finish(artifacts, &options, started, &version, &processes, &events)
}

async fn execute_plan(
    relay: &RelayUrl,
    demand: &[RelayDemand],
    planner: &dyn SubscriptionPlanner,
    cache: &MemoryEventCache,
) -> CanaryResult<usize> {
    let key = RelaySessionKey::new(relay.clone(), RelayAccess::public());
    let plan = planner.plan(&key, demand).map_err(error)?;
    let session = WebSocketTransport::default()
        .open_session(key)
        .await
        .map_err(error)?;
    for message in &plan.messages {
        let frame = encode_client(message).map_err(error)?;
        if session.send(frame).await != HandoffOutcome::HandedOff {
            return Err(CanaryError::new("planner REQ was not handed off"));
        }
    }
    read_until_eose(session.as_ref(), &plan, cache).await?;
    for id in plan.attribution.keys() {
        let frame = encode_client(&ClientMessage::close(id.clone())).map_err(error)?;
        let _ = session.send(frame).await;
    }
    session.close().await.map_err(error)?;
    Ok(plan.messages.len())
}

async fn read_until_eose(
    session: &dyn RelaySession,
    plan: &SubscriptionPlan,
    cache: &MemoryEventCache,
) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut complete = BTreeSet::new();
        while complete.len() < plan.attribution.len() {
            match decode_relay(&session.next_message().await.map_err(error)?).map_err(error)? {
                RelayMessage::Event {
                    subscription_id,
                    event,
                } => {
                    let id = subscription_id.into_owned();
                    let filter = plan
                        .attribution
                        .get(&id)
                        .ok_or_else(|| CanaryError::new("unattributed planner EVENT"))?;
                    admit_subscription_event(
                        cache,
                        session.key(),
                        &id,
                        &id,
                        filter,
                        event.into_owned(),
                        Timestamp::now(),
                    )
                    .map_err(error)?;
                }
                RelayMessage::EndOfStoredEvents(id) => {
                    let id = id.into_owned();
                    if plan.attribution.contains_key(&id) {
                        complete.insert(id);
                    }
                }
                RelayMessage::Closed { message, .. } => {
                    return Err(CanaryError::new(format!("relay CLOSED: {message}")));
                }
                RelayMessage::Auth { .. }
                | RelayMessage::Notice(_)
                | RelayMessage::Ok { .. }
                | RelayMessage::Count { .. }
                | RelayMessage::NegMsg { .. }
                | RelayMessage::NegErr { .. } => {}
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| CanaryError::new("planner EOSE deadline elapsed"))?
}

async fn visible(
    cache: Arc<MemoryEventCache>,
    authors: &[PublicKey],
) -> CanaryResult<Vec<BTreeSet<EventId>>> {
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .map_err(error)?;
    let mut results = Vec::new();
    for author in authors {
        let observation = fava
            .observe(Query::events().authors([*author]).cache_only())
            .await
            .map_err(error)?;
        results.push(
            observation
                .current()
                .events
                .iter()
                .map(fava::EventRecord::id)
                .collect(),
        );
    }
    Ok(results)
}

fn events(seed: &str) -> CanaryResult<Vec<Event>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    (0..3)
        .map(|index| {
            let keys = Keys::generate();
            EventBuilder::new(Kind::TextNote, format!("Fava M4 grouping {seed} {index}"))
                .custom_created_at(Timestamp::from(now.saturating_add(index)))
                .finalize(&keys)
                .map_err(error)
        })
        .collect()
}

fn verify_wire(path: &std::path::Path) -> CanaryResult<()> {
    let mut reqs = BTreeMap::<u64, usize>::new();
    for line in std::fs::read_to_string(path)?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        if entry.get("direction").and_then(Value::as_str) != Some("client_to_relay") {
            continue;
        }
        let payload = entry.get("payload").and_then(Value::as_str).unwrap_or("");
        if payload.starts_with("[\"REQ\"") {
            let connection = entry
                .get("connection")
                .and_then(Value::as_u64)
                .ok_or_else(|| CanaryError::new("REQ omitted proxy connection"))?;
            *reqs.entry(connection).or_default() += 1;
        }
    }
    let mut counts: Vec<_> = reqs.into_values().collect();
    counts.sort_unstable();
    if counts != [1, 3] {
        return Err(CanaryError::new(format!(
            "proxy did not witness one grouped and three separate REQs: {counts:?}"
        )));
    }
    Ok(())
}

fn finish(
    mut artifacts: RunArtifacts,
    options: &SmokeOptions,
    started: u128,
    version: &str,
    processes: &[crate::relay::ProcessFact],
    events: &[Event],
) -> CanaryResult<PathBuf> {
    let scenario = "subscription-grouping-equivalence";
    artifacts.record(
        "scenario_passed",
        json!({ "scenario": scenario, "event_ids": events.iter().map(|event| event.id.to_hex()).collect::<Vec<_>>(), "grouped_reqs": 1, "separate_reqs": 3 }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {version}\n- Grouped REQs: 1\n- No-grouping REQs: 3\n- Logical results: equal\n"
    ))?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let run_id = artifacts.run_id()?;
    let hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": run_id, "scenario": scenario, "scenario_seed": options.seed,
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
