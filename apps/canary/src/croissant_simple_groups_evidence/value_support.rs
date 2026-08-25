fn run_roots(root: &Path) -> CanaryResult<Vec<std::path::PathBuf>> {
    run_roots_with(root, |_| {})
}

fn run_roots_with(
    root: &Path,
    mut visited: impl FnMut(&Path),
) -> CanaryResult<Vec<std::path::PathBuf>> {
    let mut roots = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        visited(&entry.path());
        if roots.len() == 2 {
            return Err(CanaryError::new(
                "simple-groups pair must contain exactly two manifests",
            ));
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| CanaryError::new("simple-groups pair entry was not UTF-8"))?;
        if !entry.file_type()?.is_dir() || name.starts_with(".fava-canary-staging-") {
            return Err(CanaryError::new(
                "simple-groups pair root contained staging or non-run residue",
            ));
        }
        roots.push(entry.path());
    }
    if roots.len() != 2 {
        return Err(CanaryError::new(
            "simple-groups pair must contain exactly two manifests",
        ));
    }
    roots.sort_unstable();
    Ok(roots)
}

fn tree_contains(
    snapshot: &EvidenceSnapshot,
    needle: &[u8],
    skip_manifest: bool,
) -> CanaryResult<bool> {
    for path in snapshot.files() {
        if skip_manifest && path == Path::new("manifest.json") {
            continue;
        }
        if snapshot.contains(path, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn exact_objects<'a>(
    value: &'a Value,
    field: &str,
    count: usize,
) -> CanaryResult<Vec<&'a Map<String, Value>>> {
    exact_values(value, field, count)?
        .iter()
        .map(|item| item.as_object().ok_or_else(|| invalid_entry(field)))
        .collect()
}

fn exact_strings(value: &Value, field: &str, count: usize) -> CanaryResult<Vec<String>> {
    exact_values(value, field, count)?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_entry(field))
        })
        .collect()
}

fn exact_u64s(value: &Value, field: &str, count: usize) -> CanaryResult<Vec<u64>> {
    exact_values(value, field, count)?
        .iter()
        .map(|item| item.as_u64().ok_or_else(|| invalid_entry(field)))
        .collect()
}

fn exact_values<'a>(value: &'a Value, field: &str, count: usize) -> CanaryResult<&'a [Value]> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new(format!("simple-groups manifest omitted {field}")))?;
    if values.len() != count {
        return Err(CanaryError::new(format!(
            "simple-groups {field} count was not {count}"
        )));
    }
    Ok(values)
}

fn invalid_entry(field: &str) -> CanaryError {
    CanaryError::new(format!("simple-groups {field} entry was invalid"))
}

fn object_string<'a>(value: &'a Map<String, Value>, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("simple-groups child omitted {field}")))
}

fn required_string<'a>(value: &'a Value, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("simple-groups manifest omitted {field}")))
}

