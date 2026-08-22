//! Hash, seal, and exhaustive process-secret scan support for simple-groups evidence.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use fava::{Kind, Timestamp};
use nostr::event::{Event, EventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::nips::nip19::ToBech32;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CanaryError, CanaryResult};

pub(crate) const MAX_EVIDENCE_DEPTH: usize = 8;
pub(crate) const MAX_EVIDENCE_FILES: usize = 128;
pub(crate) const MAX_EVIDENCE_ENTRIES: usize = 256;
pub(crate) const MAX_EVIDENCE_FILE_BYTES: u64 = 2_097_152;
pub(crate) const MAX_EVIDENCE_AGGREGATE_BYTES: u64 = 8_388_608;
pub(crate) const MAX_MANIFEST_BYTES: u64 = 262_144;

pub(crate) const SECRET_SCAN_CLASSES: [&str; 10] = [
    "scenario_seed_utf8",
    "private_secret_raw",
    "private_secret_hex",
    "private_secret_upper_hex",
    "private_secret_nsec_lower",
    "private_secret_nsec_upper",
    "private_secret_nip21_lower",
    "private_secret_nip21_upper_payload",
    "private_secret_nip21_upper_scheme",
    "private_secret_nip21_upper",
];

pub(crate) fn secret_needles(seed: &[u8], keys: &[&Keys]) -> CanaryResult<Vec<Vec<u8>>> {
    if seed.is_empty() || keys.is_empty() {
        return Err(CanaryError::new(
            "simple-groups secret scan inputs were empty",
        ));
    }
    let mut needles = vec![seed.to_vec()];
    for keys in keys {
        let secret = keys.secret_key();
        let hex = secret.to_secret_hex();
        let nsec = secret.to_bech32().map_err(error)?;
        let upper = nsec.to_ascii_uppercase();
        needles.extend([
            secret.to_secret_bytes().to_vec(),
            hex.as_bytes().to_vec(),
            hex.to_ascii_uppercase().into_bytes(),
            nsec.as_bytes().to_vec(),
            upper.as_bytes().to_vec(),
            format!("nostr:{nsec}").into_bytes(),
            format!("nostr:{upper}").into_bytes(),
            format!("NOSTR:{nsec}").into_bytes(),
            format!("NOSTR:{upper}").into_bytes(),
        ]);
    }
    Ok(needles)
}

