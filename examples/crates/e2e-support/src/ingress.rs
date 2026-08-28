//! Shared command-token secret refusal before history or dispatch.

use crate::ShellError;
use crate::result::sensitive_value;

/// Return whether one ordinary command token resembles protected input.
pub(crate) fn looks_secret(word: &str) -> bool {
    sensitive_value(word)
}

/// Return whether one public relay endpoint tries to carry credentials.
pub(crate) fn credential_bearing_relay_url(value: &str) -> bool {
    let authority = value
        .split_once("://")
        .map(|(_, remainder)| remainder.split('/').next().unwrap_or_default())
        .unwrap_or_default();
    authority.contains('@')
        || value.contains('?')
        || value.contains('#')
        || contains_raw_hex_run(value)
}

fn contains_raw_hex_run(value: &str) -> bool {
    value
        .split(|character: char| !character.is_ascii_hexdigit())
        .any(|segment| segment.len() >= 64)
}

/// Reject prompted input with the same contextual raw-key policy as commands.
pub(crate) fn reject_prompted_value(label: &str, value: &str) -> Result<(), ShellError> {
    if looks_secret(value)
        || (raw_hex_key_material(value) && !matches!(label, "public-key" | "event-id"))
    {
        return Err(ShellError::SecretOnCommandLine);
    }
    Ok(())
}

/// Reject raw protected input unless its exact token position owns a public id.
pub(crate) fn reject_unsafe_words(words: &[String]) -> Result<(), ShellError> {
    if words.iter().any(|word| looks_secret(word)) {
        return Err(ShellError::SecretOnCommandLine);
    }
    if let [command, action, _, url, ..] = words
        && command == "relay"
        && action == "add"
        && credential_bearing_relay_url(url)
    {
        return Err(ShellError::SecretOnCommandLine);
    }
    for (index, word) in words.iter().enumerate() {
        if raw_hex_key_material(word) && !hex_is_public_identifier_at(words, index) {
            return Err(ShellError::SecretOnCommandLine);
        }
    }
    Ok(())
}

fn raw_hex_key_material(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_is_public_identifier_at(words: &[String], index: usize) -> bool {
    match (words, index) {
        ([group, member, action, _public_key, ..], 3)
            if group == "group"
                && member == "member"
                && (action == "add" || action == "remove") =>
        {
            true
        }
        ([group, event, action, _event_id], 3)
            if group == "group" && event == "event" && action == "delete" =>
        {
            true
        }
        _ => false,
    }
}
