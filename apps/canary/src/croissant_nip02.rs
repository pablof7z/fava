//! Controlled Croissant proof for universal publication and typed NIP-02 reads.
//!
//! This file stays above the 500-line soft limit because one owner must keep the launched
//! process lifecycle, public flow, teardown facts, and pair-level evidence checks causally
//! connected. Reusable evidence hashing and secret scanning live in the adjacent helper module.

use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use fava::ReceiptOutcome;
use fava::{EventBuilder, EventValue, Fava, Kind, Observation, Receipt, RelayUrl, Tag};
use fava_delivery_standard::StandardDeliveryPolicy;
use fava_event_cache_memory::MemoryEventCache;
use fava_nip02::ContactList;
use fava_publisher_nip01::Nip01Publisher;
use fava_query_standard::StandardQueryEvaluator;
use fava_signer::Signer;
use fava_subscriptions_no_grouping::planner;
use fava_transport_websocket::WebSocketTransport;
use fava_write::EventId;
use fava_write_store_redb::RedbWriteStore;
use nostr::key::Keys;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::croissant::{
    CroissantLimits, CroissantReadyFact, CroissantSupervisor, CroissantTeardown, process_is_alive,
};
use crate::croissant_nip02_evidence::{
    SECRET_SCAN_CLASSES, artifact_seal, assert_secrets_absent, directory_contains, manifest_roots,
    secret_needles, verify_artifact_seal, verify_hashes,
};
use crate::environment::croissant_fixture_source;
use crate::gate_signer::{GateSigner, PendingSign, deterministic_finalize, next_sign};
use crate::publication_support::{wait_record, wait_terminal};
use crate::{
    CanaryError, CanaryResult, WireProxy, command_output, deterministic_keys, repository_root, wire,
};

const SCENARIO: &str = "croissant-nip02-public-flow";
const OPERATION_MS: u64 = 30_000;
const WIRE_BYTES: u64 = 1_048_576;
const LEGACY_CONTENT: &str = "legacy shared contact-list content";
#[allow(
    clippy::unicode_not_nfc,
    reason = "the NIP-02 proof preserves decomposed petname UTF-8 without normalization"
)]
const PETNAME: &str = "alíce";

/// Process-memory input for one controlled Croissant NIP-02 proof.
#[derive(Clone, Debug)]
pub struct CroissantNip02Options {
    /// Croissant executable to launch without modifying its checkout.
    pub relay_binary: PathBuf,
    /// Disposable identity seed, never retained outside process memory.
    pub scenario_seed: String,
    /// Parent directory for one fresh durable evidence bundle.
    pub runs_directory: PathBuf,
}

/// Durable location produced by one completed controlled run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CroissantNip02Outcome {
    /// Fresh run directory containing the completed manifest and its artifacts.
    pub run_directory: PathBuf,
}

