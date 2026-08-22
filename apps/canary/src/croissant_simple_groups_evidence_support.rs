//! Hash, seal, and exhaustive process-secret scan support for simple-groups evidence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use fava::{Kind, Timestamp};
use nostr::event::{Event, EventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::nips::nip19::ToBech32;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{CanaryError, CanaryResult};

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
            if fs::read(root.join(relative))?
                .windows(needle.len())
                .any(|window| window == needle)
            {
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
            let hash = hex::encode(Sha256::digest(fs::read(root.join(&relative))?));
            Ok((relative.to_string_lossy().into_owned(), hash))
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

pub(crate) fn collect_files(root: &Path) -> CanaryResult<Vec<PathBuf>> {
    fn visit(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> CanaryResult<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else if path.is_file() {
                files.push(path.strip_prefix(root)?.to_owned());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
