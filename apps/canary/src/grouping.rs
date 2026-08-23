//! Real-relay literal tag-value subscription-planner equivalence scenario.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fava::{Fava, Query, SingleLetterTag};
use fava_event_cache_memory::MemoryEventCache;
use fava_ingest::admit_subscription_event;
use fava_query_standard::StandardQueryEvaluator;
use fava_state::{RelayAccess, RelaySessionKey, RelayUrl, Timestamp};
use fava_subscriptions::{RelayDemand, SubscriptionPlan, SubscriptionPlanner, demand_for_query};
use fava_subscriptions_no_grouping::planner;
use fava_subscriptions_standard::StandardSubscriptionPlanner;
use fava_transport::{HandoffOutcome, RelaySession, Transport};
use fava_transport_websocket::WebSocketTransport;
use fava_wire::{ClientMessage, RelayMessage, SubscriptionId, decode_relay, encode_client};
use fava_write_store_memory::MemoryWriteStore;
use nostr::event::{Event, EventBuilder, EventId, FinalizeEvent, Kind, Tag};
use nostr::filter::Filter;
use serde::Serialize;
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::RelaySupervisor;
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, command_output, deterministic_keys,
    repository_root, reserve_port, wire,
};

const LOGICAL_QUERY_COUNT: usize = 300;
const NO_GROUPING_BATCH_SIZE: usize = 32;
const CAPACITY_REFUSAL_TEXT: &str = "Maximum concurrent subscription count reached";

/// Compare standard and no-grouping planners through one controlled relay.
///
/// # Errors
///
/// Returns an error when case isolation, wire shape, logical results, relay
/// execution, source evidence, or artifact persistence differs.
pub async fn run_grouping_scenario(options: SmokeOptions) -> CanaryResult<PathBuf> {
    let scenario = "subscription-grouping-equivalence";
    let mut artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {scenario}"))?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": scenario, "seed": options.seed }),
    )?;
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
    let relay_key = RelaySessionKey::new(relay.clone(), RelayAccess::public());

    prove_case_isolation(&relay_key)?;
    let key = literal_key('d')?;
    let corpus = corpus(&options.seed, key)?;
    for item in &corpus {
        wire::publish(&proxy.url(), &item.event).await?;
    }
    let demand = corpus
        .iter()
        .enumerate()
        .map(|(index, item)| {
            demand_for_query(
                SubscriptionId::new(format!("tag-logical-{index:03}")),
                &item.query,
            )
        })
        .collect::<Vec<_>>();

    let standard_cache = Arc::new(MemoryEventCache::default());
    let standard = execute_plan(
        &relay,
        &demand,
        &StandardSubscriptionPlanner::default(),
        standard_cache.as_ref(),
    )
    .await?;
    if let Some(refusal) = standard.capacity_refusal {
        return Err(CanaryError::new(format!(
            "grouped request was refused for capacity: {refusal}"
        )));
    }
    let (separate_cache, separate) = execute_no_grouping(&relay, &demand).await?;

    let queries = corpus
        .iter()
        .map(|item| item.query.clone())
        .collect::<Vec<_>>();
    let grouped_results = visible(standard_cache, &queries).await?;
    let separate_results = visible(separate_cache, &queries).await?;
    let result_facts = compare_results(&corpus, &grouped_results, &separate_results, &relay_key)?;
    if standard.request_count != 1 || separate.request_count != LOGICAL_QUERY_COUNT {
        return Err(CanaryError::new(format!(
            "planner wire shape mismatch: grouped={}, separate={}",
            standard.request_count, separate.request_count
        )));
    }
    let grouped_filter =
        Filter::new().custom_tags(key, corpus.iter().map(|item| item.value.clone()));
    verify_wire(&artifacts.wire_log(), &grouped_filter, &demand, &separate)?;

    processes.push(process.graceful_stop().await?);
    proxy.shutdown().await?;
    finish(
        artifacts,
        &options,
        started,
        &version,
        &processes,
        &corpus,
        &result_facts,
        &separate,
    )
}

