//! Exact committed-source provenance for the simple-groups live proof.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
#[cfg(any(target_os = "linux", test))]
use std::os::unix::fs::PermissionsExt;
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

pub(crate) const MAX_PINNED_FAVA_EXECUTABLE_BYTES: u64 = 134_217_728;

#[derive(Debug)]
pub(crate) struct PinnedFavaExecutable {
    file: fs::File,
    bytes: u64,
    device: u64,
    inode: u64,
    sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct FavaSourceProvenance {
    #[serde(rename = "fava_revision")]
    pub(crate) revision: String,
    #[serde(rename = "fava_source_tree_sha256")]
    pub(crate) tree_sha256: String,
    #[serde(rename = "fava_build_revision")]
    pub(crate) build_revision: String,
    #[serde(rename = "fava_build_tree")]
    pub(crate) build_tree: String,
    #[serde(rename = "fava_source_clean")]
    pub(crate) clean: bool,
    #[serde(rename = "fava_canary_executable_sha256")]
    pub(crate) canary_executable_sha256: String,
    #[serde(rename = "fava_canary_executable_bytes")]
    pub(crate) canary_executable_bytes: u64,
    #[serde(rename = "fava_canary_executable_pinned")]
    pub(crate) canary_executable_pinned: bool,
    #[serde(rename = "fava_execution_platform")]
    pub(crate) execution_platform: &'static str,
}

impl PinnedFavaExecutable {
    pub(crate) fn inherited() -> CanaryResult<Self> {
        #[cfg(target_os = "linux")]
        {
            Self::open(Path::new("/proc/self/fd/0"), false)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(CanaryError::new(
                "descriptor-pinned Fava execution is unsupported on this host",
            ))
        }
    }

    #[cfg(test)]
    fn open_for_test(path: &Path) -> CanaryResult<Self> {
        Self::open(path, true)
    }

    #[cfg(any(target_os = "linux", test))]
    fn open(path: &Path, no_follow: bool) -> CanaryResult<Self> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        if no_follow {
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(path)?;
        let before = file.metadata()?;
        if !before.is_file()
            || before.permissions().mode() & 0o222 != 0
            || before.len() == 0
            || before.len() > MAX_PINNED_FAVA_EXECUTABLE_BYTES
        {
            return Err(CanaryError::new(
                "simple-groups pinned Fava executable was not a bounded regular file",
            ));
        }
        let sha256 = descriptor_sha256(&file, &before)?;
        Ok(Self {
            file,
            bytes: before.len(),
            device: before.dev(),
            inode: before.ino(),
            sha256,
        })
    }

    pub(crate) fn retain(&self, destination: &Path) -> CanaryResult<()> {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create_new(true).mode(0o400);
        let mut destination_file = options.open(destination)?;
        let mut offset = 0_u64;
        let mut buffer = [0_u8; 16_384];
        while offset < self.bytes {
            let wanted = usize::try_from((self.bytes - offset).min(buffer.len() as u64))
                .map_err(error)?;
            let read = self.file.read_at(&mut buffer[..wanted], offset)?;
            if read == 0 {
                return Err(CanaryError::new(
                    "simple-groups pinned Fava executable changed during retention",
                ));
            }
            destination_file.write_all(&buffer[..read])?;
            offset = offset.checked_add(u64::try_from(read).map_err(error)?).ok_or_else(|| {
                CanaryError::new("simple-groups pinned executable byte count overflow")
            })?;
        }
        destination_file.sync_all()?;
        if destination_file.metadata()?.len() != self.bytes
            || descriptor_sha256(&destination_file, &destination_file.metadata()?)? != self.sha256
        {
            return Err(CanaryError::new(
                "simple-groups retained Fava executable disagreed with its opened image",
            ));
        }
        Ok(())
    }

