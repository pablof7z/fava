//! Durable, bounded failure evidence for semantic-write canaries.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::artifacts::RunArtifacts;
use crate::semantic_write_support::seed_hash;
use crate::{CanaryResult, SmokeOptions, command_output, repository_root};

const FAILURE_DETAIL_CAPACITY: usize = 65_536;

pub(super) fn write_failure_bundle(
    mut artifacts: RunArtifacts,
    id: &str,
    options: &SmokeOptions,
    failure: &str,
) -> CanaryResult<PathBuf> {
    let failure = bounded_failure(failure);
    let repository = repository_root()?;
    artifacts.record("scenario_failed", json!({"scenario": id, "error": failure}))?;
    artifacts.write_json("failure.json", &json!({"scenario": id, "error": failure}))?;
    artifacts.write_json(
        "replay.json",
        &json!({
            "program": "cargo",
            "current_directory": repository.to_string_lossy(),
            "args": [
                "run", "--manifest-path", "apps/canary/Cargo.toml", "--", "run", id,
                "--seed", options.seed,
                "--runs-dir", artifacts.root().join("replay").to_string_lossy(),
            ],
        }),
    )?;
    artifacts.write_report(&format!("{id} failed: {failure}\n"))?;
    let revision = command_output(&repository, "git", &["rev-parse", "HEAD"])?;
    let dirty = !command_output(&repository, "git", &["status", "--porcelain"])?.is_empty();
    let artifact_hashes = artifacts.artifact_hashes()?;
    artifacts.write_json(
        "manifest.json",
        &json!({
            "run_id": artifacts.run_id()?, "scenario": id,
            "outcome": "failed", "scenario_seed_sha256": seed_hash(&options.seed),
            "selected_profile": "memory-public-fava", "fava_revision": revision,
            "canary_revision": revision, "dirty": dirty,
            "relay_implementation": Value::Null, "artifact_sha256": artifact_hashes,
        }),
    )?;
    Ok(artifacts.root().to_path_buf())
}

fn bounded_failure(failure: &str) -> String {
    if failure.len() <= FAILURE_DETAIL_CAPACITY {
        return failure.to_owned();
    }
    let mut end = FAILURE_DETAIL_CAPACITY;
    while !failure.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{} [truncated at {FAILURE_DETAIL_CAPACITY} bytes]",
        &failure[..end]
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::write_failure_bundle;
    use crate::SmokeOptions;
    use crate::artifacts::RunArtifacts;

    #[test]
    fn failure_bundle_is_durable_and_replayable() {
        let directory = TempDir::new().expect("temporary run directory");
        let options = SmokeOptions {
            runs_directory: directory.path().to_path_buf(),
            seed: "failure-seed".to_owned(),
            relay_binary: PathBuf::from("unused"),
        };
        let artifacts = RunArtifacts::create(
            &options.runs_directory,
            "replaceable-edit-first-value",
            &options.seed,
        )
        .expect("failure artifacts");
        let root = write_failure_bundle(
            artifacts,
            "replaceable-edit-first-value",
            &options,
            "deliberate failure",
        )
        .expect("durable failure bundle");
        for file in ["failure.json", "replay.json", "report.md", "manifest.json"] {
            assert!(root.join(file).is_file(), "missing {file}");
        }
        let replay: Value = serde_json::from_slice(
            &std::fs::read(root.join("replay.json")).expect("replay evidence"),
        )
        .expect("valid replay evidence");
        assert_eq!(replay["program"], "cargo");
        assert!(replay["current_directory"].as_str().is_some());
        let bundle = ["failure.json", "replay.json", "report.md", "manifest.json"]
            .into_iter()
            .map(|file| std::fs::read(root.join(file)).expect("failure evidence"))
            .flatten()
            .collect::<Vec<_>>();
        assert!(!String::from_utf8_lossy(&bundle).contains("failure-seed"));
        assert_eq!(replay["redacted_inputs"], serde_json::json!(["seed"]));
        assert_eq!(
            replay["scenario_seed_sha256"],
            crate::semantic_write_support::seed_hash("failure-seed")
        );
        assert!(
            replay["args"]
                .as_array()
                .expect("bounded arguments")
                .iter()
                .any(|argument| argument
                    .as_str()
                    .is_some_and(|argument| argument.ends_with("/replay")))
        );
    }
}