fn prove_case_isolation(relay: &RelaySessionKey) -> CanaryResult<()> {
    let lowercase = literal_key('d')?;
    let uppercase = literal_key('D')?;
    let demand = [
        demand_for_query(
            SubscriptionId::new("case-lowercase"),
            &Query::events().tag_values(lowercase, ["case-isolation"]),
        ),
        demand_for_query(
            SubscriptionId::new("case-uppercase"),
            &Query::events().tag_values(uppercase, ["case-isolation"]),
        ),
    ];
    let plan = StandardSubscriptionPlanner::default()
        .plan(relay, &demand)
        .map_err(error)?;
    if plan.messages.len() != 2 || plan.attribution.len() != 2 {
        return Err(CanaryError::new(format!(
            "opposite-case tag axes were not isolated: messages={}, attribution={}",
            plan.messages.len(),
            plan.attribution.len()
        )));
    }
    Ok(())
}

#[derive(Clone)]
struct CorpusItem {
    value: String,
    event: Event,
    query: Query,
}

fn corpus(seed: &str, key: SingleLetterTag) -> CanaryResult<Vec<CorpusItem>> {
    (0..LOGICAL_QUERY_COUNT)
        .map(|index| {
            let identity_seed = format!("grouping\0{seed}\0{index:03}");
            let keys = deterministic_keys(&identity_seed)?;
            let value = format!("tag-value-{index:03}-{seed}");
            let event = EventBuilder::new(
                Kind::TextNote,
                format!("Fava 06.1 grouping {seed} {index:03}"),
            )
            .tags([literal_tag(key, &value)?])
            .custom_created_at(Timestamp::from(1_700_000_000_u64 + index as u64))
            .finalize(&keys)
            .map_err(error)?;
            let query = Query::events()
                .tag_values(key, [value.clone()])
                .cache_only();
            Ok(CorpusItem {
                value,
                event,
                query,
            })
        })
        .collect()
}

fn literal_key(character: char) -> CanaryResult<SingleLetterTag> {
    SingleLetterTag::from_char(character).map_err(error)
}

fn literal_tag(key: SingleLetterTag, value: &str) -> CanaryResult<Tag> {
    Tag::parse([key.to_string(), value.to_owned()]).map_err(error)
}

#[derive(Debug)]
struct PlanExecution {
    request_count: usize,
    mode: &'static str,
    concurrent_attempt_requests: usize,
    capacity_refusal: Option<String>,
}

async fn execute_no_grouping(
    relay: &RelayUrl,
    demand: &[RelayDemand],
) -> CanaryResult<(Arc<MemoryEventCache>, PlanExecution)> {
    let concurrent_cache = Arc::new(MemoryEventCache::default());
    let concurrent = execute_plan(relay, demand, &planner(), concurrent_cache.as_ref()).await?;
    let Some(refusal) = concurrent.capacity_refusal else {
        return Ok((
            concurrent_cache,
            PlanExecution {
                request_count: concurrent.request_count,
                mode: "concurrent",
                concurrent_attempt_requests: concurrent.request_count,
                capacity_refusal: None,
            },
        ));
    };

    let batched_cache = Arc::new(MemoryEventCache::default());
    let mut request_count = 0;
    for batch in demand.chunks(NO_GROUPING_BATCH_SIZE) {
        let executed = execute_plan(relay, batch, &planner(), batched_cache.as_ref()).await?;
        if let Some(unexpected) = executed.capacity_refusal {
            return Err(CanaryError::new(format!(
                "bounded no-grouping batch was refused: {unexpected}"
            )));
        }
        request_count += executed.request_count;
    }
    Ok((
        batched_cache,
        PlanExecution {
            request_count,
            mode: "capacity-refusal-batched-32",
            concurrent_attempt_requests: concurrent.request_count,
            capacity_refusal: Some(refusal),
        },
    ))
}

async fn execute_plan(
    relay: &RelayUrl,
    demand: &[RelayDemand],
    planner: &dyn SubscriptionPlanner,
    cache: &MemoryEventCache,
) -> CanaryResult<PlanExecution> {
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
    let capacity_refusal = read_until_terminal(session.as_ref(), &plan, cache).await?;
    for id in plan.attribution.keys() {
        let frame = encode_client(&ClientMessage::close(id.clone())).map_err(error)?;
        let _ = session.send(frame).await;
    }
    session.close().await.map_err(error)?;
    Ok(PlanExecution {
        request_count: plan.messages.len(),
        mode: "single-plan",
        concurrent_attempt_requests: 0,
        capacity_refusal,
    })
}

