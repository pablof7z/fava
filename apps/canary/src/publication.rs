//! Real-relay M5 publication, cancellation, and process-recovery scenarios.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use fava::{EventBuilder, Kind, Timestamp};
use fava::{Fava, Query, Receipt, ReceiptId, ReceiptOutcome, RelayDeliveryOutcome, RelayUrl};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache::EventCache;
use fava_event_cache_memory::MemoryEventCache;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write_store_redb::RedbWriteStore;
use nostr::event::{EventBuilder as NostrEventBuilder, FinalizeEvent};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::publication_child::GatedSigner;
use crate::publication_support::{
    finish, next_receipt, spawn_crash_child, wait_child_marker, wait_empty, wait_record,
    wait_recovered_terminal, wait_terminal, wait_until, wait_wire, wire_count,
};
use crate::relay::{ProcessFact, RelayProcess, RelaySupervisor};
use crate::{
    CanaryError, CanaryResult, SmokeOptions, WireProxy, deterministic_keys, reserve_port, wire,
};

/// Run one complete M5 scenario through the public Fava facade.
///
/// # Errors
///
/// Returns an exact orchestration, provider, relay, or evidence failure.
pub async fn run_publication_scenario(id: &str, options: SmokeOptions) -> CanaryResult<PathBuf> {
    if !matches!(
        id,
        "explicit-publish-optimistic"
            | "mixed-relay-outcomes"
            | "cancel-pre-handoff"
            | "crash-after-acceptance"
    ) {
        return Err(CanaryError::new(format!("unknown M5 scenario: {id}")));
    }
    let mut artifacts = RunArtifacts::create(&options.runs_directory, id, &options.seed)?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": id, "seed": options.seed }),
    )?;
    let started = unix_ms()?;
    let count = usize::from(id == "mixed-relay-outcomes") + 1;
    let mut relays = Vec::new();
    let mut facts = Vec::new();
    let mut version = None;
    for index in 0..count {
        let rejecting = id == "mixed-relay-outcomes" && index == 1;
        let relay = start_relay(&mut artifacts, &options, index, rejecting).await?;
        version.get_or_insert(relay.version.clone());
        facts.push(relay.process.fact("ready"));
        relays.push(relay);
    }
    let first = relays
        .first()
        .ok_or_else(|| CanaryError::new("M5 scenario started no relay"))?;
    let completed = match id {
        "explicit-publish-optimistic" => optimistic(&artifacts, &options.seed, first).await,
        "mixed-relay-outcomes" => mixed(&artifacts, &options.seed, &relays).await,
        "cancel-pre-handoff" => cancel(&artifacts, &options.seed, first).await,
        "crash-after-acceptance" => crash(&artifacts, &options.seed, first).await,
        _ => return Err(CanaryError::new("validated M5 scenario became unknown")),
    };
    for relay in relays {
        facts.push(relay.stop().await?);
    }
    let completed = completed?;
    finish(
        artifacts,
        id,
        &options,
        started,
        version
            .as_deref()
            .ok_or_else(|| CanaryError::new("M5 scenario has no relay version"))?,
        &facts,
        &completed.event_id,
        completed.receipt_id,
        &completed.details,
    )
}

struct LabRelay {
    process: RelayProcess,
    proxy: WireProxy,
    url: String,
    log: PathBuf,
    version: String,
}

impl LabRelay {
    async fn stop(self) -> CanaryResult<ProcessFact> {
        let fact = self.process.graceful_stop().await?;
        self.proxy.shutdown().await?;
        Ok(fact)
    }
}

struct Completed {
    event_id: String,
    receipt_id: u64,
    details: Value,
}

async fn start_relay(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
    index: usize,
    rejecting: bool,
) -> CanaryResult<LabRelay> {
    let directory = artifacts
        .root()
        .join(format!("relays/nostr-rs-relay-{index}"));
    let port = reserve_port().await?;
    let supervisor = if rejecting {
        let permitted = deterministic_keys("m5-reject-whitelist")?;
        RelaySupervisor::prepare_rejecting(
            &options.relay_binary,
            &directory,
            port,
            &permitted.public_key().to_hex(),
        )?
    } else {
        RelaySupervisor::prepare(&options.relay_binary, &directory, port)?
    };
    let version = supervisor.version().await?;
    let process = supervisor.spawn(1).await?;
    artifacts.record("relay_ready", process.fact("ready"))?;
    let log = artifacts.root().join(format!("wire/proxy-{index}.jsonl"));
    let proxy = WireProxy::start(supervisor.address(), &log).await?;
    Ok(LabRelay {
        url: proxy.url(),
        process,
        proxy,
        log,
        version,
    })
}

