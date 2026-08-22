fn wire_frames(snapshot: &EvidenceSnapshot, label: &str) -> CanaryResult<(Vec<Value>, u64)> {
    let relative = format!("wire/{label}.jsonl");
    let bytes = snapshot.read(Path::new(&relative), WIRE_LIMIT, "wire log")?;
    if !bytes.ends_with(b"\n") {
        return Err(CanaryError::new("simple-groups wire log was incomplete"));
    }
    let mut frames = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut frame: Value = serde_json::from_slice(line)?;
        if frame.get("frame_type").and_then(Value::as_str) == Some("text") {
            let payload = frame
                .get("payload")
                .and_then(Value::as_str)
                .ok_or_else(|| CanaryError::new("simple-groups text frame omitted payload"))?;
            let decoded: Value = serde_json::from_str(payload.trim_end())?;
            frame
                .as_object_mut()
                .ok_or_else(|| CanaryError::new("simple-groups wire frame was not an object"))?
                .insert("decoded".to_owned(), decoded);
        }
        frames.push(frame);
    }
    if frames.is_empty() {
        return Err(CanaryError::new("simple-groups wire log was empty"));
    }
    frames.sort_by_key(|frame| frame.get("sequence").and_then(Value::as_u64));
    Ok((frames, u64::try_from(bytes.len()).map_err(error)?))
}

fn exact_filter(
    filter: &Map<String, Value>,
    axis: &str,
    group: &str,
    kinds: &[u64],
    limit: u64,
) -> bool {
    filter.len() == 3
        && filter.get(axis) == Some(&json!([group]))
        && filter.get("kinds") == Some(&json!(kinds))
        && filter.get("limit").and_then(Value::as_u64) == Some(limit)
}

fn assign_once(slot: &mut Option<String>, value: &str, label: &str) -> CanaryResult<()> {
    if value.is_empty() || slot.replace(value.to_owned()).is_some() {
        return Err(CanaryError::new(format!(
            "simple-groups wire repeated {label}"
        )));
    }
    Ok(())
}

fn event_at(payload: &Value, index: usize) -> CanaryResult<Event> {
    serde_json::from_value(
        payload
            .get(index)
            .cloned()
            .ok_or_else(|| CanaryError::new("simple-groups EVENT omitted event body"))?,
    )
    .map_err(Into::into)
}

fn select_current(current: &mut Option<Event>, candidate: Event) {
    let replace = current.as_ref().is_none_or(|existing| {
        candidate.created_at > existing.created_at
            || (candidate.created_at == existing.created_at && candidate.id < existing.id)
    });
    if replace {
        *current = Some(candidate);
    }
}

fn has_exact_tag(event: &Event, name: &str, value: &str) -> bool {
    let matches = event
        .tags
        .iter()
        .filter(|tag| tag.as_slice().first().map(String::as_str) == Some(name))
        .collect::<Vec<_>>();
    matches.len() == 1 && matches[0].as_slice().get(1).map(String::as_str) == Some(value)
}

fn has_exact_command_tags(event: &Event, expected: &[[&str; 3]]) -> bool {
    let mut actual = event
        .tags
        .iter()
        .map(|tag| tag.as_slice().to_vec())
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|tag| {
            let mut values = vec![tag[0].to_owned(), tag[1].to_owned()];
            if !tag[2].is_empty() {
                values.push(tag[2].to_owned());
            }
            values
        })
        .collect::<Vec<_>>();
    actual.sort_unstable();
    expected.sort_unstable();
    actual == expected
}

fn has_tag_value(event: &Event, name: &str, value: &str) -> bool {
    event.tags.iter().any(|tag| {
        tag.as_slice().first().map(String::as_str) == Some(name)
            && tag.as_slice().get(1).map(String::as_str) == Some(value)
    })
}

fn string<'a>(value: &'a Value, field: &str) -> CanaryResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CanaryError::new(format!("simple-groups evidence omitted {field}")))
}

fn strings(value: &Value, field: &str, count: usize) -> CanaryResult<Vec<String>> {
    let values = value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| CanaryError::new(format!("simple-groups evidence omitted {field}")))?;
    if values.len() != count {
        return Err(CanaryError::new(format!(
            "simple-groups evidence required exactly {count} {field}"
        )));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| CanaryError::new(format!("simple-groups {field} was not text")))
        })
        .collect()
}

fn error(error: impl std::fmt::Display) -> CanaryError {
    CanaryError::new(error.to_string())
}
