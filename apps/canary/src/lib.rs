//! Independent acceptance application and evidence lab for the Fava rewrite.

mod artifacts;
mod automatic_publication;
mod automatic_support;
mod grouping;
mod hostile;
mod live;
mod local;
mod multi;
mod proxy;
mod publication;
mod publication_child;
mod publication_support;
mod recon;
mod relay;
mod routing;
mod wire;

pub use automatic_publication::run_automatic_publication_scenario;
pub use grouping::run_grouping_scenario;
pub use live::run_live_scenario;
pub use local::run_local_scenario;
pub use multi::run_m3_live_scenario;
pub use publication::run_publication_scenario;
pub use publication_child::run_crash_child;
pub use recon::{ReconOptions, ReconOutcome};
pub use routing::run_routing_scenario;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use artifacts::{RunArtifacts, unix_ms};
use nostr::event::{Event, EventBuilder, FinalizeEvent, Kind};
use nostr::key::Keys;
use nostr::types::Timestamp;
use proxy::WireProxy;
use relay::{ProcessFact, RelaySupervisor};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

/// Error returned by canary orchestration, process, wire, or evidence work.
#[derive(Debug)]
pub struct CanaryError(String);

impl CanaryError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for CanaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CanaryError {}

impl From<std::io::Error> for CanaryError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<std::string::FromUtf8Error> for CanaryError {
    fn from(error: std::string::FromUtf8Error) -> Self {
        Self(error.to_string())
    }
}

impl From<std::time::SystemTimeError> for CanaryError {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self(error.to_string())
    }
}

impl From<std::path::StripPrefixError> for CanaryError {
    fn from(error: std::path::StripPrefixError) -> Self {
        Self(error.to_string())
    }
}

impl From<serde_json::Error> for CanaryError {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for CanaryError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<nostr::error::Error> for CanaryError {
    fn from(error: nostr::error::Error) -> Self {
        Self(error.to_string())
    }
}

/// Result type used by the canary.
pub type CanaryResult<T> = Result<T, CanaryError>;

/// One canary scenario known to this build.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Scenario {
    /// Stable scenario identifier.
    pub id: String,
    /// Rewrite milestone that owns the scenario.
    pub milestone: String,
    /// Requirements or authority owned by the scenario.
    pub requirements: Vec<String>,
    /// Registry classification such as enabled or reconnaissance.
    pub status: String,
}

#[derive(Deserialize)]
struct ScenarioRegistry {
    scenarios: Vec<Scenario>,
}

/// Reads the single scenario registry embedded in this build.
///
/// # Errors
///
/// Returns an error when the checked-in registry is malformed.
pub fn scenario_registry() -> CanaryResult<Vec<Scenario>> {
    Ok(serde_json::from_str::<ScenarioRegistry>(include_str!("../scenarios.json"))?.scenarios)
}

/// Returns whether this build has an executor for the identifier.
#[must_use]
pub fn has_executor(id: &str) -> bool {
    matches!(
        id,
        "lab-real-relay-smoke"
            | "public-relay-recon"
            | "local-source-merge"
            | "local-replaceable-shadow-and-cancel"
            | "local-source-removal"
            | "explicit-read-eose"
            | "explicit-read-live-after-eose"
            | "explicit-read-cancel"
            | "multi-relay-dedup-provenance"
            | "reconnect-generation"
            | "slow-consumer-latest-state"
            | "async-route-partial-read"
            | "explicit-route-bypass"
            | "fallback-reacts"
            | "subscription-grouping-equivalence"
            | "explicit-publish-optimistic"
            | "mixed-relay-outcomes"
            | "cancel-pre-handoff"
            | "crash-after-acceptance"
            | "async-recipient-routing"
            | "hint-routing"
            | "route-preview-parity"
            | "app-relay-versus-fallback-profile"
    )
}

/// Runs bounded read-only reconnaissance against an explicit public relay.
///
/// # Errors
///
/// Returns an error when inputs are invalid or connection, protocol,
/// verification, or evidence persistence fails.
pub async fn run_public_recon(options: ReconOptions) -> CanaryResult<ReconOutcome> {
    recon::run(options).await
}

/// Inputs for the deterministic real-relay smoke scenario.
#[derive(Clone, Debug)]
pub struct SmokeOptions {
    /// Pinned third-party relay executable.
    pub relay_binary: PathBuf,
    /// Caller-selected seed used to derive run and identity values.
    pub seed: String,
    /// Parent directory for preserved evidence bundles.
    pub runs_directory: PathBuf,
}

/// Successful real-relay smoke result.
#[derive(Clone, Debug)]
pub struct SmokeOutcome {
    /// Evidence bundle directory.
    pub run_directory: PathBuf,
    /// Exact event proven before and after restart.
    pub event_id: String,
}