struct FlowFacts {
    group_id: String,
    group_receipt: Receipt,
    baseline_receipt: Receipt,
    edit_receipt: Receipt,
    local_revision: u64,
    relay_revision: u64,
    author: String,
    target: String,
    relay_hint: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SecretScanStage {
    BeforeSeal,
    AfterManifest,
}

type SecretScanHook =
    Arc<dyn Fn(SecretScanStage, &Path, &[Vec<u8>]) -> CanaryResult<()> + Send + Sync>;

/// Run the kind-9007 control and README NIP-02 flow through one public Fava assembly.
///
/// # Errors
///
/// Returns an attributed process, publication, observation, wire, bound, or evidence failure.
pub async fn run_croissant_nip02_scenario(
    options: CroissantNip02Options,
) -> CanaryResult<CroissantNip02Outcome> {
    Box::pin(run_croissant_nip02_scenario_inner(options, None)).await
}

async fn run_croissant_nip02_scenario_inner(
    options: CroissantNip02Options,
    secret_scan_hook: Option<SecretScanHook>,
) -> CanaryResult<CroissantNip02Outcome> {
    let seed_hash = hex::encode(Sha256::digest(options.scenario_seed.as_bytes()));
    let keys = deterministic_keys(&format!("croissant-author\0{}", options.scenario_seed))?;
    let mut artifacts =
        RunArtifacts::create_staged(&options.runs_directory, SCENARIO, &options.scenario_seed)?;
    let started = unix_ms()?;
    artifacts.record(
        "scenario_started",
        json!({
            "scenario": SCENARIO,
            "scenario_seed_sha256": seed_hash,
            "author_public_key": keys.public_key().to_hex(),
        }),
    )?;
    let relay_root = artifacts.root().join("relays/croissant");
    let croissant_source = croissant_fixture_source()?;
    let supervisor = CroissantSupervisor::prepare(
        &options.relay_binary,
        &croissant_source,
        &relay_root,
        &keys.public_key().to_hex(),
        &seed_hash,
        CroissantLimits::default(),
    )
    .map_err(error)?;
    let expected_logs = (
        supervisor.stdout_path().to_owned(),
        supervisor.stderr_path().to_owned(),
    );
    let process = supervisor.start().await.map_err(error)?;
    let ready = process.ready_fact();
    if (ready.stdout_path.clone(), ready.stderr_path.clone()) != expected_logs {
        return Err(CanaryError::new(
            "Croissant log provenance changed after launch",
        ));
    }
    artifacts.record("croissant_ready", &ready)?;
    let proxy = WireProxy::start(ready.endpoint, &artifacts.wire_log()).await?;
    let proxy_url = proxy.url();
    let flow = tokio::time::timeout(
        Duration::from_millis(OPERATION_MS),
        Box::pin(execute_flow(
            &artifacts,
            &options.scenario_seed,
            keys,
            &proxy_url,
        )),
    )
    .await
    .map_err(|_| CanaryError::new("Croissant NIP-02 operation deadline elapsed"));
    let proxy_stop = proxy.shutdown().await;
    let teardown = process.stop().await.map_err(error);
    proxy_stop?;
    let teardown = teardown?;
    artifacts.record("croissant_teardown", &teardown)?;
    let facts = flow??;
    finish_run(
        artifacts,
        &options,
        started,
        &ready,
        &teardown,
        &facts,
        secret_scan_hook.as_ref(),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one linear function keeps the exact cross-boundary evidence chronology auditable"
)]
async fn execute_flow(
    artifacts: &RunArtifacts,
    seed: &str,
    keys: Keys,
    proxy_url: &str,
) -> CanaryResult<FlowFacts> {
    let group_id =
        hex::encode(Sha256::digest(format!("fava-croissant-group\0{seed}")))[..32].to_owned();
    let target = deterministic_keys(&format!("croissant-target\0{seed}"))?.public_key();
    let relay = RelayUrl::parse(proxy_url).map_err(error)?;
    let (gate, mut sign_requests) = GateSigner::new(keys.public_key());
    let signer: Arc<dyn Signer> = Arc::new(gate);
    let fava = assembly(
        artifacts.root().join("children/croissant-nip02.redb"),
        signer,
    )?;
    let mut observation = fava
        .observe(
            fava_nip02::contact_list(keys.public_key())
                .map_err(error)?
                .from_relays([relay.clone()])
                .map_err(error)?,
        )
        .await
        .map_err(error)?;

    let group = EventBuilder::new(keys.public_key(), Kind::from(9007_u16))
        .tag(Tag::parse(["h", &group_id]).map_err(error)?)
        .build()
        .map_err(error)?;
    let group_write = fava
        .to([relay.clone()])
        .map_err(error)?
        .publish(group)
        .map_err(error)?;
    complete_next_sign(&mut sign_requests, &keys).await?;
    let group_receipt = wait_terminal(&group_write).await?;
    require_terminal(&group_write, &group_receipt, 9007)?;
    let group_wire = wire::query_exact(
        proxy_url,
        group_receipt.current.id(),
        "fava-croissant-group-create",
    )
    .await?;
    if !group_wire.found_event || !group_wire.saw_eose {
        return Err(CanaryError::new(
            "kind-9007 relay control was not exactly readable",
        ));
    }

    let baseline = EventBuilder::new(keys.public_key(), Kind::ContactList)
        .tags([
            Tag::parse(["h", &group_id]).map_err(error)?,
            Tag::parse(["t", "nostr"]).map_err(error)?,
            Tag::parse(["something-something"]).map_err(error)?,
        ])
        .content(LEGACY_CONTENT)
        .build()
        .map_err(error)?;
    let baseline_write = fava
        .to([relay.clone()])
        .map_err(error)?
        .publish(baseline)
        .map_err(error)?;
    let baseline_id = baseline_write.receipt().map_err(error)?.current.id();
    wait_record(&mut observation, baseline_id, 0).await?;
    let baseline_record = record(&observation, baseline_id)?;
    let empty = ContactList::from_event(baseline_record.event()).map_err(error)?;
    if !empty.follows().is_empty() || !empty.evidence().is_empty() {
        return Err(CanaryError::new(
            "baseline kind-3 did not decode as valid empty",
        ));
    }
    complete_next_sign(&mut sign_requests, &keys).await?;
    let baseline_receipt = wait_terminal(&baseline_write).await?;
    require_terminal(&baseline_write, &baseline_receipt, 3)?;
    wait_record(&mut observation, baseline_receipt.current.id(), 1).await?;

    let edit =
        fava_nip02::follow_with(target, Some(relay.clone()), Some(PETNAME)).map_err(error)?;
    let edit_write = fava
        .by(keys.public_key())
        .to([relay.clone()])
        .map_err(error)?
        .publish(edit)
        .map_err(error)?;
    let edit_id = edit_write.receipt().map_err(error)?.current.id();
    wait_record(&mut observation, edit_id, 0).await?;
    let local = record(&observation, edit_id)?;
    let local_revision = observation.current().revision.0;
    let local_publication = local.publication().ok_or_else(|| {
        CanaryError::new("local NIP-02 revision lacked publication evidence")
    })?;
    if local_publication.write_id != edit_write.write_id()
        || local_publication.receipt_id != edit_write.receipt_id()
        || !local.relay_occurrences().is_empty()
    {
        return Err(CanaryError::new(
            "local NIP-02 evidence did not precede relay acknowledgement",
        ));
    }
    complete_next_sign(&mut sign_requests, &keys).await?;
    let edit_receipt = wait_terminal(&edit_write).await?;
    require_terminal(&edit_write, &edit_receipt, 3)?;
    wait_record(&mut observation, edit_receipt.current.id(), 1).await?;
    let relay_revision = observation.current().revision.0;
    let relayed = record(&observation, edit_receipt.current.id())?;
    validate_final(relayed.event(), target, &relay, &group_id)?;
    if relay_revision <= local_revision
        || edit_receipt.current.publication.revision_source
            != Some(baseline_receipt.current.id())
    {
        return Err(CanaryError::new(
            "relay revision or revision source did not advance exactly",
        ));
    }
    let exact =
        wire::query_exact(proxy_url, edit_receipt.current.id(), "fava-croissant-nip02").await?;
    if !exact.found_event || !exact.saw_eose {
        return Err(CanaryError::new("final kind-3 relay echo was not exact"));
    }
    let wire_size = fs::metadata(artifacts.wire_log())?.len();
    if wire_size > WIRE_BYTES {
        return Err(CanaryError::new(
            "Croissant wire witness exceeded declared byte bound",
        ));
    }
    observation.close();
    Ok(FlowFacts {
        group_id,
        group_receipt,
        baseline_receipt,
        edit_receipt,
        local_revision,
        relay_revision,
        author: keys.public_key().to_hex(),
        target: target.to_hex(),
        relay_hint: relay.to_string(),
    })
}

fn assembly(database: PathBuf, signer: Arc<dyn Signer>) -> CanaryResult<Fava> {
    Fava::builder()
        .event_cache(Arc::new(MemoryEventCache::default()))
        .write_store(Arc::new(RedbWriteStore::open(database).map_err(error)?))
        .query_evaluator(Arc::new(StandardQueryEvaluator))
        .subscription_planner(Arc::new(planner()))
        .transport(Arc::new(WebSocketTransport::default()))
        .publisher(Arc::new(Nip01Publisher))
        .delivery_policy(Arc::new(StandardDeliveryPolicy::default()))
        .signers([signer])
        .appliers([fava_nip02::applier()])
        .build()
        .map_err(error)
}

async fn complete_next_sign(
    requests: &mut mpsc::Receiver<PendingSign>,
    keys: &Keys,
) -> CanaryResult<()> {
    let pending = next_sign(requests).await?;
    let signed = deterministic_finalize(pending.event.clone(), keys).map_err(error)?;
    pending.complete(signed)
}

fn record(observation: &Observation, id: EventId) -> CanaryResult<fava::EventRecord> {
    observation
        .current()
        .events
        .iter()
        .find(|record| record.id() == id)
        .cloned()
        .ok_or_else(|| CanaryError::new("expected contact-list observation record was absent"))
}

fn require_terminal(write: &fava::Write, receipt: &Receipt, kind: u16) -> CanaryResult<()> {
    if receipt.write_id != write.write_id()
        || receipt.receipt_id != write.receipt_id()
        || receipt.outcome != ReceiptOutcome::Complete
        || receipt.acknowledged() != 1
        || receipt.current.event.kind().as_u16() != kind
    {
        return Err(CanaryError::new(
            "publication identities or terminal receipt diverged",
        ));
    }
    Ok(())
}

fn validate_final(
    event: &EventValue,
    target: nostr::key::PublicKey,
    relay: &RelayUrl,
    group_id: &str,
) -> CanaryResult<()> {
    let list = ContactList::from_event(event).map_err(error)?;
    let follows = list.follows();
    if follows.len() != 1
        || follows[0].pubkey() != target
        || follows[0].relay() != Some(relay)
        || follows[0].petname() != Some(PETNAME)
        || follows[0].source_index() != 3
        || !list.evidence().is_empty()
    {
        return Err(CanaryError::new(
            "typed contact-list decode did not preserve exact metadata",
        ));
    }
    let tags = event.tags().iter().map(Tag::as_slice).collect::<Vec<_>>();
    let exact = |index: usize, expected: &[&str]| {
        tags.get(index).is_some_and(|actual| {
            actual
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied())
        })
    };
    if !exact(0, &["h", group_id])
        || !exact(1, &["t", "nostr"])
        || !exact(2, &["something-something"])
        || event_content(event) != LEGACY_CONTENT
    {
        return Err(CanaryError::new("foreign kind-3 tags or content changed"));
    }
    Ok(())
}

