//! Private canonical parser for immutable canary compiler-input attestations.

use std::path::Path;

use serde::Deserialize;

pub(crate) const MAX_BUILD_ATTESTATION_BYTES: u64 = 4_096;
pub(crate) const MAX_SOURCE_MANIFEST_BYTES: u64 = 1_048_576;
pub(crate) const MAX_SOURCE_FILES: u64 = 4_096;
pub(crate) const MAX_SOURCE_TOTAL_BYTES: u64 = 67_108_864;
pub(crate) const PINNED_TARGET_MAXIMUM_BYTES: u64 = 4_294_967_296;
const MAX_SOURCE_FILE_BYTES: u64 = 8_388_608;
pub(crate) const BUILD_COMMAND_SHA256: &str =
    "8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08";
pub(crate) const REGISTRY_IMAGE_SHA256: &str =
    "a3d8aaa63ed8681a604f1dea0aa03f100d5895b6a58ace528858a7b332415373";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BuildAttestation {
    pub(crate) schema: String,
    pub(crate) fava_revision: String,
    pub(crate) fava_build_tree: String,
    pub(crate) fava_build_source_tree_sha256: String,
    pub(crate) fava_build_source_manifest_sha256: String,
    pub(crate) fava_build_source_image_sha256: String,
    pub(crate) rust_base_image_sha256: String,
    pub(crate) build_command_sha256: String,
    pub(crate) fava_canary_executable_sha256: String,
    pub(crate) fava_canary_subject_image_sha256: String,
    pub(crate) source_file_count: u64,
    pub(crate) source_total_bytes: u64,
    pub(crate) toctou_read_only_attempt: String,
    pub(crate) toctou_deliberate_break: String,
    pub(crate) source_root: String,
    pub(crate) target_root: String,
    pub(crate) compiler_network: String,
    pub(crate) compiler_source_mount: String,
    pub(crate) compiler_user: String,
    pub(crate) target_storage: String,
    pub(crate) target_maximum_bytes: u64,
    pub(crate) subject_digest_origin: String,
    pub(crate) source_transport: String,
    pub(crate) source_transport_image_sha256: String,
}

pub(crate) struct SourceManifestClaim {
    pub(crate) revision: String,
    pub(crate) tree: String,
    pub(crate) file_count: u64,
    pub(crate) total_bytes: u64,
}

pub(crate) fn parse_build_attestation(
    bytes: &[u8],
    expected_executable_sha256: &str,
) -> Result<BuildAttestation, String> {
    let claim: BuildAttestation =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if claim.schema != "fava-pinned-build-v1"
        || !is_lower_hex(&claim.fava_revision, 40)
        || !is_lower_hex(&claim.fava_build_tree, 40)
        || !is_lower_hex(&claim.fava_build_source_tree_sha256, 64)
        || !nonzero_lower_hex(&claim.fava_build_source_manifest_sha256)
        || !nonzero_lower_hex(&claim.fava_build_source_image_sha256)
        || !nonzero_lower_hex(&claim.rust_base_image_sha256)
        || claim.build_command_sha256 != BUILD_COMMAND_SHA256
        || !is_lower_hex(&claim.fava_canary_executable_sha256, 64)
        || !nonzero_lower_hex(&claim.fava_canary_subject_image_sha256)
        || claim.fava_canary_executable_sha256 != expected_executable_sha256
        || claim.fava_revision != env!("FAVA_BUILD_REVISION")
        || claim.fava_build_tree != env!("FAVA_BUILD_TREE")
        || claim.fava_build_source_tree_sha256 != env!("FAVA_BUILD_SOURCE_TREE_SHA256")
        || claim.fava_build_source_manifest_sha256 != env!("FAVA_BUILD_SOURCE_MANIFEST_SHA256")
        || claim.fava_build_source_image_sha256 != env!("FAVA_BUILD_SOURCE_IMAGE_SHA256")
        || claim.rust_base_image_sha256 != env!("FAVA_BUILD_RUST_BASE_IMAGE_SHA256")
        || env!("FAVA_BUILD_SOURCE_IMMUTABLE") != "true"
        || claim.source_file_count == 0
        || claim.source_file_count > MAX_SOURCE_FILES
        || claim.source_total_bytes > MAX_SOURCE_TOTAL_BYTES
        || claim.toctou_read_only_attempt != "EROFS"
        || claim.toctou_deliberate_break != "compiled-hostile-bytes"
        || claim.source_root != "/source"
        || claim.target_root != "/target"
        || claim.compiler_network != "none"
        || claim.compiler_source_mount != "read-only"
        || claim.compiler_user != "65532:65532"
        || claim.target_storage != "engine-content-addressed-image"
        || claim.target_maximum_bytes != PINNED_TARGET_MAXIMUM_BYTES
        || claim.subject_digest_origin != "engine-image"
        || claim.source_transport != "owned-loopback-registry"
        || claim.source_transport_image_sha256 != REGISTRY_IMAGE_SHA256
    {
        return Err("pinned build attestation did not match immutable compiler inputs".to_owned());
    }
    Ok(claim)
}