fn verify_source_provenance(snapshot: &EvidenceSnapshot, manifest: &Value) -> CanaryResult<()> {
    let source: Value = serde_json::from_slice(snapshot.read(
        Path::new("source/fava.json"),
        4_096,
        "Fava source provenance",
    )?)?;
    if source.as_object().map(Map::len) != Some(13) {
        return Err(CanaryError::new(
            "simple-groups retained Fava source proof had an unrecognized claim",
        ));
    }
    for field in [
        "fava_revision",
        "fava_source_tree_sha256",
        "fava_build_revision",
        "fava_build_tree",
        "fava_build_source_tree_sha256",
        "fava_build_source_manifest_sha256",
        "fava_build_source_image_sha256",
        "fava_build_source_immutable",
        "fava_source_clean",
        "fava_canary_executable_sha256",
        "fava_canary_executable_bytes",
        "fava_canary_executable_pinned",
        "fava_execution_platform",
    ] {
        if source.get(field) != manifest.get(field) {
            return Err(CanaryError::new(
                "simple-groups retained Fava source proof disagreed with the manifest",
            ));
        }
    }
    if source
        .get("fava_canary_executable_pinned")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err(CanaryError::new(
            "simple-groups Fava executable was not independently pinned",
        ));
    }
    let executable = snapshot.read(
        Path::new("source/fava-canary"),
        crate::croissant_simple_groups_source::MAX_PINNED_FAVA_EXECUTABLE_BYTES,
        "retained Fava executable",
    )?;
    let bytes = u64::try_from(executable.len()).map_err(error)?;
    let digest = hex::encode(Sha256::digest(executable));
    if source
        .get("fava_canary_executable_bytes")
        .and_then(Value::as_u64)
        != Some(bytes)
        || required_string(&source, "fava_canary_executable_sha256")? != digest
    {
        return Err(CanaryError::new(
            "simple-groups retained Fava executable bytes disagreed with their proof",
        ));
    }
    verify_build_attestation(snapshot, manifest, &digest)?;
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "one retained-attestation oracle binds its complete fixed schema and source manifest"
)]
fn verify_build_attestation(
    snapshot: &EvidenceSnapshot,
    manifest: &Value,
    executable_sha256: &str,
) -> CanaryResult<()> {
    let build: Value = serde_json::from_slice(snapshot.read(
        Path::new("source/fava-build.json"),
        4_096,
        "Fava build attestation",
    )?)?;
    if build.as_object().map(Map::len) != Some(24)
        || required_string(&build, "schema")? != "fava-pinned-build-v1"
    {
        return Err(CanaryError::new(
            "simple-groups retained Fava build attestation had an unrecognized claim",
        ));
    }
    for (build_field, manifest_field) in [
        ("fava_revision", "fava_revision"),
        ("fava_build_tree", "fava_build_tree"),
        (
            "fava_build_source_tree_sha256",
            "fava_build_source_tree_sha256",
        ),
        (
            "fava_build_source_manifest_sha256",
            "fava_build_source_manifest_sha256",
        ),
        (
            "fava_build_source_image_sha256",
            "fava_build_source_image_sha256",
        ),
        (
            "rust_base_image_sha256",
            "fava_build_rust_base_image_sha256",
        ),
        ("build_command_sha256", "fava_build_command_sha256"),
        ("target_storage", "fava_build_target_storage"),
        ("subject_digest_origin", "fava_build_subject_digest_origin"),
        (
            "fava_canary_executable_sha256",
            "fava_canary_executable_sha256",
        ),
        (
            "fava_canary_subject_image_sha256",
            "fava_canary_subject_image_sha256",
        ),
        ("source_transport", "fava_build_source_transport"),
        (
            "source_transport_image_sha256",
            "fava_build_source_transport_image_sha256",
        ),
    ] {
        if build.get(build_field) != manifest.get(manifest_field) {
            return Err(CanaryError::new(
                "simple-groups retained Fava build attestation disagreed with the manifest",
            ));
        }
    }
    if required_string(&build, "fava_canary_executable_sha256")? != executable_sha256
        || required_string(&build, "build_command_sha256")?
            != "8e010e7b68d708e96ebc25f34935b42d8e6198436a65cf41e27a60c7765bae08"
        || required_string(&build, "toctou_read_only_attempt")? != "EROFS"
        || required_string(&build, "toctou_deliberate_break")? != "compiled-hostile-bytes"
        || required_string(&build, "source_root")? != "/source"
        || required_string(&build, "target_root")? != "/target"
        || required_string(&build, "compiler_network")? != "none"
        || required_string(&build, "compiler_source_mount")? != "read-only"
        || required_string(&build, "compiler_user")? != "65532:65532"
        || required_string(&build, "target_storage")? != "engine-content-addressed-image"
        || build
            .get("target_maximum_bytes")
            .and_then(Value::as_u64)
            != Some(4_294_967_296)
        || build.get("target_maximum_bytes")
            != manifest.get("fava_build_target_maximum_bytes")
        || required_string(&build, "subject_digest_origin")? != "engine-image"
        || required_string(&build, "source_transport")? != "owned-loopback-registry"
        || required_string(&build, "source_transport_image_sha256")?
            != crate::pinned_build_input::REGISTRY_IMAGE_SHA256
        || !is_lower_hex(
            required_string(&build, "fava_canary_subject_image_sha256")?,
            64,
        )
        || required_string(&build, "fava_canary_subject_image_sha256")?
            .bytes()
            .all(|byte| byte == b'0')
    {
        return Err(CanaryError::new(
            "simple-groups retained Fava build attestation did not prove immutable execution",
        ));
    }
    let source_manifest = snapshot.read(
        Path::new("source/fava-build-source.manifest"),
        1_048_576,
        "Fava build source manifest",
    )?;
    let source_manifest_sha256 = hex::encode(Sha256::digest(source_manifest));
    if source_manifest_sha256
        != required_string(&build, "fava_build_source_manifest_sha256")?
    {
        return Err(CanaryError::new(
            "simple-groups retained compiler-input manifest digest disagreed",
        ));
    }
    verify_source_manifest_claim(source_manifest, &build)
}