fn event_content(event: &EventValue) -> &str {
    match event {
        EventValue::Unsigned(event) => &event.content,
        EventValue::Signed(event) => &event.content,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one finalizer owns the bounded artifact scan, manifest seal, and teardown proof"
)]
fn finish_run(
    mut artifacts: RunArtifacts,
    options: &CroissantNip02Options,
    started: u128,
    ready: &CroissantReadyFact,
    teardown: &CroissantTeardown,
    facts: &FlowFacts,
    secret_scan_hook: Option<&SecretScanHook>,
) -> CanaryResult<CroissantNip02Outcome> {
    let run_id = artifacts.run_id()?;
    let wire_bytes = fs::metadata(artifacts.wire_log())?.len();
    let edit_id = facts.edit_receipt.current.id().to_hex();
    let scoped_write = format!("{run_id}:{}", facts.edit_receipt.write_id.as_u64());
    let scoped_receipt = format!("{run_id}:{}", facts.edit_receipt.receipt_id.as_u64());
    artifacts.record(
        "scenario_passed",
        json!({
            "group_id": facts.group_id,
            "group_event_id": facts.group_receipt.current.id().to_hex(),
            "baseline_event_id": facts.baseline_receipt.current.id().to_hex(),
            "event_id": edit_id,
            "write_id": scoped_write,
            "receipt_id": scoped_receipt,
            "local_revision": facts.local_revision,
            "relay_revision": facts.relay_revision,
        }),
    )?;
    artifacts.write_json(
        "relays/croissant/process.json",
        &json!({
            "ready": ready,
            "teardown": teardown,
        }),
    )?;
    artifacts.write_json("flow.json", &json!({
        "author_public_key": facts.author,
        "target_public_key": facts.target,
        "group_id": facts.group_id,
        "relay_hint": facts.relay_hint,
        "petname": PETNAME,
        "foreign_tags": [["h", facts.group_id.as_str()], ["t", "nostr"], ["something-something"]],
        "foreign_content": LEGACY_CONTENT,
        "group_event_id": facts.group_receipt.current.id().to_hex(),
        "baseline_event_id": facts.baseline_receipt.current.id().to_hex(),
        "event_id": edit_id,
        "write_id": scoped_write,
        "receipt_id": scoped_receipt,
        "write_sequence": facts.edit_receipt.write_id.as_u64(),
        "receipt_sequence": facts.edit_receipt.receipt_id.as_u64(),
        "revision_id": facts.edit_receipt.current.publication.revision_id.as_u64(),
        "revision_source": facts.edit_receipt.current.publication.revision_source.map(|id| id.to_hex()),
        "local_revision": facts.local_revision,
        "relay_revision": facts.relay_revision,
        "terminal_outcome": format!("{:?}", facts.edit_receipt.outcome),
        "acknowledged": facts.edit_receipt.acknowledged(),
        "wire_bytes": wire_bytes,
    }))?;
    artifacts.write_report(&format!(
        "# Controlled Croissant NIP-02 proof\n\n- Result: passed\n- Group: {}\n- Author: {}\n- Event: {}\n- Local revision: {}\n- Relay revision: {}\n",
        facts.group_id, facts.author, edit_id, facts.local_revision, facts.relay_revision,
    ))?;
    let author_keys = deterministic_keys(&format!("croissant-author\0{}", options.scenario_seed))?;
    let target_keys = deterministic_keys(&format!("croissant-target\0{}", options.scenario_seed))?;
    let secret_needles = secret_needles(
        options.scenario_seed.as_bytes(),
        [&author_keys, &target_keys],
    )?;
    run_secret_scan_hook(
        secret_scan_hook,
        SecretScanStage::BeforeSeal,
        artifacts.root(),
        &secret_needles,
    )?;
    assert_secrets_absent(artifacts.root(), &secret_needles)?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let hashes = artifacts.artifact_hashes()?;
    let mut manifest = json!({
        "run_id": run_id,
        "scenario": SCENARIO,
        "scenario_seed_sha256": ready.scenario_seed_sha256,
        "group_id": facts.group_id,
        "author_public_key": facts.author,
        "target_public_key": facts.target,
        "group_event_id": facts.group_receipt.current.id().to_hex(),
        "baseline_event_id": facts.baseline_receipt.current.id().to_hex(),
        "event_id": edit_id,
        "write_id": scoped_write,
        "receipt_id": scoped_receipt,
        "write_sequence": facts.edit_receipt.write_id.as_u64(),
        "receipt_sequence": facts.edit_receipt.receipt_id.as_u64(),
        "revision_id": facts.edit_receipt.current.publication.revision_id.as_u64(),
        "revision_source": facts.edit_receipt.current.publication.revision_source.map(|id| id.to_hex()),
        "local_revision": facts.local_revision,
        "relay_revision": facts.relay_revision,
        "foreign_tags_preserved": true,
        "foreign_content_preserved": true,
        "typed_decode_exact": true,
        "secret_scan_passed": true,
        "secret_scan_classes": SECRET_SCAN_CLASSES,
        "executable_sha256": ready.executable_sha256,
        "source_head": ready.source_head,
        "ready": ready,
        "teardown": teardown,
        "bounds": {
            "operation_ms": OPERATION_MS,
            "wire_bytes": WIRE_BYTES,
            "wire_bytes_observed": wire_bytes,
            "log_bytes": ready.limits.log_bytes,
            "readiness_ms": ready.limits.readiness_ms,
            "readiness_stability_ms": ready.limits.readiness_stability_ms,
            "teardown_ms": ready.limits.teardown_ms,
        },
        "terminal": {
            "outcome": format!("{:?}", facts.edit_receipt.outcome),
            "acknowledged": facts.edit_receipt.acknowledged(),
            "destinations": facts.edit_receipt.destinations().len(),
        },
        "artifact_sha256": hashes,
        "fava_revision": revision,
        "started_unix_ms": started,
        "ended_unix_ms": unix_ms()?,
    });
    let seal = artifact_seal(&author_keys, &manifest)?;
    manifest
        .as_object_mut()
        .ok_or_else(|| CanaryError::new("Croissant manifest was not an object"))?
        .insert("artifact_seal".to_owned(), serde_json::to_value(seal)?);
    artifacts.write_json("manifest.json", &manifest)?;
    run_secret_scan_hook(
        secret_scan_hook,
        SecretScanStage::AfterManifest,
        artifacts.root(),
        &secret_needles,
    )?;
    assert_secrets_absent(artifacts.root(), &secret_needles)?;
    let run_directory = artifacts.promote()?;
    Ok(CroissantNip02Outcome { run_directory })
}

