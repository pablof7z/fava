//! Bounded process, observation, wire, and artifact helpers for M5 canaries.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use fava::{Fava, Observation, Receipt, ReceiptId, Write, all_terminal};
use serde_json::{Value, json};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::relay::ProcessFact;
use crate::{CanaryError, CanaryResult, SmokeOptions, command_output, repository_root};

pub(crate) async fn wait_terminal(write: &Write) -> CanaryResult<Receipt> {
    tokio::time::timeout(Duration::from_secs(10), write.settled(all_terminal()))
        .await
        .map_err(|_| CanaryError::new("terminal receipt deadline elapsed"))?
        .map_err(error)
}

pub(crate) async fn wait_recovered_terminal(
    fava: &Fava,
    receipt_id: ReceiptId,
) -> CanaryResult<Receipt> {
    let mut changes = fava.receipt_changes();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(receipt) = fava.receipt(receipt_id).map_err(error)?
                && receipt.is_terminal()
            {
                return Ok(receipt);
            }
            match changes.recv().await {
                Ok((changed_id, Some(receipt)))
                    if changed_id == receipt_id && receipt.is_terminal() =>
                {
                    return Ok(receipt);
                }
                Ok((changed_id, None)) if changed_id == receipt_id => {
                    return Err(CanaryError::new(
                        "recovered receipt removed before terminal state",
                    ));
                }
                Ok(_) => {}
                Err(error) => {
                    return Err(CanaryError::new(format!(
                        "recovered receipt change delivery failed explicitly: {error}"
                    )));
                }
            }
        }
    })
    .await
    .map_err(|_| CanaryError::new("recovered terminal receipt deadline elapsed"))?
}

pub(crate) async fn next_receipt(
    changes: &mut broadcast::Receiver<(ReceiptId, Option<Receipt>)>,
    expected: ReceiptId,
) -> CanaryResult<Receipt> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match changes.recv().await {
                Ok((id, Some(receipt))) if id == expected => return Ok(receipt),
                Ok((id, None)) if id == expected => {
                    return Err(CanaryError::new("receipt removed before expected change"));
                }
                Ok(_) => {}
                Err(error) => {
                    return Err(CanaryError::new(format!(
                        "receipt change delivery failed explicitly: {error}"
                    )));
                }
            }
        }
    })
    .await
    .map_err(|_| CanaryError::new("receipt change deadline elapsed"))?
}

pub(crate) async fn wait_record(
    observation: &mut Observation,
    event_id: fava_write::EventId,
    relay_count: usize,
) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if observation.current().events.iter().any(|event| {
                event.id() == event_id && event.relay_occurrences().len() == relay_count
            }) {
                return Ok(());
            }
            observation.changed().await.map_err(error)?;
        }
    })
    .await
    .map_err(|_| CanaryError::new("query evidence deadline elapsed"))?
}

pub(crate) async fn wait_empty(observation: &mut Observation) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !observation.current().events.is_empty() {
            observation.changed().await.map_err(error)?;
        }
        Ok(())
    })
    .await
    .map_err(|_| CanaryError::new("query retraction deadline elapsed"))?
}

pub(crate) async fn wait_until(predicate: impl Fn() -> bool) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("provider call deadline elapsed"))
}

pub(crate) fn spawn_crash_child(
    database: &Path,
    marker: &Path,
    relay: &str,
    seed: &str,
    root: &Path,
) -> CanaryResult<Child> {
    let stdout = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("children/crash-child.stdout.log"))?;
    let stderr = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(root.join("children/crash-child.stderr.log"))?;
    Ok(Command::new(std::env::current_exe()?)
        .args([
            "crash-child",
            database.to_string_lossy().as_ref(),
            marker.to_string_lossy().as_ref(),
            relay,
            seed,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(true)
        .spawn()?)
}

pub(crate) async fn wait_child_marker(path: &Path, child: &mut Child) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if path.exists() {
                return Ok(());
            }
            if let Some(status) = child.try_wait()? {
                return Err(CanaryError::new(format!(
                    "crash child exited before acceptance marker: {status}"
                )));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| CanaryError::new("crash child acceptance deadline elapsed"))?
}

pub(crate) async fn wait_wire(path: &Path, message: &str, count: usize) -> CanaryResult<()> {
    tokio::time::timeout(Duration::from_secs(5), async {
        while wire_count(path, message)? < count {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Ok(())
    })
    .await
    .map_err(|_| CanaryError::new(format!("wire {message} deadline elapsed")))?
}

pub(crate) fn wire_count(path: &Path, message: &str) -> CanaryResult<usize> {
    Ok(fs::read_to_string(path)?
        .lines()
        .filter(|line| {
            line.contains("client_to_relay") && line.contains(&format!("\\\"{message}\\\""))
        })
        .count())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finish(
    mut artifacts: RunArtifacts,
    scenario: &str,
    options: &SmokeOptions,
    started: u128,
    version: &str,
    processes: &[ProcessFact],
    event_id: &str,
    receipt_id: u64,
    details: &Value,
) -> CanaryResult<PathBuf> {
    artifacts.record(
        "scenario_passed",
        json!({ "scenario": scenario, "event_id": event_id, "receipt_id": receipt_id, "details": details }),
    )?;
    artifacts.write_json("relays/nostr-rs-relay/process.json", &processes)?;
    artifacts.write_report(&format!(
        "# Canary report\n\n- Scenario: {scenario}\n- Result: passed\n- Relay: {version}\n- Event: {event_id}\n- Receipt: {receipt_id}\n"
    ))?;
    let repository = repository_root()?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": artifacts.run_id()?, "scenario": scenario, "scenario_seed": options.seed,
            "selected_profile": "redb-durable-write-store+nostr-rs-relay-0.8.12",
            "fava_revision": revision, "canary_revision": revision, "dirty": dirty,
            "relay_implementation": "nostr-rs-relay", "relay_version": version,
            "started_unix_ms": started, "ended_unix_ms": unix_ms()?, "artifact_sha256": hashes,
        }),
    )?;
    Ok(artifacts.root().to_owned())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
