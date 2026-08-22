//! Hash, seal, and exhaustive process-secret scan support for simple-groups evidence.

use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Component;
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

pub(crate) fn collect_files(root: &Path) -> CanaryResult<Vec<PathBuf>> {
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
    files.sort();
    Ok(files)
}

fn visit_files(
    root: &Path,
    canonical_root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
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
    files: &mut Vec<PathBuf>,
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
    files.push(relative);
    Ok(())
}

pub(crate) fn read_bounded(
    root: &Path,
    relative: &Path,
    maximum: u64,
    label: &str,
) -> CanaryResult<Vec<u8>> {
    let (file, bytes) = open_checked(root, relative)?;
    if bytes > maximum {
        return Err(CanaryError::new(format!(
            "simple-groups {label} bytes exceeded bound"
        )));
    }
    let capacity = usize::try_from(bytes)
        .map_err(|_| CanaryError::new(format!("simple-groups {label} bytes overflow")))?;
    let mut output = Vec::with_capacity(capacity);
    file.take(maximum.saturating_add(1))
        .read_to_end(&mut output)?;
    if u64::try_from(output.len()).unwrap_or(u64::MAX) > maximum {
        return Err(CanaryError::new(format!(
            "simple-groups {label} bytes exceeded bound"
        )));
    }
    Ok(output)
}

pub(crate) fn stream_contains(root: &Path, relative: &Path, needle: &[u8]) -> CanaryResult<bool> {
    if needle.is_empty() {
        return Err(CanaryError::new(
            "simple-groups evidence search needle was empty",
        ));
    }
    let (mut file, _) = open_checked(root, relative)?;
    let mut buffer = [0_u8; 8192];
    let mut retained = Vec::new();
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(false);
        }
        retained.extend_from_slice(&buffer[..read]);
        if retained
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return Ok(true);
        }
        let keep = needle.len().saturating_sub(1).min(retained.len());
        retained.drain(..retained.len().saturating_sub(keep));
    }
}

fn hash_file(root: &Path, relative: &Path) -> CanaryResult<String> {
    let (mut file, _) = open_checked(root, relative)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn open_checked(root: &Path, relative: &Path) -> CanaryResult<(File, u64)> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CanaryError::new(
            "simple-groups evidence path was not a normal relative path",
        ));
    }
    let root = fs::canonicalize(root)?;
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CanaryError::new(
            "simple-groups evidence open target was not a regular file",
        ));
    }
    let canonical = fs::canonicalize(&path)?;
    if !canonical.starts_with(&root) {
        return Err(CanaryError::new(
            "simple-groups evidence open escaped its canonical root",
        ));
    }
    if metadata.len() > MAX_EVIDENCE_FILE_BYTES {
        return Err(CanaryError::new(
            "simple-groups evidence file bytes exceeded bound",
        ));
    }
    Ok((File::open(canonical)?, metadata.len()))
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
