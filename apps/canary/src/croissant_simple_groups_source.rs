//! Exact committed-source provenance for the simple-groups live proof.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::pinned_build_input::{
    BuildAttestation, MAX_BUILD_ATTESTATION_BYTES, MAX_SOURCE_MANIFEST_BYTES,
    parse_build_attestation, parse_source_manifest,
};
use crate::{CanaryError, CanaryResult};

pub(crate) const SOURCE_PATHS: [&str; 6] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo",
    "apps/canary",
    "crates",
];

pub(crate) const MAX_PINNED_FAVA_EXECUTABLE_BYTES: u64 = 134_217_728;

/// Bounded compiler-input and executable subject emitted by the immutable build pipeline.
pub(crate) struct PinnedBuildAttestation {
    claim: BuildAttestation,
    raw: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct PinnedSourceManifest {
    raw: Vec<u8>,
    sha256: String,
}

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
    #[serde(rename = "fava_build_source_tree_sha256")]
    pub(crate) build_source_tree_sha256: String,
    #[serde(rename = "fava_build_source_manifest_sha256")]
    pub(crate) build_source_manifest_sha256: String,
    #[serde(rename = "fava_build_source_image_sha256")]
    pub(crate) build_source_image_sha256: String,
    #[serde(rename = "fava_build_source_immutable")]
    pub(crate) build_source_immutable: bool,
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
    pub(crate) fn open_for_test(path: &Path) -> CanaryResult<Self> {
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
            let wanted =
                usize::try_from((self.bytes - offset).min(buffer.len() as u64)).map_err(error)?;
            let read = self.file.read_at(&mut buffer[..wanted], offset)?;
            if read == 0 {
                return Err(CanaryError::new(
                    "simple-groups pinned Fava executable changed during retention",
                ));
            }
            destination_file.write_all(&buffer[..read])?;
            offset = offset
                .checked_add(u64::try_from(read).map_err(error)?)
                .ok_or_else(|| {
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

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

pub(crate) fn load_pinned_build_attestation(
    path: &Path,
    expected_executable_sha256: &str,
) -> CanaryResult<PinnedBuildAttestation> {
    let bytes = read_bounded_read_only(path, MAX_BUILD_ATTESTATION_BYTES, "build attestation")?;
    let claim =
        parse_build_attestation(&bytes, expected_executable_sha256).map_err(CanaryError::new)?;
    Ok(PinnedBuildAttestation { claim, raw: bytes })
}

impl PinnedBuildAttestation {
    fn source_manifest_sha256(&self) -> &str {
        &self.claim.fava_build_source_manifest_sha256
    }

    fn source_file_count(&self) -> u64 {
        self.claim.source_file_count
    }

    fn source_total_bytes(&self) -> u64 {
        self.claim.source_total_bytes
    }

    pub(crate) fn rust_base_image_sha256(&self) -> &str {
        &self.claim.rust_base_image_sha256
    }

    pub(crate) fn build_command_sha256(&self) -> &str {
        &self.claim.build_command_sha256
    }

    pub(crate) fn target_storage(&self) -> &str {
        &self.claim.target_storage
    }

    pub(crate) fn target_maximum_bytes(&self) -> u64 {
        self.claim.target_maximum_bytes
    }

    pub(crate) fn subject_digest_origin(&self) -> &str {
        &self.claim.subject_digest_origin
    }

    pub(crate) fn retain(&self, destination: &Path) -> CanaryResult<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o400);
        let mut destination = options.open(destination)?;
        destination.write_all(&self.raw)?;
        destination.sync_all()?;
        Ok(())
    }
}

pub(crate) fn load_pinned_source_manifest(
    path: &Path,
    attestation: &PinnedBuildAttestation,
) -> CanaryResult<PinnedSourceManifest> {
    let raw = read_bounded_read_only(path, MAX_SOURCE_MANIFEST_BYTES, "source manifest")?;
    let sha256 = hex::encode(Sha256::digest(&raw));
    if sha256 != attestation.source_manifest_sha256()
        || sha256 != env!("FAVA_BUILD_SOURCE_MANIFEST_SHA256")
    {
        return Err(CanaryError::new(
            "pinned Fava source manifest digest disagreed with its immutable build",
        ));
    }
    let source = parse_source_manifest(&raw).map_err(CanaryError::new)?;
    if source.revision != env!("FAVA_BUILD_REVISION")
        || source.tree != env!("FAVA_BUILD_TREE")
        || source.file_count != attestation.source_file_count()
        || source.total_bytes != attestation.source_total_bytes()
    {
        return Err(CanaryError::new(
            "pinned Fava source manifest disagreed with its immutable build",
        ));
    }
    Ok(PinnedSourceManifest { raw, sha256 })
}

impl PinnedSourceManifest {
    pub(crate) fn retain(&self, destination: &Path) -> CanaryResult<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o400);
        let mut destination = options.open(destination)?;
        destination.write_all(&self.raw)?;
        destination.sync_all()?;
        Ok(())
    }

    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

fn read_bounded_read_only(path: &Path, maximum: u64, label: &str) -> CanaryResult<Vec<u8>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    let inherited = path.parent() == Some(Path::new("/proc/self/fd"))
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            });
    if !inherited {
        return Err(CanaryError::new(format!(
            "pinned Fava {label} was not an inherited exact descriptor"
        )));
    }
    let mut file = options.open(path)?;
    let before = file.metadata()?;
    if !before.is_file()
        || before.permissions().mode() & 0o222 != 0
        || before.len() == 0
        || before.len() > maximum
    {
        return Err(CanaryError::new(format!(
            "pinned Fava {label} was not a bounded read-only regular file"
        )));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).map_err(error)?);
    file.read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || u64::try_from(bytes.len()).map_err(error)? != before.len()
    {
        return Err(CanaryError::new(format!(
            "pinned Fava {label} changed while reading"
        )));
    }
    Ok(bytes)
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
        env!("FAVA_BUILD_SOURCE_TREE_SHA256"),
        env!("FAVA_BUILD_SOURCE_MANIFEST_SHA256"),
        env!("FAVA_BUILD_SOURCE_CLEAN") == "true",
        env!("FAVA_BUILD_SOURCE_IMMUTABLE") == "true",
        env!("FAVA_BUILD_SOURCE_IMAGE_SHA256"),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "every independently expected compiler-input identity remains explicit"
)]
pub(crate) fn clean_fava_source_against(
    root: &Path,
    executable: &PinnedFavaExecutable,
    build_revision: &str,
    build_tree: &str,
    build_source_tree_sha256: &str,
    build_source_manifest_sha256: &str,
    build_source_clean: bool,
    build_source_immutable: bool,
    build_source_image_sha256: &str,
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
    if !build_source_clean
        || !build_source_immutable
        || !is_lower_hex(build_source_manifest_sha256, 64)
        || build_source_manifest_sha256
            .bytes()
            .all(|byte| byte == b'0')
        || !is_lower_hex(build_source_image_sha256, 64)
        || revision != build_revision
        || source_tree != build_tree
        || tree_sha256 != build_source_tree_sha256
    {
        return Err(CanaryError::new(
            "simple-groups executing Fava build did not match immutable committed inputs",
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
        build_source_tree_sha256: build_source_tree_sha256.to_owned(),
        build_source_manifest_sha256: build_source_manifest_sha256.to_owned(),
        build_source_image_sha256: build_source_image_sha256.to_owned(),
        build_source_immutable,
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
        let wanted =
            usize::try_from((before.len() - offset).min(buffer.len() as u64)).map_err(error)?;
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

pub(crate) fn git(root: &Path, arguments: &[&str]) -> CanaryResult<Vec<u8>> {
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

pub(crate) fn text(bytes: Vec<u8>) -> CanaryResult<String> {
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