fn run_secret_scan_hook(
    hook: Option<&SecretScanHook>,
    stage: SecretScanStage,
    root: &Path,
    needles: &[Vec<u8>],
) -> CanaryResult<()> {
    hook.map_or(Ok(()), |hook| hook(stage, root, needles))
}

/// Verify exactly two completed, independent, bounded Croissant scenario manifests.
///
/// # Errors
///
/// Returns a redacted refusal for missing, reused, tampered, unbounded, live, or secret evidence.
pub fn verify_croissant_run_pair(runs_directory: impl AsRef<Path>) -> CanaryResult<()> {
    let roots = manifest_roots(runs_directory.as_ref())?;
    if roots.len() != 2 {
        return Err(CanaryError::new(
            "Croissant pair root must contain exactly two manifests",
        ));
    }
    let mut runs = Vec::new();
    for root in roots {
        let manifest: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        reject_secret_fields(&manifest)?;
        validate_manifest(&root, &manifest)?;
        runs.push((root, manifest));
    }
    for field in [
        "scenario_seed_sha256",
        "group_id",
        "group_event_id",
        "baseline_event_id",
        "event_id",
        "write_id",
        "receipt_id",
    ] {
        if required_string(&runs[0].1, field)? == required_string(&runs[1].1, field)? {
            return Err(CanaryError::new(format!("Croissant pair reused {field}")));
        }
    }
    reject_cross_run_data(&runs[0], &runs[1])?;
    Ok(())
}