/// Runs the lab-real-relay-smoke scenario.
///
/// # Errors
///
/// Returns an error when the pinned relay is unavailable or when process,
/// protocol, persistence, witness, or evidence checks fail.
pub async fn run_real_relay_smoke(options: SmokeOptions) -> CanaryResult<SmokeOutcome> {
    let scenario = "lab-real-relay-smoke";
    let mut artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    artifacts.append_app_stdout(&format!("starting {scenario}"))?;
    artifacts.record(
        "scenario_started",
        json!({ "scenario": scenario, "seed": options.seed }),
    )?;
    let started_at = unix_ms()?;

    match execute_smoke(&mut artifacts, &options).await {
        Ok(completed) => finish_success(artifacts, scenario, &options, started_at, completed),
        Err(error) => {
            let _ = artifacts.append_app_stderr(&error.to_string());
            let _ = artifacts.record(
                "scenario_failed",
                json!({ "scenario": scenario, "error": error.to_string() }),
            );
            let _ = artifacts.write_report(&format!(
                "# Canary report\n\n- Scenario: {scenario}\n- Result: failed\n- Error: {error}\n"
            ));
            Err(error)
        }
    }
}

fn finish_success(
    mut artifacts: RunArtifacts,
    scenario: &str,
    options: &SmokeOptions,
    started_at: u128,
    completed: CompletedSmoke,
) -> CanaryResult<SmokeOutcome> {
    let ended_at = unix_ms()?;
    artifacts.record(
        "scenario_passed",
        json!({ "scenario": scenario, "event_id": completed.event_id }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &completed.processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {}\n- Event: {}\n- First query: event + EOSE\n- Post-restart query: event + EOSE\n- Restart storage: same isolated data directory\n",
        completed.relay_version, completed.event_id
    ))?;
    artifacts.append_app_stdout(&format!(
        "passed {scenario} for event {}",
        completed.event_id
    ))?;
    let run_id = artifacts.run_id()?;
    let hashes = artifacts.artifact_hashes()?;
    let manifest = Manifest::collect(
        run_id, scenario, options, started_at, ended_at, &completed, hashes,
    )?;
    artifacts.write_json("manifest.json", &manifest)?;
    Ok(SmokeOutcome {
        run_directory: artifacts.root().to_owned(),
        event_id: completed.event_id,
    })
}

struct CompletedSmoke {
    event_id: String,
    relay_version: String,
    relay_command: String,
    proxy_url: String,
    processes: Vec<ProcessFact>,
}

async fn execute_smoke(
    artifacts: &mut RunArtifacts,
    options: &SmokeOptions,
) -> CanaryResult<CompletedSmoke> {
    let relay_port = reserve_port().await?;
    let supervisor =
        RelaySupervisor::prepare(&options.relay_binary, &artifacts.relay_dir(), relay_port)?;
    let relay_version = supervisor.version().await?;
    artifacts.record(
        "relay_prerequisite_verified",
        json!({ "version": relay_version, "binary": options.relay_binary }),
    )?;
    let proxy = WireProxy::start(supervisor.address(), &artifacts.wire_log()).await?;
    let proxy_url = proxy.url();
    let mut processes = Vec::new();

    let first = supervisor.spawn(1).await?;
    processes.push(first.fact("ready"));
    artifacts.record("relay_ready", first.fact("ready"))?;
    artifacts.record_resource(first.pid(), 1)?;

    let event = create_event(&options.seed)?;
    artifacts.record(
        "signed_event_created",
        json!({ "event_id": event.id.to_hex(), "pubkey": event.pubkey.to_hex() }),
    )?;
    let acknowledgement = wire::publish(&proxy_url, &event).await?;
    artifacts.record(
        "relay_acknowledged_event",
        json!({ "event_id": event.id.to_hex(), "message": acknowledgement }),
    )?;
    let first_query = wire::query_exact(&proxy_url, event.id, "m0-before-restart").await?;
    require_complete_query("pre-restart", first_query)?;
    artifacts.record("pre_restart_query_completed", first_query)?;

    let killed = first.hard_kill().await?;
    processes.push(killed);
    artifacts.record("relay_hard_killed", killed)?;

    let second = supervisor.spawn(2).await?;
    processes.push(second.fact("ready"));
    artifacts.record("relay_restarted", second.fact("ready"))?;
    artifacts.record_resource(second.pid(), 2)?;
    let second_query = wire::query_exact(&proxy_url, event.id, "m0-after-restart").await?;
    require_complete_query("post-restart", second_query)?;
    artifacts.record("post_restart_query_completed", second_query)?;

    let stopped = second.graceful_stop().await?;
    processes.push(stopped);
    artifacts.record("relay_gracefully_stopped", stopped)?;
    proxy.shutdown().await?;

    Ok(CompletedSmoke {
        event_id: event.id.to_hex(),
        relay_version,
        relay_command: format!(
            "{} --config <run>/relays/nostr-rs-relay/config.toml --db <run>/relays/nostr-rs-relay/data",
            options.relay_binary.display()
        ),
        proxy_url,
        processes,
    })
}