async fn optimistic(
    artifacts: &RunArtifacts,
    seed: &str,
    relay: &LabRelay,
) -> CanaryResult<Completed> {
    let keys = deterministic_keys(seed)?;
    let signer = Arc::new(GatedSigner::new(keys.clone()));
    let cache = Arc::new(MemoryEventCache::default());
    let store = Arc::new(
        RedbWriteStore::open(artifacts.root().join("children/optimistic.redb")).map_err(error)?,
    );
    let fava = assembly(Arc::clone(&cache), store, Some(Arc::clone(&signer)))?;
    let mut receipt_changes = fava.receipt_changes();
    let relay_url = RelayUrl::parse(&relay.url).map_err(error)?;
    let mut observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .map_err(error)?
                .from_relays([relay_url.clone()])
                .map_err(error)?,
        )
        .await
        .map_err(error)?;
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .created_at(Timestamp::now())
        .content(format!("Fava M5 optimistic {seed}"))
        .build()
        .map_err(error)?;
    let event_id = event.id.expect("checked builder installs id");
    let accepted = fava
        .to([relay_url])
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    let accepted_receipt = next_receipt(&mut receipt_changes, accepted.receipt_id()).await?;
    if !matches!(
        accepted_receipt.current.event,
        fava::EventValue::Unsigned(_)
    ) {
        return Err(CanaryError::new("first receipt change was not unsigned"));
    }
    wait_record(&mut observation, event_id, 0).await?;
    wait_until(|| signer.calls() == 1).await?;
    if wire_count(&relay.log, "EVENT")? != 0 {
        return Err(CanaryError::new("EVENT crossed wire before signer release"));
    }
    if cache.event(event_id).map_err(error)?.is_some() {
        return Err(CanaryError::new(
            "unsigned optimistic event entered event cache",
        ));
    }
    signer.release();
    let signed_receipt = next_receipt(&mut receipt_changes, accepted.receipt_id()).await?;
    if !matches!(signed_receipt.current.event, fava::EventValue::Signed(_)) {
        return Err(CanaryError::new("second receipt change was not signed"));
    }
    let attempting = next_receipt(&mut receipt_changes, accepted.receipt_id()).await?;
    if !attempting
        .destinations()
        .values()
        .all(|outcome| matches!(outcome, RelayDeliveryOutcome::Attempting))
    {
        return Err(CanaryError::new("third receipt change was not attempting"));
    }
    let outcome = next_receipt(&mut receipt_changes, accepted.receipt_id()).await?;
    if !outcome
        .destinations()
        .values()
        .all(|outcome| matches!(outcome, RelayDeliveryOutcome::Acknowledged { .. }))
    {
        return Err(CanaryError::new(
            "fourth receipt change was not acknowledged",
        ));
    }
    let receipt = wait_terminal(&accepted).await?;
    wait_record(&mut observation, event_id, 1).await?;
    if cache.event(event_id).map_err(error)?.is_none() {
        return Err(CanaryError::new("signed relay echo did not enter cache"));
    }
    observation.close();
    Ok(Completed {
        event_id: event_id.to_hex(),
        receipt_id: accepted.receipt_id().as_u64(),
        details: json!({
            "outcome": receipt.outcome,
            "destinations": receipt.destinations().iter().collect::<Vec<_>>(),
            "receipt_changes": ["unsigned", "signed", "attempting", "acknowledged"],
        }),
    })
}

async fn mixed(
    artifacts: &RunArtifacts,
    seed: &str,
    relays: &[LabRelay],
) -> CanaryResult<Completed> {
    let keys = deterministic_keys(seed)?;
    let event = NostrEventBuilder::new(Kind::TextNote, format!("Fava M5 mixed {seed}"))
        .custom_created_at(Timestamp::now())
        .finalize(&keys)?;
    let event_id = event.id;
    let unreachable =
        RelayUrl::parse(&format!("ws://127.0.0.1:{}", reserve_port().await?)).map_err(error)?;
    let destinations = [
        RelayUrl::parse(&relays[0].url).map_err(error)?,
        RelayUrl::parse(&relays[1].url).map_err(error)?,
        unreachable,
    ];
    let store = Arc::new(
        RedbWriteStore::open(artifacts.root().join("children/mixed.redb")).map_err(error)?,
    );
    let fava = assembly(Arc::new(MemoryEventCache::default()), store, None)?;
    let accepted = fava
        .to(destinations)
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    let receipt = wait_terminal(&accepted).await?;
    require_mixed(&receipt)?;
    wait_wire(&relays[0].log, "EVENT", 1).await?;
    wait_wire(&relays[1].log, "EVENT", 1).await?;
    Ok(Completed {
        event_id: event_id.to_hex(),
        receipt_id: accepted.receipt_id().as_u64(),
        details: json!({
            "outcome": receipt.outcome,
            "destinations": receipt.destinations().iter().collect::<Vec<_>>(),
        }),
    })
}