pub(crate) fn parse_source_manifest(bytes: &[u8]) -> Result<SourceManifestClaim, String> {
    if !bytes.ends_with(b"\n") || bytes.contains(&b'\r') {
        return Err("pinned source manifest was not canonical LF text".to_owned());
    }
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() < 6 || lines[0] != "format=fava-pinned-source-v1" {
        return Err("pinned source manifest header was invalid".to_owned());
    }
    let revision = exact_hex(lines[1], "revision=", 40)?;
    let tree = exact_hex(lines[2], "tree=", 40)?;
    let file_count = canonical_prefixed_decimal(lines[3], "file_count=")?;
    let total_bytes = canonical_prefixed_decimal(lines[4], "total_bytes=")?;
    if file_count == 0
        || file_count > MAX_SOURCE_FILES
        || total_bytes > MAX_SOURCE_TOTAL_BYTES
        || lines.len() != 5 + usize::try_from(file_count).map_err(|error| error.to_string())?
    {
        return Err("pinned source manifest bounds were invalid".to_owned());
    }
    let mut previous = None;
    let mut observed_total = 0_u64;
    for line in &lines[5..] {
        let fields = line
            .strip_prefix("file=")
            .ok_or_else(|| "pinned source manifest row was invalid".to_owned())?
            .split('\t')
            .collect::<Vec<_>>();
        if fields.len() != 4
            || !matches!(fields[0], "100644" | "100755")
            || !is_lower_hex(fields[1], 64)
        {
            return Err("pinned source manifest row was invalid".to_owned());
        }
        let bytes = canonical_decimal(fields[2])?;
        if bytes > MAX_SOURCE_FILE_BYTES || !canonical_path(fields[3]) {
            return Err("pinned source manifest row exceeded its bound".to_owned());
        }
        if previous.is_some_and(|value: &str| value >= fields[3]) {
            return Err("pinned source manifest rows were not ordered".to_owned());
        }
        previous = Some(fields[3]);
        observed_total = observed_total
            .checked_add(bytes)
            .ok_or_else(|| "pinned source manifest byte count overflow".to_owned())?;
    }
    if observed_total != total_bytes {
        return Err("pinned source manifest byte total disagreed".to_owned());
    }
    Ok(SourceManifestClaim {
        revision,
        tree,
        file_count,
        total_bytes,
    })
}

fn nonzero_lower_hex(value: &str) -> bool {
    is_lower_hex(value, 64) && !value.bytes().all(|byte| byte == b'0')
}

pub(crate) fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn exact_hex(line: &str, prefix: &str, length: usize) -> Result<String, String> {
    let value = line
        .strip_prefix(prefix)
        .ok_or_else(|| "pinned source manifest identity was absent".to_owned())?;
    if !is_lower_hex(value, length) {
        return Err("pinned source manifest identity was invalid".to_owned());
    }
    Ok(value.to_owned())
}

fn canonical_prefixed_decimal(line: &str, prefix: &str) -> Result<u64, String> {
    canonical_decimal(
        line.strip_prefix(prefix)
            .ok_or_else(|| "pinned source manifest count was absent".to_owned())?,
    )
}

fn canonical_decimal(value: &str) -> Result<u64, String> {
    let parsed = value.parse::<u64>().map_err(|error| error.to_string())?;
    if parsed.to_string() != value {
        return Err("pinned source manifest decimal was not canonical".to_owned());
    }
    Ok(parsed)
}

fn canonical_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('/')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/+@=-".contains(&byte))
        && Path::new(value)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        && is_allowed_source_path(value)
}

pub(crate) fn is_allowed_source_path(value: &str) -> bool {
    matches!(value, "Cargo.toml" | "Cargo.lock" | "rust-toolchain.toml")
        || value.starts_with(".cargo/")
        || value.starts_with("apps/canary/")
        || value.starts_with("crates/")
}
