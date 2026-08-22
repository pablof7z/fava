//! Retained-evidence sealing and secret scanning for the Croissant NIP-02 canary.

use std::fs;
use std::path::{Path, PathBuf};

use fava::{Kind, Timestamp};
use nostr::event::{Event, EventBuilder, FinalizeEvent};
use nostr::key::Keys;
use nostr::nips::nip19::ToBech32;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{CanaryError, CanaryResult};

pub(super) const SECRET_SCAN_CLASSES: [&str; 19] = [
    "scenario_seed_utf8",
    "author_secret_raw",
    "author_secret_hex",
    "author_secret_upper_hex",
    "author_secret_nsec_lower",
    "author_secret_nsec_upper",
    "author_secret_nip21_lower",
    "author_secret_nip21_upper_payload",
    "author_secret_nip21_upper_scheme",
    "author_secret_nip21_upper",
    "target_secret_raw",
    "target_secret_hex",
    "target_secret_upper_hex",
    "target_secret_nsec_lower",
    "target_secret_nsec_upper",
    "target_secret_nip21_lower",
    "target_secret_nip21_upper_payload",
    "target_secret_nip21_upper_scheme",
    "target_secret_nip21_upper",
];

pub(super) fn secret_needles(
    seed: &[u8],
    keys: [&Keys; 2],
) -> CanaryResult<Vec<Vec<u8>>> {
    if seed.is_empty() {
        return Err(CanaryError::new("Croissant scenario seed was empty"));
    }
    let mut needles = vec![seed.to_vec()];
    for keys in keys {
        let secret = keys.secret_key();
        let hex = secret.to_secret_hex();
        let nsec = secret.to_bech32().map_err(error)?;
        let upper_nsec = nsec.to_ascii_uppercase();
        needles.push(secret.to_secret_bytes().to_vec());
        needles.push(hex.as_bytes().to_vec());
        needles.push(hex.to_ascii_uppercase().into_bytes());
        needles.push(nsec.as_bytes().to_vec());
        needles.push(upper_nsec.as_bytes().to_vec());
        needles.push(format!("nostr:{nsec}").into_bytes());
        needles.push(format!("nostr:{upper_nsec}").into_bytes());
        needles.push(format!("NOSTR:{nsec}").into_bytes());
        needles.push(format!("NOSTR:{upper_nsec}").into_bytes());
    }
    Ok(needles)
}

pub(super) fn assert_secrets_absent(root: &Path, secrets: &[Vec<u8>]) -> CanaryResult<()> {
    for secret in secrets {
        if secret.is_empty() || directory_contains(root, secret, false)? {
            return Err(CanaryError::new(
                "retained Croissant evidence contained secret material",
            ));
        }
    }
    Ok(())
}

pub(super) fn artifact_seal(
    keys: &Keys,
    manifest: &Value,
) -> CanaryResult<Event> {
    let digest = signed_manifest_digest(manifest)?;
    EventBuilder::new(Kind::TextNote, digest)
        .custom_created_at(Timestamp::from(0))
        .finalize(keys)
        .map_err(error)
}

pub(super) fn verify_artifact_seal(manifest: &Value) -> CanaryResult<()> {
    let seal: Event = serde_json::from_value(
        manifest
            .get("artifact_seal")
            .cloned()
            .ok_or_else(|| CanaryError::new("Croissant manifest omitted artifact seal"))?,
    )?;
    seal.verify().map_err(error)?;
    if seal.pubkey.to_hex() != required_string(manifest, "author_public_key")? {
        return Err(CanaryError::new(
            "Croissant artifact seal author disagreed with the flow author",
        ));
    }
    if seal.content != signed_manifest_digest(manifest)? {
        return Err(CanaryError::new(
            "Croissant artifact seal did not cover the signed manifest claims",
        ));
    }
    Ok(())
}

pub(super) fn verify_hashes(root: &Path, manifest: &Value) -> CanaryResult<()> {
    let expected = manifest
        .get("artifact_sha256")
        .and_then(Value::as_object)
        .ok_or_else(|| CanaryError::new("Croissant manifest omitted artifact hashes"))?;
    if expected.is_empty() {
        return Err(CanaryError::new("Croissant artifact hashes were empty"));
    }
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    let actual = files
        .into_iter()
        .filter(|relative| relative != Path::new("manifest.json"))
        .map(|relative| {
            let hash = hex::encode(Sha256::digest(fs::read(root.join(&relative))?));
            Ok((relative.to_string_lossy().into_owned(), Value::String(hash)))
        })
        .collect::<CanaryResult<Map<String, Value>>>()?;
    if &actual != expected {
        return Err(CanaryError::new(
            "Croissant artifact hash set did not verify",
        ));
    }
    Ok(())
}

pub(super) fn directory_contains(
    root: &Path,
    needle: &[u8],
    skip_manifest: bool,
) -> CanaryResult<bool> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    for relative in files {
        if skip_manifest && relative == Path::new("manifest.json") {
            continue;
        }
        if fs::read(root.join(relative))?
            .windows(needle.len())
            .any(|window| window == needle)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn manifest_roots(root: &Path) -> CanaryResult<Vec<PathBuf>> {
    let mut manifests = Vec::new();
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    for relative in files {
        if relative.file_name().and_then(|name| name.to_str()) == Some("manifest.json") {
            manifests.push(
                root.join(relative)
                    .parent()
                    .ok_or_else(|| CanaryError::new("manifest had no parent"))?
                    .to_owned(),
            );
        }
    }
    manifests.sort();
    Ok(manifests)
}

fn signed_manifest_digest(manifest: &Value) -> CanaryResult<String> {
    let mut claims = manifest.clone();
    claims
        .as_object_mut()
        .ok_or_else(|| CanaryError::new("Croissant manifest was not an object"))?
        .remove("artifact_seal");
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&claims)?)))
}

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<PathBuf>) -> CanaryResult<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(path.strip_prefix(root)?.to_owned());
        }
    }
    files.sort();
    Ok(())
}

fn required_string<'a>(value: &'a Value, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("Croissant manifest omitted {field}")))
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