pub(crate) fn assert_secrets_absent(root: &Path, needles: &[Vec<u8>]) -> CanaryResult<()> {
    let files = collect_files(root)?;
    for needle in needles {
        if needle.is_empty() {
            return Err(CanaryError::new("simple-groups secret needle was empty"));
        }
        for relative in &files {
            if stream_contains(root, relative, needle)? {
                return Err(CanaryError::new(
                    "retained simple-groups evidence contained secret material",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn artifact_hashes(root: &Path) -> CanaryResult<BTreeMap<String, String>> {
    collect_files(root)?
        .into_iter()
        .filter(|relative| relative != Path::new("manifest.json"))
        .map(|relative| {
            let path = relative
                .to_str()
                .ok_or_else(|| CanaryError::new("simple-groups evidence path was not UTF-8"))?
                .to_owned();
            Ok((path, hash_file(root, &relative)?))
        })
        .collect()
}

pub(crate) fn artifact_seal(keys: &Keys, manifest: &Value) -> CanaryResult<Event> {
    EventBuilder::new(Kind::TextNote, signed_digest(manifest)?)
        .custom_created_at(Timestamp::from(0))
        .finalize(keys)
        .map_err(error)
}

pub(crate) fn signed_digest(manifest: &Value) -> CanaryResult<String> {
    let mut claims = manifest.clone();
    claims
        .as_object_mut()
        .ok_or_else(|| CanaryError::new("simple-groups manifest was not an object"))?
        .remove("artifact_seal");
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&claims)?)))
}

struct WalkCounts {
    entries: usize,
    bytes: u64,
}

#[derive(Clone)]
struct InventoryFile {
    relative: PathBuf,
    device: u64,
    inode: u64,
    bytes: u64,
}

/// One bounded, immutable view of every retained evidence byte used by verification.
pub(crate) struct EvidenceSnapshot {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl EvidenceSnapshot {
    pub(crate) fn capture(root: &Path) -> CanaryResult<Self> {
        Self::capture_inner(root, &mut |_, _| {})
    }

    fn capture_inner(
        root: &Path,
        hook: &mut impl FnMut(CapturePoint, &Path),
    ) -> CanaryResult<Self> {
        let inventory = collect_inventory(root)?;
        let canonical_root = fs::canonicalize(root)?;
        let mut files = BTreeMap::new();
        let mut actual_bytes = 0_u64;
        for expected in &inventory {
            hook(CapturePoint::AfterInventory, &expected.relative);
            let path = canonical_root.join(&expected.relative);
            let mut options = fs::OpenOptions::new();
            options.read(true).custom_flags(libc::O_NOFOLLOW);
            let mut file = options.open(&path).map_err(error)?;
            let before = file.metadata()?;
            if !before.is_file()
                || before.dev() != expected.device
                || before.ino() != expected.inode
                || before.len() != expected.bytes
            {
                return Err(CanaryError::new(
                    "simple-groups evidence changed before bounded capture",
                ));
            }
            hook(CapturePoint::AfterOpen, &expected.relative);
            let mut bytes = Vec::with_capacity(usize::try_from(before.len()).map_err(error)?);
            file.by_ref()
                .take(MAX_EVIDENCE_FILE_BYTES.saturating_add(1))
                .read_to_end(&mut bytes)?;
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_EVIDENCE_FILE_BYTES {
                return Err(CanaryError::new(
                    "simple-groups evidence file bytes exceeded bound",
                ));
            }
            hook(CapturePoint::AfterRead, &expected.relative);
            let after = file.metadata()?;
            let path_after = fs::symlink_metadata(&path)?;
            if after.dev() != before.dev()
                || after.ino() != before.ino()
                || after.len() != before.len()
                || path_after.file_type().is_symlink()
                || !path_after.is_file()
                || path_after.dev() != before.dev()
                || path_after.ino() != before.ino()
                || path_after.len() != before.len()
                || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            {
                return Err(CanaryError::new(
                    "simple-groups evidence changed during bounded capture",
                ));
            }
            actual_bytes = actual_bytes
                .checked_add(u64::try_from(bytes.len()).map_err(error)?)
                .ok_or_else(|| {
                    CanaryError::new("simple-groups evidence aggregate bytes overflow")
                })?;
            if actual_bytes > MAX_EVIDENCE_AGGREGATE_BYTES {
                return Err(CanaryError::new(
                    "simple-groups evidence aggregate bytes exceeded bound",
                ));
            }
            files.insert(expected.relative.clone(), bytes);
        }
        let final_inventory = collect_inventory(root)?;
        if !same_inventory(&inventory, &final_inventory) {
            return Err(CanaryError::new(
                "simple-groups evidence tree changed during bounded capture",
            ));
        }
        Ok(Self { files })
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(PathBuf::as_path)
    }

    pub(crate) fn read(&self, relative: &Path, maximum: u64, label: &str) -> CanaryResult<&[u8]> {
        let bytes = self.files.get(relative).ok_or_else(|| {
            CanaryError::new(format!("simple-groups {label} was absent from snapshot"))
        })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
            return Err(CanaryError::new(format!(
                "simple-groups {label} bytes exceeded bound"
            )));
        }
        Ok(bytes)
    }

    pub(crate) fn contains(&self, relative: &Path, needle: &[u8]) -> CanaryResult<bool> {
        if needle.is_empty() {
            return Err(CanaryError::new(
                "simple-groups evidence search needle was empty",
            ));
        }
        let bytes = self
            .files
            .get(relative)
            .ok_or_else(|| CanaryError::new("simple-groups evidence file left snapshot"))?;
        Ok(bytes.windows(needle.len()).any(|window| window == needle))
    }

    pub(crate) fn artifact_hashes(&self) -> CanaryResult<BTreeMap<String, String>> {
        self.files
            .iter()
            .filter(|(relative, _)| relative.as_path() != Path::new("manifest.json"))
            .map(|(relative, bytes)| {
                let path = relative
                    .to_str()
                    .ok_or_else(|| CanaryError::new("simple-groups evidence path was not UTF-8"))?
                    .to_owned();
                Ok((path, hex::encode(Sha256::digest(bytes))))
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum CapturePoint {
    AfterInventory,
    AfterOpen,
    AfterRead,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SnapshotTestPoint {
    AfterInventory,
    AfterOpen,
    AfterRead,
}

#[cfg(test)]
pub(crate) fn capture_with_test_hook(
    root: &Path,
    mut hook: impl FnMut(SnapshotTestPoint, &Path),
) -> CanaryResult<EvidenceSnapshot> {
    EvidenceSnapshot::capture_inner(root, &mut |point, relative| {
        let point = match point {
            CapturePoint::AfterInventory => SnapshotTestPoint::AfterInventory,
            CapturePoint::AfterOpen => SnapshotTestPoint::AfterOpen,
            CapturePoint::AfterRead => SnapshotTestPoint::AfterRead,
        };
        hook(point, relative);
    })
}

pub(crate) fn collect_files(root: &Path) -> CanaryResult<Vec<PathBuf>> {
    Ok(collect_inventory(root)?
        .into_iter()
        .map(|file| file.relative)
        .collect())
}

#[allow(dead_code, reason = "bounded single-file compatibility for evidence producers")]
pub(crate) fn read_bounded(
    root: &Path,
    relative: &Path,
    maximum: u64,
    label: &str,
) -> CanaryResult<Vec<u8>> {
    Ok(EvidenceSnapshot::capture(root)?
        .read(relative, maximum, label)?
        .to_vec())
}

fn collect_inventory(root: &Path) -> CanaryResult<Vec<InventoryFile>> {
    let root_metadata = fs::symlink_metadata(root)?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(CanaryError::new(
            "simple-groups evidence root must be a real directory",
        ));
    }
    let canonical_root = fs::canonicalize(root)?;
    let mut files = Vec::new();
    visit_files(
        root,
        &canonical_root,
        root,
        0,
        &mut files,
        &mut WalkCounts {
            entries: 0,
            bytes: 0,
        },
    )?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn visit_files(
    root: &Path,
    canonical_root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<InventoryFile>,
    counts: &mut WalkCounts,
) -> CanaryResult<()> {
    if depth > MAX_EVIDENCE_DEPTH {
        return Err(CanaryError::new(
            "simple-groups evidence depth exceeded bound",
        ));
    }
    for entry in fs::read_dir(directory)? {
        counts.entries = counts
            .entries
            .checked_add(1)
            .ok_or_else(|| CanaryError::new("simple-groups evidence entry count overflow"))?;
        if counts.entries > MAX_EVIDENCE_ENTRIES {
            return Err(CanaryError::new(
                "simple-groups evidence entry count exceeded bound",
            ));
        }
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(CanaryError::new(
                "simple-groups evidence contained a symlink",
            ));
        }
        let canonical = fs::canonicalize(&path)?;
        if !canonical.starts_with(canonical_root) {
            return Err(CanaryError::new(
                "simple-groups evidence path escaped its canonical root",
            ));
        }
        if file_type.is_dir() {
            visit_files(
                root,
                canonical_root,
                &path,
                depth.saturating_add(1),
                files,
                counts,
            )?;
        } else if file_type.is_file() {
            record_file(root, &entry, &path, files, counts)?;
        } else {
            return Err(CanaryError::new(
                "simple-groups evidence contained a non-regular entry",
            ));
        }
    }
    Ok(())
}

fn record_file(
    root: &Path,
    entry: &fs::DirEntry,
    path: &Path,
    files: &mut Vec<InventoryFile>,
    counts: &mut WalkCounts,
) -> CanaryResult<()> {
    let bytes = entry.metadata()?.len();
    if bytes > MAX_EVIDENCE_FILE_BYTES {
        return Err(CanaryError::new(
            "simple-groups evidence file bytes exceeded bound",
        ));
    }
    counts.bytes = counts
        .bytes
        .checked_add(bytes)
        .ok_or_else(|| CanaryError::new("simple-groups evidence aggregate bytes overflow"))?;
    if counts.bytes > MAX_EVIDENCE_AGGREGATE_BYTES {
        return Err(CanaryError::new(
            "simple-groups evidence aggregate bytes exceeded bound",
        ));
    }
    if files.len() >= MAX_EVIDENCE_FILES {
        return Err(CanaryError::new(
            "simple-groups evidence file count exceeded bound",
        ));
    }
    let relative = path.strip_prefix(root)?.to_owned();
    if relative.to_str().is_none() {
        return Err(CanaryError::new(
            "simple-groups evidence path was not UTF-8",
        ));
    }
    let metadata = entry.metadata()?;
    files.push(InventoryFile {
        relative,
        device: metadata.dev(),
        inode: metadata.ino(),
        bytes: metadata.len(),
    });
    Ok(())
}

fn same_inventory(first: &[InventoryFile], second: &[InventoryFile]) -> bool {
    first.len() == second.len()
        && first.iter().zip(second).all(|(left, right)| {
            left.relative == right.relative
                && left.device == right.device
                && left.inode == right.inode
                && left.bytes == right.bytes
        })
}

pub(crate) fn stream_contains(root: &Path, relative: &Path, needle: &[u8]) -> CanaryResult<bool> {
    EvidenceSnapshot::capture(root)?.contains(relative, needle)
}

#[cfg(test)]
fn hash_file(root: &Path, relative: &Path) -> CanaryResult<String> {
    let snapshot = EvidenceSnapshot::capture(root)?;
    let bytes = snapshot.read(relative, MAX_EVIDENCE_FILE_BYTES, "artifact")?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
