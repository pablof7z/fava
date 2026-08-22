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

fn error(value: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(value.to_string())
}
