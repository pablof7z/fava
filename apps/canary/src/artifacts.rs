//! Evidence bundle creation for canary runs.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{CanaryError, CanaryResult};

pub(crate) struct RunArtifacts {
    root: PathBuf,
    retained_root: Option<PathBuf>,
    staging_parent: Option<PathBuf>,
    run_id: String,
    evidence: File,
    sequence: u64,
}

impl RunArtifacts {
    pub(crate) fn create(runs_dir: &Path, scenario: &str, seed: &str) -> CanaryResult<Self> {
        let run_id = run_id(scenario, seed);
        let root = runs_dir.join(run_id);
        Self::create_at(root, None, None)
    }

    /// Create owner-private evidence outside the retained root until explicit promotion.
    pub(crate) fn create_staged(runs_dir: &Path, scenario: &str, seed: &str) -> CanaryResult<Self> {
        fs::create_dir_all(runs_dir)?;
        let run_id = run_id(scenario, seed);
        let retained_root = runs_dir.join(&run_id);
        if retained_root.exists() {
            return Err(CanaryError::new(format!(
                "run directory already exists: {}; choose another seed",
                retained_root.display()
            )));
        }
        let staging_parent = create_private_staging_parent(runs_dir)?;
        let staging_root = staging_parent.join(&run_id);
        match Self::create_at(
            staging_root,
            Some(retained_root),
            Some(staging_parent.clone()),
        ) {
            Ok(artifacts) => Ok(artifacts),
            Err(error) => {
                let _ = fs::remove_dir_all(staging_parent);
                Err(error)
            }
        }
    }

    fn create_at(
        root: PathBuf,
        retained_root: Option<PathBuf>,
        staging_parent: Option<PathBuf>,
    ) -> CanaryResult<Self> {
        if root.exists() {
            return Err(CanaryError::new(format!(
                "run directory already exists: {}; choose another seed",
                root.display()
            )));
        }
        fs::create_dir_all(root.join("relays/nostr-rs-relay/data"))?;
        fs::create_dir_all(root.join("wire"))?;
        fs::create_dir_all(root.join("children"))?;
        File::create(root.join("app.stdout.log"))?;
        File::create(root.join("app.stderr.log"))?;
        File::create(root.join("resources.csv"))?.write_all(b"unix_ms,pid,rss_kib,generation\n")?;
        let evidence = File::create(root.join("evidence.jsonl"))?;
        let run_id = root
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned)
            .ok_or_else(|| CanaryError::new("run directory has no UTF-8 identifier"))?;
        Ok(Self {
            root,
            retained_root,
            staging_parent,
            run_id,
            evidence,
            sequence: 0,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn run_id(&self) -> CanaryResult<String> {
        Ok(self.run_id.clone())
    }

    pub(crate) fn relay_dir(&self) -> PathBuf {
        self.root.join("relays/nostr-rs-relay")
    }

    pub(crate) fn wire_log(&self) -> PathBuf {
        self.root.join("wire/proxy.jsonl")
    }

    pub(crate) fn record<T: Serialize>(&mut self, kind: &str, data: T) -> CanaryResult<()> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| CanaryError::new("evidence sequence exhausted"))?;
        let line = EvidenceLine {
            sequence: self.sequence,
            unix_ms: unix_ms()?,
            kind,
            data,
        };
        serde_json::to_writer(&mut self.evidence, &line)?;
        self.evidence.write_all(b"\n")?;
        self.evidence.flush()?;
        Ok(())
    }

    pub(crate) fn append_app_stdout(&self, line: &str) -> CanaryResult<()> {
        append_line(&self.root.join("app.stdout.log"), line)
    }

    pub(crate) fn append_app_stderr(&self, line: &str) -> CanaryResult<()> {
        append_line(&self.root.join("app.stderr.log"), line)
    }

    pub(crate) fn record_resource(&self, pid: u32, generation: u64) -> CanaryResult<()> {
        let output = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()?;
        let rss = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        append_line(
            &self.root.join("resources.csv"),
            &format!("{},{pid},{rss},{generation}", unix_ms()?),
        )
    }

    pub(crate) fn write_json<T: Serialize>(&self, relative: &str, value: &T) -> CanaryResult<()> {
        let path = self.root.join(relative);
        let bytes = serde_json::to_vec_pretty(value)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub(crate) fn write_report(&self, report: &str) -> CanaryResult<()> {
        fs::write(self.root.join("report.md"), report)?;
        Ok(())
    }

    pub(crate) fn artifact_hashes(&self) -> CanaryResult<BTreeMap<String, String>> {
        let mut files = Vec::new();
        collect_files(&self.root, &self.root, &mut files)?;
        files.sort();
        let mut hashes = BTreeMap::new();
        for relative in files {
            if relative == Path::new("manifest.json") {
                continue;
            }
            let bytes = fs::read(self.root.join(&relative))?;
            hashes.insert(
                relative.to_string_lossy().into_owned(),
                hex::encode(Sha256::digest(bytes)),
            );
        }
        Ok(hashes)
    }

    /// Atomically publish a fully scanned staged run beneath its retained evidence root.
    pub(crate) fn promote(mut self) -> CanaryResult<PathBuf> {
        self.evidence.flush()?;
        let retained_root = self
            .retained_root
            .clone()
            .ok_or_else(|| CanaryError::new("run artifacts were not created in staging"))?;
        let staging_parent = self
            .staging_parent
            .clone()
            .ok_or_else(|| CanaryError::new("staged run omitted its private parent"))?;
        if retained_root.exists() {
            return Err(CanaryError::new(format!(
                "run directory already exists: {}; choose another seed",
                retained_root.display()
            )));
        }
        fs::rename(&self.root, &retained_root)?;
        if let Err(error) = fs::remove_dir(&staging_parent) {
            let _ = fs::remove_dir_all(&retained_root);
            return Err(error.into());
        }
        self.root = retained_root.clone();
        self.retained_root = None;
        self.staging_parent = None;
        Ok(retained_root)
    }
}

impl Drop for RunArtifacts {
    fn drop(&mut self) {
        if let Some(staging_parent) = self.staging_parent.take() {
            let _ = fs::remove_dir_all(staging_parent);
        }
    }
}

#[derive(Serialize)]
struct EvidenceLine<'a, T> {
    sequence: u64,
    unix_ms: u128,
    kind: &'a str,
    data: T,
}

fn run_id(scenario: &str, seed: &str) -> String {
    let digest = Sha256::digest(format!("{scenario}\0{seed}"));
    format!("{scenario}-{}", &hex::encode(digest)[..16])
}

fn create_private_staging_parent(runs_dir: &Path) -> CanaryResult<PathBuf> {
    let parent = runs_dir
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    for attempt in 0_u8..32 {
        let path = parent.join(format!(
            ".fava-canary-staging-{}-{}-{attempt}",
            process::id(),
            unix_ms()?
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(CanaryError::new(
        "could not allocate an owner-private canary staging directory",
    ))
}

pub(crate) fn unix_ms() -> CanaryResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn append_line(path: &Path, line: &str) -> CanaryResult<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> CanaryResult<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(path.strip_prefix(root)?.to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_id;

    #[test]
    fn run_identity_is_stable_and_seed_scoped() {
        assert_eq!(run_id("scenario", "seed"), run_id("scenario", "seed"));
        assert_ne!(run_id("scenario", "seed"), run_id("scenario", "other"));
    }
}