fn verify_source_manifest_claim(bytes: &[u8], build: &Value) -> CanaryResult<()> {
    if !bytes.ends_with(b"\n") || bytes.contains(&b'\r') {
        return Err(CanaryError::new(
            "simple-groups compiler-input manifest was not canonical LF text",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(error)?;
    let lines = text.lines().collect::<Vec<_>>();
    let file_count = build
        .get("source_file_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| CanaryError::new("simple-groups build file count was invalid"))?;
    let total_bytes = build
        .get("source_total_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| CanaryError::new("simple-groups build byte count was invalid"))?;
    if file_count == 0
        || file_count > 4_096
        || total_bytes > 67_108_864
        || lines.len() != 5 + usize::try_from(file_count).map_err(error)?
        || lines.first().copied() != Some("format=fava-pinned-source-v1")
        || lines.get(1).copied()
            != Some(format!("revision={}", required_string(build, "fava_revision")?).as_str())
        || lines.get(2).copied()
            != Some(format!("tree={}", required_string(build, "fava_build_tree")?).as_str())
        || lines.get(3).copied() != Some(format!("file_count={file_count}").as_str())
        || lines.get(4).copied() != Some(format!("total_bytes={total_bytes}").as_str())
    {
        return Err(CanaryError::new(
            "simple-groups compiler-input manifest headers disagreed",
        ));
    }
    let mut previous = None;
    let mut observed_total = 0_u64;
    for line in &lines[5..] {
        let fields = line
            .strip_prefix("file=")
            .ok_or_else(|| CanaryError::new("simple-groups compiler-input entry was invalid"))?
            .split('\t')
            .collect::<Vec<_>>();
        if fields.len() != 4
            || !matches!(fields[0], "100644" | "100755")
            || !crate::croissant_simple_groups_source::is_lower_hex(fields[1], 64)
            || fields[2].parse::<u64>().map_err(error)?.to_string() != fields[2]
            || fields[3].is_empty()
            || fields[3].len() > 512
            || fields[3].starts_with('/')
            || !fields[3]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._/+@=-".contains(&byte))
            || !Path::new(fields[3])
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
            || !crate::pinned_build_input::is_allowed_source_path(fields[3])
            || previous.is_some_and(|value: &str| value >= fields[3])
        {
            return Err(CanaryError::new(
                "simple-groups compiler-input entry was invalid",
            ));
        }
        let size = fields[2].parse::<u64>().map_err(error)?;
        if size > 8_388_608 {
            return Err(CanaryError::new(
                "simple-groups compiler-input file exceeded its bound",
            ));
        }
        observed_total = observed_total
            .checked_add(size)
            .ok_or_else(|| CanaryError::new("simple-groups compiler-input bytes overflowed"))?;
        previous = Some(fields[3]);
    }
    if observed_total != total_bytes {
        return Err(CanaryError::new(
            "simple-groups compiler-input bytes disagreed",
        ));
    }
    Ok(())
}

fn event_identities(manifest: &Value) -> CanaryResult<BTreeSet<String>> {
    let mut identities = BTreeSet::from([
        required_string(manifest, "shared_event_id")?.to_owned(),
        required_string(manifest, "custom_event_id")?.to_owned(),
    ]);
    for identity in exact_strings(manifest, "unique_event_ids", 2)? {
        identities.insert(identity);
    }
    if identities.len() != 4 {
        return Err(CanaryError::new(
            "simple-groups run reused an event identity",
        ));
    }
    Ok(identities)
}

fn verify_scan_classes(manifest: &Value) -> CanaryResult<()> {
    let values = manifest
        .get("secret_scan_classes")
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new("simple-groups manifest omitted scan classes"))?;
    if !values
        .iter()
        .map(Value::as_str)
        .eq(SECRET_SCAN_CLASSES.iter().copied().map(Some))
    {
        return Err(CanaryError::new(
            "simple-groups secret scan classes were incomplete",
        ));
    }
    Ok(())
}

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
