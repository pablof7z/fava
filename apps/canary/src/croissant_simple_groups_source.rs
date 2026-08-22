//! Exact committed-source provenance for the simple-groups live proof.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{CanaryError, CanaryResult};

const SOURCE_PATHS: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "apps/canary",
    "crates",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct FavaSourceProvenance {
    #[serde(rename = "fava_revision")]
    pub(crate) revision: String,
    #[serde(rename = "fava_source_tree_sha256")]
    pub(crate) tree_sha256: String,
    #[serde(rename = "fava_source_clean")]
    pub(crate) clean: bool,
    #[serde(rename = "fava_canary_executable_sha256")]
    pub(crate) canary_executable_sha256: String,
}

pub(crate) fn clean_fava_source(
    root: &Path,
    expected_canary_executable_sha256: &str,
) -> CanaryResult<FavaSourceProvenance> {
    let mut status_arguments = vec!["status", "--porcelain=v1", "--untracked-files=all", "--"];
    status_arguments.extend(SOURCE_PATHS);
    if !git(root, &status_arguments)?.is_empty() {
        return Err(CanaryError::new(
            "simple-groups Fava source/build inputs differed from committed HEAD",
        ));
    }
    let revision = text(git(root, &["rev-parse", "HEAD"])?)?;
    if !is_lower_hex(&revision, 40) {
        return Err(CanaryError::new(
            "simple-groups Fava source HEAD was not exact Git hex",
        ));
    }
    let mut tree_arguments = vec!["ls-tree", "-r", "--full-tree", "HEAD", "--"];
    tree_arguments.extend(SOURCE_PATHS);
    let tree = git(root, &tree_arguments)?;
    if tree.is_empty() {
        return Err(CanaryError::new(
            "simple-groups Fava committed source tree was empty",
        ));
    }
    Ok(FavaSourceProvenance {
        revision,
        tree_sha256: hex::encode(Sha256::digest(tree)),
        clean: true,
        canary_executable_sha256: exact_executable_sha256(
            &std::env::current_exe().map_err(error)?,
            expected_canary_executable_sha256,
        )?,
    })
}

fn exact_executable_sha256(path: &Path, expected: &str) -> CanaryResult<String> {
    if !is_lower_hex(expected, 64) {
        return Err(CanaryError::new(
            "simple-groups expected canary executable was not SHA-256 hex",
        ));
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(CanaryError::new(
            "simple-groups executing canary path was not a regular non-symlink file",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let actual = hex::encode(digest.finalize());
    if actual != expected {
        return Err(CanaryError::new(
            "simple-groups executing canary bytes differed from the expected executable",
        ));
    }
    Ok(actual)
}

#[cfg(test)]
fn current_executable_sha256() -> String {
    let bytes = fs::read(std::env::current_exe().expect("current executable")).expect("read exe");
    hex::encode(Sha256::digest(bytes))
}

fn git(root: &Path, arguments: &[&str]) -> CanaryResult<Vec<u8>> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(error)?;
    if !output.status.success() {
        return Err(CanaryError::new(
            "simple-groups Fava source provenance command failed",
        ));
    }
    Ok(output.stdout)
}

fn text(bytes: Vec<u8>) -> CanaryResult<String> {
    String::from_utf8(bytes)
        .map(|value| value.trim().to_owned())
        .map_err(error)
}

pub(crate) fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn error(error: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::{clean_fava_source, current_executable_sha256, exact_executable_sha256};

    #[test]
    fn production_source_gate_refuses_tracked_and_relevant_untracked_changes() {
        let executable = current_executable_sha256();
        let repository = TempDir::new().expect("source repository");
        fs::create_dir_all(repository.path().join("apps/canary/src")).expect("canary source");
        fs::create_dir_all(repository.path().join("crates/example/src")).expect("crate source");
        fs::create_dir_all(repository.path().join(".cargo")).expect("Cargo config directory");
        fs::write(repository.path().join("Cargo.toml"), "[workspace]\n").expect("manifest");
        fs::write(repository.path().join("Cargo.lock"), "version = 4\n").expect("lock");
        fs::write(repository.path().join("rust-toolchain.toml"), "[toolchain]\nchannel='stable'\n")
            .expect("toolchain");
        fs::write(repository.path().join(".cargo/config.toml"), "[build]\n")
            .expect("Cargo config");
        fs::write(
            repository.path().join("apps/canary/src/lib.rs"),
            "pub fn proof() {}\n",
        )
        .expect("canary file");
        fs::write(
            repository.path().join("crates/example/src/lib.rs"),
            "pub fn value() {}\n",
        )
        .expect("crate file");
        for arguments in [
            vec!["init"],
            vec!["config", "user.email", "canary@example.invalid"],
            vec!["config", "user.name", "Canary"],
            vec!["add", "."],
            vec!["commit", "-m", "fixture"],
        ] {
            assert!(
                Command::new("git")
                    .args(arguments)
                    .current_dir(repository.path())
                    .status()
                    .expect("git subprocess")
                    .success()
            );
        }
        clean_fava_source(repository.path(), &executable).expect("committed inputs are clean");
        fs::write(
            repository.path().join("apps/canary/src/lib.rs"),
            "pub fn changed() {}\n",
        )
        .expect("tracked mutation");
        assert!(clean_fava_source(repository.path(), &executable).is_err());
        assert!(
            Command::new("git")
                .args(["checkout", "--", "apps/canary/src/lib.rs"])
                .current_dir(repository.path())
                .status()
                .expect("git restore")
                .success()
        );
        fs::write(repository.path().join("rust-toolchain.toml"), "[toolchain]\nchannel='beta'\n")
            .expect("toolchain mutation");
        assert!(clean_fava_source(repository.path(), &executable).is_err());
        assert!(
            Command::new("git")
                .args(["checkout", "--", "rust-toolchain.toml"])
                .current_dir(repository.path())
                .status()
                .expect("git restore")
                .success()
        );
        fs::write(repository.path().join(".cargo/config"), "[net]\noffline=true\n")
            .expect("hostile alternate Cargo config");
        assert!(clean_fava_source(repository.path(), &executable).is_err());
        fs::remove_file(repository.path().join(".cargo/config")).expect("config cleanup");
        fs::write(
            repository.path().join("apps/canary/src/untracked.rs"),
            "hostile\n",
        )
        .expect("untracked mutation");
        assert!(clean_fava_source(repository.path(), &executable).is_err());
    }

    #[test]
    fn expected_executable_refuses_replaced_reusable_target() {
        let directory = TempDir::new().expect("target fixture");
        let executable = directory.path().join("canary");
        fs::write(&executable, b"reviewed canary bytes").expect("reviewed target");
        let expected = hex::encode(Sha256::digest(fs::read(&executable).unwrap()));
        exact_executable_sha256(&executable, &expected).expect("reviewed bytes match");
        fs::write(&executable, b"replaced reusable target").expect("replace target");
        assert!(exact_executable_sha256(&executable, &expected).is_err());
    }
}