async fn read_until_terminal(
    session: &dyn RelaySession,
    plan: &SubscriptionPlan,
    cache: &MemoryEventCache,
) -> CanaryResult<Option<String>> {
    tokio::time::timeout(Duration::from_secs(20), async {
        let mut complete = BTreeSet::new();
        while complete.len() < plan.attribution.len() {
            match decode_relay(&session.next_message().await.map_err(error)?).map_err(error)? {
                RelayMessage::Event {
                    subscription_id,
                    event,
                } => {
                    let id = subscription_id.into_owned();
                    admit_subscription_event(
                        cache,
                        session.key(),
                        &plan.attribution,
                        &id,
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
                RelayMessage::Notice(message) => {
                    let message = message.into_owned();
                    if message.contains(CAPACITY_REFUSAL_TEXT) {
                        return Ok(Some(format!("NOTICE: {message}")));
                    }
                    return Err(CanaryError::new(format!("relay NOTICE: {message}")));
                }
                RelayMessage::Closed {
                    subscription_id,
                    message,
                } => {
                    let refusal = format!("CLOSED {subscription_id}: {message}");
                    if message.contains(CAPACITY_REFUSAL_TEXT) {
                        return Ok(Some(refusal));
                    }
                    return Err(CanaryError::new(format!("relay {refusal}")));
                }
                RelayMessage::Auth { .. }
                | RelayMessage::Ok { .. }
                | RelayMessage::Count { .. }
                | RelayMessage::NegMsg { .. }
                | RelayMessage::NegErr { .. } => {}
            }
        }
        Ok(None)
    })
    .await
    .map_err(|_| CanaryError::new("planner EOSE deadline elapsed"))?
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LogicalResult {
    event_ids: BTreeSet<EventId>,
    relay_sessions: BTreeSet<RelaySessionKey>,
}

async fn visible(
    cache: Arc<MemoryEventCache>,
    queries: &[Query],
) -> CanaryResult<Vec<LogicalResult>> {
    let fava = Fava::builder()
        .event_cache(cache)
        .write_store(Arc::new(MemoryWriteStore::default()))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .build()
        .map_err(error)?;
    let mut results = Vec::with_capacity(queries.len());
    for query in queries {
        let observation = fava.observe(query.clone()).await.map_err(error)?;
        let current = observation.current();
        results.push(LogicalResult {
            event_ids: current.events.iter().map(fava::EventRecord::id).collect(),
            relay_sessions: current
                .events
                .iter()
                .flat_map(|record| record.relay_evidence.observations())
                .map(|observation| observation.session.clone())
                .collect(),
        });
    }
    Ok(results)
}

#[derive(Serialize)]
struct ResultFact {
    logical_index: usize,
    tag_value: String,
    expected_event_id: String,
    grouped_event_ids: Vec<String>,
    separate_event_ids: Vec<String>,
    grouped_relay_sessions: Vec<RelaySessionKey>,
    separate_relay_sessions: Vec<RelaySessionKey>,
}

fn compare_results(
    corpus: &[CorpusItem],
    grouped: &[LogicalResult],
    separate: &[LogicalResult],
    expected_relay: &RelaySessionKey,
) -> CanaryResult<Vec<ResultFact>> {
    let expected_sessions = BTreeSet::from([expected_relay.clone()]);
    let mut facts = Vec::with_capacity(corpus.len());
    for (index, ((item, grouped), separate)) in corpus.iter().zip(grouped).zip(separate).enumerate()
    {
        let expected_ids = BTreeSet::from([item.event.id]);
        if grouped != separate
            || grouped.event_ids != expected_ids
            || grouped.relay_sessions != expected_sessions
        {
            return Err(CanaryError::new(format!(
                "logical query {index:03} differed: expected={expected_ids:?}, grouped={grouped:?}, separate={separate:?}"
            )));
        }
        facts.push(ResultFact {
            logical_index: index,
            tag_value: item.value.clone(),
            expected_event_id: item.event.id.to_hex(),
            grouped_event_ids: grouped.event_ids.iter().map(EventId::to_hex).collect(),
            separate_event_ids: separate.event_ids.iter().map(EventId::to_hex).collect(),
            grouped_relay_sessions: grouped.relay_sessions.iter().cloned().collect(),
            separate_relay_sessions: separate.relay_sessions.iter().cloned().collect(),
        });
    }
    if facts.len() != LOGICAL_QUERY_COUNT {
        return Err(CanaryError::new("logical result corpus was incomplete"));
    }
    Ok(facts)
}

fn verify_wire(
    path: &std::path::Path,
    grouped_filter: &Filter,
    logical_demand: &[RelayDemand],
    separate: &PlanExecution,
) -> CanaryResult<()> {
    let mut reqs = BTreeMap::<u64, Vec<Value>>::new();
    let mut capacity_refusals = Vec::new();
    read_wire_requests(path, &mut reqs, &mut capacity_refusals)?;
    let mut counts: Vec<_> = reqs.values().map(Vec::len).collect();
    counts.sort_unstable();
    let expected = if separate.capacity_refusal.is_some() {
        let mut expected = vec![1, LOGICAL_QUERY_COUNT];
        expected.extend(std::iter::repeat_n(
            NO_GROUPING_BATCH_SIZE,
            LOGICAL_QUERY_COUNT / NO_GROUPING_BATCH_SIZE,
        ));
        let remainder = LOGICAL_QUERY_COUNT % NO_GROUPING_BATCH_SIZE;
        if remainder != 0 {
            expected.push(remainder);
        }
        expected.sort_unstable();
        expected
    } else {
        vec![1, LOGICAL_QUERY_COUNT]
    };
    if counts != expected {
        return Err(CanaryError::new(format!(
            "proxy REQ witness mismatch: expected={expected:?}, actual={counts:?}"
        )));
    }
    if separate.capacity_refusal.is_some() && capacity_refusals.is_empty() {
        return Err(CanaryError::new(
            "batched execution omitted explicit relay capacity refusal",
        ));
    }
    verify_filter_sets(&reqs, grouped_filter, logical_demand, separate)
}

fn read_wire_requests(
    path: &std::path::Path,
    reqs: &mut BTreeMap<u64, Vec<Value>>,
    capacity_refusals: &mut Vec<String>,
) -> CanaryResult<()> {
    for line in std::fs::read_to_string(path)?.lines() {
        let entry: Value = serde_json::from_str(line)?;
        let payload = entry.get("payload").and_then(Value::as_str).unwrap_or("");
        let direction = entry.get("direction").and_then(Value::as_str);
        let frame_type = entry.get("frame_type").and_then(Value::as_str);
        if direction == Some("client_to_relay") && frame_type == Some("text") {
            let message: Value = serde_json::from_str(payload)?;
            let Some(parts) = message.as_array() else {
                return Err(CanaryError::new("client wire payload was not a JSON array"));
            };
            if parts.first().and_then(Value::as_str) != Some("REQ") {
                continue;
            }
            if parts.len() != 3
                || parts.get(1).and_then(Value::as_str).is_none()
                || parts.get(2).and_then(Value::as_object).is_none()
            {
                return Err(CanaryError::new(
                    "planner REQ must contain one subscription id and one filter",
                ));
            }
            let connection = entry
                .get("connection")
                .and_then(Value::as_u64)
                .ok_or_else(|| CanaryError::new("REQ omitted proxy connection"))?;
            reqs.entry(connection).or_default().push(parts[2].clone());
        }
        if direction == Some("relay_to_client") && payload.contains(CAPACITY_REFUSAL_TEXT) {
            capacity_refusals.push(payload.to_owned());
        }
    }
    Ok(())
}

fn verify_filter_sets(
    reqs: &BTreeMap<u64, Vec<Value>>,
    grouped_filter: &Filter,
    logical_demand: &[RelayDemand],
    separate: &PlanExecution,
) -> CanaryResult<()> {
    let expected_grouped = serde_json::to_value(grouped_filter)?;
    let grouped_connections = reqs
        .iter()
        .filter(|(_, filters)| filters.as_slice() == [expected_grouped.clone()])
        .map(|(connection, _)| *connection)
        .collect::<Vec<_>>();
    if grouped_connections.len() != 1 {
        return Err(CanaryError::new(format!(
            "expected one exact grouped filter, found connections {grouped_connections:?}"
        )));
    }
    let grouped_connection = grouped_connections[0];
    let expected_separate = logical_demand
        .iter()
        .map(|demand| serde_json::to_string(&demand.filter))
        .collect::<Result<Vec<_>, _>>()?;
    let expected_separate = filter_multiset(expected_separate);
    let concurrent_connections = reqs
        .iter()
        .filter(|(connection, filters)| {
            **connection != grouped_connection && filters.len() == LOGICAL_QUERY_COUNT
        })
        .map(|(connection, filters)| (*connection, filters))
        .collect::<Vec<_>>();
    if concurrent_connections.len() != 1 {
        return Err(CanaryError::new(format!(
            "expected one {LOGICAL_QUERY_COUNT}-REQ no-grouping connection, found {}",
            concurrent_connections.len()
        )));
    }
    let concurrent_connection = concurrent_connections[0].0;
    let concurrent_filters =
        filter_multiset(concurrent_connections[0].1.iter().map(Value::to_string));
    if concurrent_filters != expected_separate {
        return Err(CanaryError::new(
            "concurrent no-grouping filters differed from logical demand",
        ));
    }
    let retry_filters = filter_multiset(
        reqs.iter()
            .filter(|(connection, _)| {
                **connection != grouped_connection && **connection != concurrent_connection
            })
            .flat_map(|(_, filters)| filters.iter().map(Value::to_string)),
    );
    if separate.capacity_refusal.is_some() {
        if retry_filters != expected_separate {
            return Err(CanaryError::new(
                "batched no-grouping retry filters differed from logical demand",
            ));
        }
    } else if !retry_filters.is_empty() {
        return Err(CanaryError::new(
            "unexpected extra no-grouping filters without a capacity retry",
        ));
    }
    Ok(())
}

fn filter_multiset(filters: impl IntoIterator<Item = String>) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for filter in filters {
        *counts.entry(filter).or_default() += 1;
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn finish(
    mut artifacts: RunArtifacts,
    options: &SmokeOptions,
    started: u128,
    version: &str,
    processes: &[crate::relay::ProcessFact],
    corpus: &[CorpusItem],
    results: &[ResultFact],
    separate: &PlanExecution,
) -> CanaryResult<PathBuf> {
    let scenario = "subscription-grouping-equivalence";
    artifacts.write_json("results.json", &results)?;
    artifacts.record(
        "scenario_passed",
        json!({
            "scenario": scenario,
            "logical_queries": LOGICAL_QUERY_COUNT,
            "case_isolation": true,
            "grouped_reqs": 1,
            "separate_reqs": separate.request_count,
            "no_grouping_execution_mode": separate.mode,
            "concurrent_attempt_reqs": separate.concurrent_attempt_requests,
            "capacity_refusal": separate.capacity_refusal,
            "result_equivalence": true,
            "relay_source_evidence_equivalence": true,
            "event_ids": corpus.iter().map(|item| item.event.id.to_hex()).collect::<Vec<_>>(),
        }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {version}\n- Logical queries: {LOGICAL_QUERY_COUNT}\n- Case-isolation preflight: passed (lowercase and uppercase axes planned separately)\n- Grouped REQs: 1\n- No-grouping REQs: {}\n- No-grouping execution mode: {}\n- Concurrent-first attempt REQs: {}\n- Exact capacity refusal: {}\n- Per-query event IDs: exactly one and equal\n- Per-query relay source evidence: exact and equal\n",
        separate.request_count,
        separate.mode,
        separate.concurrent_attempt_requests,
        separate.capacity_refusal.as_deref().unwrap_or("none"),
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
            "relay_version": version,
            "started_unix_ms": started,
            "ended_unix_ms": unix_ms()?,
            "logical_queries": LOGICAL_QUERY_COUNT,
            "case_isolation": true,
            "grouped_reqs": 1,
            "separate_reqs": separate.request_count,
            "no_grouping_execution_mode": separate.mode,
            "concurrent_attempt_reqs": separate.concurrent_attempt_requests,
            "capacity_refusal": separate.capacity_refusal,
            "result_equivalence": true,
            "relay_source_evidence_equivalence": true,
            "relay_processes": processes,
            "artifact_sha256": hashes,
        }),
    )?;
    artifacts.append_app_stdout(&format!("passed {scenario}"))?;
    Ok(artifacts.root().to_owned())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}

#[cfg(test)]
#[path = "grouping_tests.rs"]
mod tests;
