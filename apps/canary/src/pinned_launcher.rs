//! Linux-only launcher that binds immutable build inputs to one sealed canary child.

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Stdio;

use rustix::io::dup;
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::pinned_build_input::{
    BuildAttestation, MAX_BUILD_ATTESTATION_BYTES, MAX_SOURCE_MANIFEST_BYTES,
    parse_build_attestation, parse_source_manifest,
};
use crate::sealed_executable::SealedExecutable;

pub(crate) async fn launch(
    binary: &Path,
    source: &Path,
    attestation: &Path,
    source_manifest: &Path,
    arguments: impl IntoIterator<Item = String>,
) -> canary::CanaryResult<()> {
    ensure_clean_build_source(source)?;
    let mut open = fs::OpenOptions::new();
    open.read(true).custom_flags(libc::O_NOFOLLOW);
    let executable = open.open(binary)?;
    let metadata = executable.metadata()?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o222 != 0
        || metadata.len() == 0
        || metadata.len() > 134_217_728
    {
        return Err(std::io::Error::other("pinned canary executable exceeded its bound").into());
    }
    let sealed = SealedExecutable::copy_from(&executable, 134_217_728)?;
    let pinned_digest = sealed.sha256().to_owned();
    let (attestation_file, attestation_bytes) =
        open_input(attestation, MAX_BUILD_ATTESTATION_BYTES, "build attestation")?;
    let claim = parse_build_attestation(&attestation_bytes, &pinned_digest)
        .map_err(std::io::Error::other)?;
    let (source_manifest_file, source_manifest_bytes) =
        open_input(source_manifest, MAX_SOURCE_MANIFEST_BYTES, "source manifest")?;
    verify_source_manifest(&source_manifest_bytes, &claim)?;
    let mut child = opened_child(
        &sealed,
        source,
        arguments,
        attestation_file,
        source_manifest_file,
    )?;
    let status = child.wait().await?;
    if !status.success() {
        return Err(std::io::Error::other(format!("pinned canary child failed with {status}")).into());
    }
    println!("pinned_fava_canary_executable_sha256: {pinned_digest}");
    Ok(())
}

fn open_input(
    path: &Path,
    maximum: u64,
    label: &str,
) -> canary::CanaryResult<(SealedExecutable, Vec<u8>)> {
    let mut options = fs::OpenOptions::new();
    options.read(true).custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let before = file.metadata()?;
    if !before.is_file()
        || before.permissions().mode() & 0o222 != 0
        || before.len() == 0
        || before.len() > maximum
    {
        return Err(std::io::Error::other(format!("pinned {label} exceeded its bound")).into());
    }
    let sealed = SealedExecutable::copy_from(&file, maximum)?;
    let sealed_file = sealed.try_clone()?;
    let mut bytes = vec![0_u8; usize::try_from(before.len()).map_err(|error| {
        std::io::Error::other(format!("pinned {label} byte count was invalid: {error}"))
    })?];
    let read = sealed_file.read_at(&mut bytes, 0)?;
    if read != bytes.len() {
        return Err(std::io::Error::other(format!("sealed {label} ended early")).into());
    }
    Ok((sealed, bytes))
}

fn verify_source_manifest(bytes: &[u8], claim: &BuildAttestation) -> canary::CanaryResult<()> {
    let digest = hex::encode(Sha256::digest(bytes));
    let source = parse_source_manifest(bytes).map_err(std::io::Error::other)?;
    if digest != claim.fava_build_source_manifest_sha256
        || digest != env!("FAVA_BUILD_SOURCE_MANIFEST_SHA256")
        || source.revision != claim.fava_revision
        || source.tree != claim.fava_build_tree
        || source.file_count != claim.source_file_count
        || source.total_bytes != claim.source_total_bytes
    {
        return Err(std::io::Error::other(
            "pinned source manifest did not match its build attestation",
        )
        .into());
    }
    Ok(())
}

fn opened_child(
    executable: &SealedExecutable,
    current_directory: &Path,
    arguments: impl IntoIterator<Item = String>,
    build_attestation: SealedExecutable,
    source_manifest: SealedExecutable,
) -> canary::CanaryResult<tokio::process::Child> {
    let build_file = build_attestation.try_clone()?;
    let source_file = source_manifest.try_clone()?;
    let build_attestation = dup(&build_file).map_err(std::io::Error::from)?;
    let source_manifest = dup(&source_file).map_err(std::io::Error::from)?;
    let build_path = format!("/proc/self/fd/{}", build_attestation.as_raw_fd());
    let source_path = format!("/proc/self/fd/{}", source_manifest.as_raw_fd());
    let mut command = Command::new("/proc/self/fd/0");
    command
        .args(["run", "croissant-simple-groups-public-flow"])
        .args([
            "--fava-build-attestation".to_owned(),
            build_path,
            "--fava-build-source-manifest".to_owned(),
            source_path,
        ])
        .args(arguments)
        .stdin(Stdio::from(executable.try_clone()?))
        .current_dir(current_directory)
        .kill_on_drop(true);
    Ok(command.spawn()?)
}

fn ensure_clean_build_source(root: &Path) -> canary::CanaryResult<()> {
    let status = std::process::Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            ".cargo",
            "apps/canary",
            "crates",
        ])
        .current_dir(root)
        .output()?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(std::io::Error::other(
            "pinned canary source/build inputs were not clean committed files",
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    use tempfile::TempDir;

    use super::opened_child;
    use crate::sealed_executable::SealedExecutable;

    #[tokio::test]
    async fn opened_launcher_executes_original_after_candidate_path_replacement() {
        let fixture = TempDir::new().expect("launcher fixture");
        let candidate = fixture.path().join("canary");
        fs::write(&candidate, b"#!/bin/sh\nexit 0\n").expect("candidate");
        fs::set_permissions(&candidate, fs::Permissions::from_mode(0o500)).expect("permissions");
        let mut open = fs::OpenOptions::new();
        open.read(true).custom_flags(libc::O_NOFOLLOW);
        let opened = open.open(&candidate).expect("opened candidate");
        let sealed = SealedExecutable::copy_from(&opened, 1024).expect("sealed candidate");
        let marker = fixture.path().join("replacement-executed");
        let replacement = fixture.path().join("replacement");
        fs::write(
            &replacement,
            format!("#!/bin/sh\ntouch {}\nexit 72\n", marker.display()),
        )
        .expect("replacement");
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o500))
            .expect("replacement permissions");
        fs::rename(replacement, candidate).expect("replace candidate after open/hash");
        let build_input = SealedExecutable::copy_from(&opened, 1024).expect("sealed attestation");
        let source_input = SealedExecutable::copy_from(&opened, 1024).expect("sealed manifest");
        let status = opened_child(
            &sealed,
            fixture.path(),
            Vec::new(),
            build_input,
            source_input,
        )
        .expect("descriptor child")
        .wait()
        .await
        .expect("descriptor launch");
        assert!(status.success());
        assert!(!marker.exists(), "replacement exited 72");
    }
}
