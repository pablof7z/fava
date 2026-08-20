//! Read-only public-relay reconnaissance.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::artifacts::{RunArtifacts, unix_ms};
use crate::{CanaryError, CanaryResult, command_output, repository_root, wire};

/// Inputs for one bounded read-only public-relay observation.
#[derive(Clone, Debug)]
pub struct ReconOptions {
    /// Explicit WebSocket relay URL. No default public relay exists.
    pub relay_url: String,
    /// Caller-selected deterministic run seed.
    pub seed: String,
    /// Parent directory for preserved evidence bundles.
    pub runs_directory: PathBuf,
}

/// Evidence-only public-relay reconnaissance result.
#[derive(Clone, Debug)]
pub struct ReconOutcome {
    /// Evidence bundle directory.
    pub run_directory: PathBuf,
    /// Bounded terminal observation such as EOSE, deadline, or relay terminal.
    pub terminal: String,
    /// Number of relay frames preserved.
    pub frame_count: usize,
}

pub(crate) async fn run(options: ReconOptions) -> CanaryResult<ReconOutcome> {
    if options.relay_url.trim().is_empty() {
        return Err(CanaryError::new(
            "public-relay reconnaissance requires an explicit relay URL",
        ));
    }
    let scenario = "public-relay-recon";
    let mut artifacts = RunArtifacts::create(&options.runs_directory, scenario, &options.seed)?;
    let started = unix_ms()?;
    artifacts.record(
        "recon_started",
        json!({ "relay": options.relay_url, "seed": options.seed }),
    )?;

    let witness = match wire::reconnaissance(&options.relay_url, "public-recon").await {
        Ok(witness) => witness,
        Err(error) => {
            artifacts.record(
                "recon_failed",
                json!({ "relay": options.relay_url, "error": error.to_string() }),
            )?;
            artifacts.write_report(&format!(
                "# Public-relay reconnaissance\n\n- Relay: {}\n- Result: external failure\n- Error: {}\n",
                options.relay_url, error
            ))?;
            let ended = unix_ms()?;
            let hashes = artifacts.artifact_hashes()?;
            let manifest = ReconManifest::collect(
                &options,
                artifacts.root(),
                "external-failure",
                started,
                ended,
                hashes,
            )?;
            artifacts.write_json("manifest.json", &manifest)?;
            return Err(error);
        }
    };
    for frame in &witness.frames {
        artifacts.record("relay_frame", frame)?;
    }
    artifacts.record(
        "recon_completed",
        json!({ "terminal": witness.terminal, "frame_count": witness.frames.len() }),
    )?;
    artifacts.write_report(&format!(
        "# Public-relay reconnaissance\n\n- Relay: {}\n- Classification: evidence only\n- Terminal observation: {}\n- Preserved frames: {}\n",
        options.relay_url,
        witness.terminal,
        witness.frames.len()
    ))?;
    let ended = unix_ms()?;
    let hashes = artifacts.artifact_hashes()?;
    let manifest = ReconManifest::collect(
        &options,
        artifacts.root(),
        "reconnaissance",
        started,
        ended,
        hashes,
    )?;
    artifacts.write_json("manifest.json", &manifest)?;
    Ok(ReconOutcome {
        run_directory: artifacts.root().to_owned(),
        terminal: witness.terminal.to_owned(),
        frame_count: witness.frames.len(),
    })
}

#[derive(Serialize)]
struct ReconManifest<'a> {
    run_id: String,
    scenario: &'static str,
    scenario_seed: &'a str,
    classification: &'static str,
    relay_url: &'a str,
    revision: String,
    dirty: bool,
    started_unix_ms: u128,
    ended_unix_ms: u128,
    artifact_sha256: BTreeMap<String, String>,
}

impl<'a> ReconManifest<'a> {
    fn collect(
        options: &'a ReconOptions,
        run_directory: &Path,
        classification: &'static str,
        started_unix_ms: u128,
        ended_unix_ms: u128,
        artifact_sha256: BTreeMap<String, String>,
    ) -> CanaryResult<Self> {
        let repository = repository_root()?;
        let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
        let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
        let run_id = run_directory
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| CanaryError::new("run directory has no UTF-8 identifier"))?
            .to_owned();
        Ok(Self {
            run_id,
            scenario: "public-relay-recon",
            scenario_seed: &options.seed,
            classification,
            relay_url: &options.relay_url,
            revision,
            dirty,
            started_unix_ms,
            ended_unix_ms,
            artifact_sha256,
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{ReconOptions, run};

    #[tokio::test]
    async fn public_recon_requires_an_explicit_relay_url() {
        let runs_directory = tempdir().expect("temporary runs directory");
        let error = run(ReconOptions {
            relay_url: String::new(),
            seed: "seed".to_owned(),
            runs_directory: runs_directory.path().to_owned(),
        })
        .await
        .expect_err("empty relay URL is refused");
        assert!(error.to_string().contains("explicit relay URL"));
    }
}