fn reject_cross_run_data(first: &(PathBuf, Value), second: &(PathBuf, Value)) -> CanaryResult<()> {
    for field in [
        "group_id",
        "group_event_id",
        "baseline_event_id",
        "event_id",
    ] {
        let first_value = required_string(&first.1, field)?.as_bytes();
        let second_value = required_string(&second.1, field)?.as_bytes();
        if directory_contains(&first.0, second_value, true)?
            || directory_contains(&second.0, first_value, true)?
        {
            return Err(CanaryError::new(format!(
                "a Croissant run retained the other run's {field} data"
            )));
        }
    }
    Ok(())
}

fn validate_manifest(root: &Path, manifest: &Value) -> CanaryResult<()> {
    verify_hashes(root, manifest)?;
    verify_artifact_seal(manifest)?;
    if manifest.get("scenario").and_then(Value::as_str) != Some(SCENARIO)
        || manifest.get("secret_scan_passed").and_then(Value::as_bool) != Some(true)
        || manifest
            .get("foreign_tags_preserved")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest
            .get("foreign_content_preserved")
            .and_then(Value::as_bool)
            != Some(true)
        || manifest.get("typed_decode_exact").and_then(Value::as_bool) != Some(true)
    {
        return Err(CanaryError::new(
            "Croissant manifest completion facts are incomplete",
        ));
    }
    let classes = manifest
        .get("secret_scan_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new("Croissant manifest omitted secret scan classes"))?;
    if !classes
        .iter()
        .map(|value| value.as_str())
        .eq(SECRET_SCAN_CLASSES.iter().copied().map(Some))
    {
        return Err(CanaryError::new(
            "Croissant manifest secret scan classes were incomplete",
        ));
    }
    for field in [
        "scenario_seed_sha256",
        "group_id",
        "author_public_key",
        "group_event_id",
        "baseline_event_id",
        "event_id",
        "write_id",
        "receipt_id",
        "executable_sha256",
        "source_head",
    ] {
        let value = required_string(manifest, field)?;
        if value.is_empty() {
            return Err(CanaryError::new(format!(
                "Croissant manifest omitted {field}"
            )));
        }
    }
    validate_manifest_bounds_and_teardown(manifest)
}

fn validate_manifest_bounds_and_teardown(manifest: &Value) -> CanaryResult<()> {
    let bounds = manifest
        .get("bounds")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("Croissant manifest omitted bounds"))?;
    for field in [
        "operation_ms",
        "wire_bytes",
        "log_bytes",
        "readiness_ms",
        "readiness_stability_ms",
        "teardown_ms",
    ] {
        if bounds.get(field).and_then(Value::as_u64).unwrap_or(0) == 0 {
            return Err(CanaryError::new(format!(
                "Croissant manifest omitted {field} bound"
            )));
        }
    }
    if bounds
        .get("wire_bytes_observed")
        .and_then(Value::as_u64)
        .unwrap_or(u64::MAX)
        > bounds["wire_bytes"].as_u64().unwrap_or(0)
    {
        return Err(CanaryError::new(
            "Croissant wire evidence exceeded its bound",
        ));
    }
    let teardown = manifest
        .get("teardown")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("Croissant manifest omitted teardown"))?;
    if teardown.get("completed").and_then(Value::as_bool) != Some(true)
        || teardown.get("pid_alive_after").and_then(Value::as_bool) != Some(false)
        || teardown.get("port_open_after").and_then(Value::as_bool) != Some(false)
    {
        return Err(CanaryError::new("Croissant teardown remained incomplete"));
    }
    let pid = teardown.get("pid").and_then(Value::as_u64).unwrap_or(0);
    if pid == 0 || process_is_alive(u32::try_from(pid).unwrap_or(u32::MAX)) {
        return Err(CanaryError::new(
            "Croissant child pid remains live or invalid",
        ));
    }
    let endpoint: SocketAddr = teardown
        .get("endpoint")
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new("Croissant teardown omitted endpoint"))?
        .parse()
        .map_err(|_| CanaryError::new("Croissant endpoint was invalid"))?;
    if TcpStream::connect_timeout(&endpoint, Duration::from_millis(25)).is_ok() {
        return Err(CanaryError::new("Croissant teardown port remains open"));
    }
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("Croissant manifest omitted {field}")))
}

fn reject_secret_fields(value: &Value) -> CanaryResult<()> {
    match value {
        Value::Object(map) => {
            let forbidden = ["scenario_seed", "raw_seed", "private_key", "secret_key"];
            if map.keys().any(|key| forbidden.contains(&key.as_str())) {
                return Err(CanaryError::new(
                    "Croissant manifest contained a secret field",
                ));
            }
            for child in map.values() {
                reject_secret_fields(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_secret_fields(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}

#[cfg(test)]
#[path = "croissant_nip02_tests.rs"]
mod tests;