async fn cancel(artifacts: &RunArtifacts, seed: &str, relay: &LabRelay) -> CanaryResult<Completed> {
    let keys = deterministic_keys(seed)?;
    let signer = Arc::new(GatedSigner::new(keys.clone()));
    let store = Arc::new(
        RedbWriteStore::open(artifacts.root().join("children/cancel.redb")).map_err(error)?,
    );
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        store,
        Some(Arc::clone(&signer)),
    )?;
    let relay_url = RelayUrl::parse(&relay.url).map_err(error)?;
    let mut observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .map_err(error)?
                .cache_only(),
        )
        .await
        .map_err(error)?;
    let event = EventBuilder::new(keys.public_key(), Kind::TextNote)
        .content(format!("Fava M5 cancel {seed}"))
        .build()
        .map_err(error)?;
    let event_id = event.id.expect("checked builder installs id");
    let accepted = fava
        .to([relay_url])
        .map_err(error)?
        .publish(event)
        .map_err(error)?;
    wait_record(&mut observation, event_id, 0).await?;
    wait_until(|| signer.calls() == 1).await?;
    let cancelled = fava
        .cancel_publication(accepted.receipt_id())
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("accepted receipt disappeared"))?;
    wait_empty(&mut observation).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    if wire_count(&relay.log, "EVENT")? != 0 {
        return Err(CanaryError::new("cancelled event crossed handoff boundary"));
    }
    let repeated = fava
        .cancel_publication(accepted.receipt_id())
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("cancelled receipt disappeared"))?;
    if cancelled != repeated || !fava.remove_receipt(accepted.receipt_id()).map_err(error)? {
        return Err(CanaryError::new(
            "cancellation idempotence or receipt removal failed",
        ));
    }
    observation.close();
    Ok(Completed {
        event_id: event_id.to_hex(),
        receipt_id: accepted.receipt_id().as_u64(),
        details: json!({ "outcome": ReceiptOutcome::Cancelled, "event_frames": 0 }),
    })
}

async fn crash(artifacts: &RunArtifacts, seed: &str, relay: &LabRelay) -> CanaryResult<Completed> {
    let database = artifacts.root().join("children/crash.redb");
    let marker = artifacts.root().join("children/accepted.json");
    let mut child = spawn_crash_child(&database, &marker, &relay.url, seed, artifacts.root())?;
    wait_child_marker(&marker, &mut child).await?;
    child.kill().await?;
    let status = child.wait().await?;
    if status.success() {
        return Err(CanaryError::new("crash child did not die by hard kill"));
    }
    let marker: AcceptedMarker = serde_json::from_slice(&fs::read(&marker)?)?;
    let store = Arc::new(RedbWriteStore::open(&database).map_err(error)?);
    let keys = deterministic_keys(seed)?;
    let fava = assembly(
        Arc::new(MemoryEventCache::default()),
        store,
        Some(Arc::new(GatedSigner::new_released(keys))),
    )?;
    let recovered = fava
        .receipt(ReceiptId::from_u64(marker.receipt_id))
        .map_err(error)?
        .ok_or_else(|| CanaryError::new("accepted receipt missing after SIGKILL"))?;
    if recovered.current.id().to_hex() != marker.event_id {
        return Err(CanaryError::new("recovered event identity changed"));
    }
    let observation = fava
        .observe(
            Query::events()
                .kinds([Kind::TextNote])
                .map_err(error)?
                .cache_only(),
        )
        .await
        .map_err(error)?;
    if observation.current().events.len() != 1 {
        return Err(CanaryError::new(
            "recovered write was not query-visible without resubmission",
        ));
    }
    let receipt = wait_recovered_terminal(&fava, ReceiptId::from_u64(marker.receipt_id)).await?;
    let witness =
        wire::query_exact(&relay.url, recovered.current.id(), "m5-crash-recovery").await?;
    if !witness.found_event || !witness.saw_eose {
        return Err(CanaryError::new("recovered delivery not served by relay"));
    }
    Ok(Completed {
        event_id: marker.event_id,
        receipt_id: marker.receipt_id,
        details: json!({ "outcome": receipt.outcome, "recovered_without_resubmission": true }),
    })
}

fn assembly(
    cache: Arc<MemoryEventCache>,
    store: Arc<RedbWriteStore>,
    signer: Option<Arc<GatedSigner>>,
) -> CanaryResult<Fava> {
    let mut builder = Fava::builder()
        .event_cache(cache)
        .write_store(store)
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()));
    if let Some(signer) = signer {
        builder = builder.signer(signer);
    }
    builder.build().map_err(error)
}

fn require_mixed(receipt: &Receipt) -> CanaryResult<()> {
    let outcomes: Vec<_> = receipt.destinations().values().collect();
    if receipt.outcome != ReceiptOutcome::Complete
        || !outcomes
            .iter()
            .any(|outcome| matches!(outcome, RelayDeliveryOutcome::Acknowledged { .. }))
        || !outcomes
            .iter()
            .any(|outcome| matches!(outcome, RelayDeliveryOutcome::Rejected { .. }))
        || !outcomes
            .iter()
            .any(|outcome| matches!(outcome, RelayDeliveryOutcome::GivenUp { .. }))
    {
        return Err(CanaryError::new(format!(
            "mixed receipt lost exact outcomes: {outcomes:?}"
        )));
    }
    Ok(())
}

#[derive(Deserialize)]
struct AcceptedMarker {
    receipt_id: u64,
    event_id: String,
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