fn create_event(seed: &str) -> CanaryResult<Event> {
    let keys = deterministic_keys(seed)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(
        EventBuilder::new(Kind::TextNote, format!("Fava M0 canary {seed}"))
            .custom_created_at(Timestamp::from(now))
            .finalize(&keys)?,
    )
}

fn require_complete_query(label: &str, witness: wire::QueryWitness) -> CanaryResult<()> {
    if !witness.found_event || !witness.saw_eose {
        return Err(CanaryError::new(format!(
            "{label} exact query was incomplete: event={}, eose={}",
            witness.found_event, witness.saw_eose
        )));
    }
    Ok(())
}

async fn reserve_port() -> CanaryResult<u16> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn deterministic_keys(seed: &str) -> CanaryResult<Keys> {
    for counter in 0_u64..1024 {
        let digest = Sha256::digest(format!("fava-m0-identity\0{seed}\0{counter}"));
        if let Ok(keys) = Keys::parse(&hex::encode(digest)) {
            return Ok(keys);
        }
    }
    Err(CanaryError::new(
        "could not derive a valid disposable identity from the seed",
    ))
}

#[derive(Serialize)]
struct Manifest<'a> {
    run_id: String,
    scenario: &'a str,
    scenario_seed: &'a str,
    selected_profile: &'a str,
    fava_revision: String,
    canary_revision: String,
    dirty: bool,
    relay_implementation: &'a str,
    relay_version: &'a str,
    relay_command: &'a str,
    relay_processes: &'a [ProcessFact],
    proxy_url: &'a str,
    platform: String,
    toolchain: String,
    started_unix_ms: u128,
    ended_unix_ms: u128,
    artifact_sha256: BTreeMap<String, String>,
}

impl<'a> Manifest<'a> {
    fn collect(
        run_id: String,
        scenario: &'a str,
        options: &'a SmokeOptions,
        started_unix_ms: u128,
        ended_unix_ms: u128,
        completed: &'a CompletedSmoke,
        artifact_sha256: BTreeMap<String, String>,
    ) -> CanaryResult<Self> {
        let repository = repository_root()?;
        let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
        let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
        Ok(Self {
            run_id,
            scenario,
            scenario_seed: &options.seed,
            selected_profile: "nostr-rs-relay-0.8.12-local-process",
            fava_revision: revision.clone(),
            canary_revision: revision,
            dirty,
            relay_implementation: "nostr-rs-relay",
            relay_version: &completed.relay_version,
            relay_command: &completed.relay_command,
            relay_processes: &completed.processes,
            proxy_url: &completed.proxy_url,
            platform: command_output(&repository, "uname", &["-a"])?,
            toolchain: command_output(&repository, "rustc", &["--version"])?,
            started_unix_ms,
            ended_unix_ms,
            artifact_sha256,
        })
    }
}

pub(crate) fn repository_root() -> CanaryResult<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_owned)
        .ok_or_else(|| CanaryError::new("canary manifest is not under apps/canary"))
}

pub(crate) fn command_output(
    directory: &Path,
    command: &str,
    arguments: &[&str],
) -> CanaryResult<String> {
    let output = Command::new(command)
        .args(arguments)
        .current_dir(directory)
        .output()?;
    if !output.status.success() {
        return Err(CanaryError::new(format!(
            "{command} {} failed with {}",
            arguments.join(" "),
            output.status
        )));
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{deterministic_keys, has_executor, run_local_scenario, scenario_registry};

    #[test]
    fn every_enabled_scenario_has_an_executor() {
        let scenarios = scenario_registry().expect("registry parses");
        for scenario in scenarios
            .iter()
            .filter(|scenario| scenario.status == "enabled")
        {
            assert!(
                has_executor(&scenario.id),
                "missing executor for {}",
                scenario.id
            );
        }
    }

    #[tokio::test]
    async fn local_scenarios_pass_through_the_public_facade() {
        for scenario in [
            "local-source-merge",
            "local-replaceable-shadow-and-cancel",
            "local-source-removal",
            "slow-consumer-latest-state",
        ] {
            run_local_scenario(scenario, "m1-test")
                .await
                .expect("local scenario passes");
        }
    }

    #[test]
    fn disposable_identity_is_seed_deterministic() {
        let first = deterministic_keys("seed").expect("identity derives");
        let second = deterministic_keys("seed").expect("identity derives");
        assert_eq!(first.public_key(), second.public_key());
    }
}