    fn unchanged(&self) -> CanaryResult<bool> {
        let metadata = self.file.metadata()?;
        Ok(metadata.dev() == self.device
            && metadata.ino() == self.inode
            && metadata.len() == self.bytes
            && descriptor_sha256(&self.file, &metadata)? == self.sha256)
    }
}

pub(crate) fn clean_fava_source(
    root: &Path,
    executable: &PinnedFavaExecutable,
) -> CanaryResult<FavaSourceProvenance> {
    clean_fava_source_against(
        root,
        executable,
        env!("FAVA_BUILD_REVISION"),
        env!("FAVA_BUILD_TREE"),
        env!("FAVA_BUILD_SOURCE_CLEAN") == "true",
    )
}

fn clean_fava_source_against(
    root: &Path,
    executable: &PinnedFavaExecutable,
    build_revision: &str,
    build_tree: &str,
    build_source_clean: bool,
) -> CanaryResult<FavaSourceProvenance> {
    let mut status_arguments = vec!["status", "--porcelain=v1", "--untracked-files=all", "--"];
    status_arguments.extend(SOURCE_PATHS);
    if !git(root, &status_arguments)?.is_empty() {
        return Err(CanaryError::new(
            "simple-groups Fava source/build inputs differed from committed HEAD",
        ));
    }
    let revision = text(git(root, &["rev-parse", "HEAD"])?)?;
    let source_tree = text(git(root, &["rev-parse", "HEAD^{tree}"])?)?;
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
    let tree_sha256 = hex::encode(Sha256::digest(tree));
    if !build_source_clean || revision != build_revision || source_tree != build_tree {
        return Err(CanaryError::new(
            "simple-groups executing Fava build did not match the clean source revision",
        ));
    }
    if !executable.unchanged()? {
        return Err(CanaryError::new(
            "simple-groups pinned Fava executable changed during the live proof",
        ));
    }
    Ok(FavaSourceProvenance {
        revision,
        tree_sha256,
        build_revision: build_revision.to_owned(),
        build_tree: build_tree.to_owned(),
        clean: true,
        canary_executable_sha256: executable.sha256.clone(),
        canary_executable_bytes: executable.bytes,
        canary_executable_pinned: true,
        execution_platform: "linux-sealed-memfd-proc-fd",
    })
}

fn descriptor_sha256(file: &fs::File, before: &fs::Metadata) -> CanaryResult<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16_384];
    let mut offset = 0_u64;
    while offset < before.len() {
        let wanted = usize::try_from((before.len() - offset).min(buffer.len() as u64))
            .map_err(error)?;
        let read = file.read_at(&mut buffer[..wanted], offset)?;
        if read == 0 {
            return Err(CanaryError::new(
                "simple-groups pinned executable changed during hashing",
            ));
        }
        digest.update(&buffer[..read]);
        offset = offset
            .checked_add(u64::try_from(read).map_err(error)?)
            .ok_or_else(|| CanaryError::new("simple-groups executable byte count overflow"))?;
    }
    let after = file.metadata()?;
    if before.dev() != after.dev() || before.ino() != after.ino() || before.len() != after.len() {
        return Err(CanaryError::new(
            "simple-groups pinned executable changed during hashing",
        ));
    }
    Ok(hex::encode(digest.finalize()))
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
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use tempfile::TempDir;

    use super::{PinnedFavaExecutable, clean_fava_source_against, git, text};

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one committed fixture exercises every tracked, untracked, stale, and dirty build input"
    )]
    fn production_source_gate_refuses_tracked_and_relevant_untracked_changes() {
        let executable_file = TempDir::new().expect("pinned executable");
        let executable_path = executable_file.path().join("canary");
        fs::write(&executable_path, b"pinned canary bytes").expect("pinned bytes");
        fs::set_permissions(&executable_path, fs::Permissions::from_mode(0o500))
            .expect("pin permissions");
        let executable = PinnedFavaExecutable::open_for_test(&executable_path).expect("pin");
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
        let revision = text(git(repository.path(), &["rev-parse", "HEAD"]).unwrap()).unwrap();
        let build_tree =
            text(git(repository.path(), &["rev-parse", "HEAD^{tree}"]).unwrap()).unwrap();
        clean_fava_source_against(repository.path(), &executable, &revision, &build_tree, true)
            .expect("committed inputs are clean");
        fs::write(
            repository.path().join("apps/canary/src/lib.rs"),
            "pub fn changed() {}\n",
        )
        .expect("tracked mutation");
        assert!(
            clean_fava_source_against(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
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
        assert!(
            clean_fava_source_against(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
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
        assert!(
            clean_fava_source_against(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        fs::remove_file(repository.path().join(".cargo/config")).expect("config cleanup");
        fs::write(
            repository.path().join("apps/canary/src/untracked.rs"),
            "hostile\n",
        )
        .expect("untracked mutation");
        assert!(
            clean_fava_source_against(repository.path(), &executable, &revision, &build_tree, true)
                .is_err()
        );
        fs::remove_file(repository.path().join("apps/canary/src/untracked.rs"))
            .expect("untracked cleanup");
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                "0000000000000000000000000000000000000000",
                &build_tree,
                true,
            )
            .is_err(),
            "stale build revision was accepted"
        );
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                &revision,
                "0000000000000000000000000000000000000000",
                true,
            )
            .is_err(),
            "stale build tree was accepted"
        );
        assert!(
            clean_fava_source_against(
                repository.path(),
                &executable,
                &revision,
                &build_tree,
                false,
            )
            .is_err(),
            "dirty build provenance was accepted"
        );
    }

    #[test]
    fn opened_executable_retains_original_after_path_replacement_and_deletion() {
        let directory = TempDir::new().expect("target fixture");
        let executable = directory.path().join("canary");
        fs::write(&executable, b"reviewed canary bytes").expect("reviewed target");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500))
            .expect("reviewed permissions");
        let pinned = PinnedFavaExecutable::open_for_test(&executable).expect("open exact image");
        let replacement = directory.path().join("replacement");
        fs::write(&replacement, b"replaced reusable target").expect("replacement bytes");
        fs::rename(&replacement, &executable).expect("replace target path");
        fs::remove_file(&executable).expect("delete replacement path");
        let retained = directory.path().join("retained-canary");
        pinned.retain(&retained).expect("retain opened original image");
        assert_eq!(fs::read(retained).unwrap(), b"reviewed canary bytes");
    }
}
