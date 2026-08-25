use fava_write::{Event, WriteIntentError};

const MAX_EVENT_BYTES: usize = 131_072;
const MIN_EVENT_WITH_ONE_TAG_BYTES: usize = 334;
const MAX_TAG_VALUES: usize = (MAX_EVENT_BYTES - MIN_EVENT_WITH_ONE_TAG_BYTES) / 3;
const EMPTY_EVENT_OBJECT_BYTES: usize = 71;
const FIXED_HEX_BYTES: usize = 64 + 64 + 128;

pub(super) fn validate_source(source: &Event) -> Result<(), WriteIntentError> {
    encoded_len(source).map(|_| ())
}

pub(super) fn encoded_len(source: &Event) -> Result<usize, WriteIntentError> {
    let mut bytes = EMPTY_EVENT_OBJECT_BYTES;
    add(&mut bytes, FIXED_HEX_BYTES)?;
    add(&mut bytes, decimal_len(source.created_at.as_secs()))?;
    add(&mut bytes, decimal_len(u64::from(source.kind.as_u16())))?;
    add(&mut bytes, 2)?;

    let mut value_count = 0usize;
    for (tag_index, tag) in source.tags.iter().enumerate() {
        if tag_index > 0 {
            add(&mut bytes, 1)?;
        }
        add(&mut bytes, 2)?;
        for (value_index, value) in tag.as_slice().iter().enumerate() {
            value_count = value_count
                .checked_add(1)
                .ok_or_else(|| encoding("bookmark source tag-value count overflow"))?;
            if value_count > MAX_TAG_VALUES {
                return Err(WriteIntentError::TooLarge {
                    bytes: value_count,
                    maximum: MAX_TAG_VALUES,
                });
            }
            if value_index > 0 {
                add(&mut bytes, 1)?;
            }
            add_json_string(&mut bytes, value)?;
        }
    }
    add_json_string(&mut bytes, &source.content)?;
    Ok(bytes)
}

fn add_json_string(bytes: &mut usize, value: &str) -> Result<(), WriteIntentError> {
    add(bytes, 2)?;
    for character in value.chars() {
        let encoded = match character {
            '"' | '\\' | '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            other => other.len_utf8(),
        };
        add(bytes, encoded)?;
    }
    Ok(())
}

fn add(bytes: &mut usize, amount: usize) -> Result<(), WriteIntentError> {
    *bytes = bytes
        .checked_add(amount)
        .ok_or_else(|| encoding("bookmark source encoded-size overflow"))?;
    if *bytes > MAX_EVENT_BYTES {
        return Err(WriteIntentError::TooLarge {
            bytes: *bytes,
            maximum: MAX_EVENT_BYTES,
        });
    }
    Ok(())
}

fn decimal_len(value: u64) -> usize {
    if value == 0 {
        1
    } else {
        usize::try_from(value.ilog10()).expect("u32 fits usize") + 1
    }
}

fn encoding(reason: &str) -> WriteIntentError {
    WriteIntentError::Encoding(reason.to_owned())
}
