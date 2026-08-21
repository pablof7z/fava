//! Pure NIP-02 follow-list semantic edits.
//!
//! The compile-fail examples are external privacy checks for protocol nouns.
//!
//! ```compile_fail
//! use fava_nip02::ContactList;
//! ```
//!
//! ```compile_fail
//! use fava_nip02::Change;
//! ```
//!
//! ```compile_fail
//! use fava_nip02::Nip02Materializer;
//! ```

use std::sync::Arc;

use fava_write::{
    Event, EventBuildError, EventBuilder, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent, WriteIntentError,
};

const ADD: u8 = 1;
const REMOVE: u8 = 2;
const CODEC_LEN: usize = 33;
mod bounds;

use bounds::MAX_TAGS;

/// Produce one bounded edit that follows `target` in a kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
pub fn follow(target: PublicKey) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(target, Operation::Add)
}

/// Produce one bounded edit that removes `target` from a kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
pub fn unfollow(target: PublicKey) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(target, Operation::Remove)
}

/// Select the pure NIP-02 materializer for application assembly.
#[must_use]
pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
    Arc::new(Nip02Materializer)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Operation {
    Add,
    Remove,
}

impl Operation {
    const fn code(self) -> u8 {
        match self {
            Self::Add => ADD,
            Self::Remove => REMOVE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Change {
    operation: Operation,
    target: PublicKey,
}

fn edit(target: PublicKey, operation: Operation) -> Result<ReplaceableEventEdit, WriteIntentError> {
    ReplaceableEventEdit::new(
        Kind::ContactList,
        None,
        encode(Change { operation, target }),
    )
}

fn encode(change: Change) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(CODEC_LEN);
    bytes.push(change.operation.code());
    bytes.extend_from_slice(change.target.as_bytes());
    bytes
}

fn decode(bytes: &[u8]) -> Result<Change, WriteIntentError> {
    if bytes.len() != CODEC_LEN {
        return Err(codec_refusal("malformed NIP-02 edit"));
    }
    let operation = match bytes[0] {
        ADD => Operation::Add,
        REMOVE => Operation::Remove,
        _ => return Err(codec_refusal("unknown NIP-02 edit operation")),
    };
    let target = PublicKey::from_slice(&bytes[1..])
        .map_err(|_| codec_refusal("invalid NIP-02 target public key"))?;
    Ok(Change { operation, target })
}

fn decode_edit(edit: &ReplaceableEventEdit) -> Result<Change, WriteIntentError> {
    validate_coordinate(edit)?;
    decode(edit.change())
}

fn validate_coordinate(edit: &ReplaceableEventEdit) -> Result<(), WriteIntentError> {
    if edit.kind() == Kind::ContactList && edit.identifier().is_none() {
        Ok(())
    } else {
        Err(WriteIntentError::InvalidEvent(
            "NIP-02 edit requires a non-addressable kind-3 coordinate".to_owned(),
        ))
    }
}

struct Nip02Materializer;

impl ReplaceableEventMaterializer for Nip02Materializer {
    fn kind(&self) -> Kind {
        Kind::ContactList
    }

    fn supports(&self, edit: &ReplaceableEventEdit) -> bool {
        decode_edit(edit).is_ok()
    }

    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        let change = decode_edit(edit)?;
        let (content, source_tags) = qualified_source(author, source, created_at)?;
        let tags = apply(source_tags, change)?;
        let event = build(author, content, tags, created_at)?;
        validate_output(author, &event, created_at)?;
        Ok(event)
    }
}

fn qualified_source(
    author: PublicKey,
    source: Option<&Event>,
    created_at: Timestamp,
) -> Result<(&str, &[Tag]), WriteIntentError> {
    let Some(source) = source else {
        return Ok(("", &[]));
    };
    bounds::validate_source(source)?;
    source
        .verify()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
    if source.pubkey != author || source.kind != Kind::ContactList {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 source author or kind does not match accepted write".to_owned(),
        ));
    }
    if created_at <= source.created_at {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 materialization timestamp must succeed its source".to_owned(),
        ));
    }
    Ok((&source.content, source.tags.as_slice()))
}

fn apply(source: &[Tag], change: Change) -> Result<Vec<Tag>, WriteIntentError> {
    let matching = source
        .iter()
        .filter(|tag| tag_targets(tag, change.target))
        .count();
    let retained = usize::from(change.operation == Operation::Add);
    let capacity = source
        .len()
        .checked_sub(matching)
        .and_then(|without_matches| without_matches.checked_add(retained))
        .ok_or_else(|| codec_refusal("NIP-02 output tag count overflow"))?;
    if capacity > MAX_TAGS {
        return Err(WriteIntentError::TooLarge {
            bytes: capacity,
            maximum: MAX_TAGS,
        });
    }
    let mut found = false;
    let mut tags = Vec::with_capacity(capacity);
    for tag in source {
        if tag_targets(tag, change.target) {
            if change.operation == Operation::Add && !found {
                tags.push(tag.clone());
                found = true;
            }
        } else {
            tags.push(tag.clone());
        }
    }
    if change.operation == Operation::Add && !found {
        tags.push(Tag::public_key(change.target));
    }
    debug_assert_eq!(tags.len(), capacity);
    Ok(tags)
}

fn tag_targets(tag: &Tag, target: PublicKey) -> bool {
    let values = tag.as_slice();
    values.first().map(String::as_str) == Some("p")
        && values
            .get(1)
            .and_then(|value| PublicKey::from_hex(value).ok())
            == Some(target)
}

fn build(
    actor: PublicKey,
    content: &str,
    tags: Vec<Tag>,
    created_at: Timestamp,
) -> Result<UnsignedEvent, WriteIntentError> {
    let mut builder = EventBuilder::new(actor, Kind::ContactList)
        .created_at(created_at)
        .content(content);
    for tag in tags {
        builder = builder.tag(tag);
    }
    builder.build().map_err(map_build_error)
}

fn validate_output(
    author: PublicKey,
    event: &UnsignedEvent,
    created_at: Timestamp,
) -> Result<(), WriteIntentError> {
    if event.pubkey != author || event.kind != Kind::ContactList || event.created_at != created_at {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 materializer produced the wrong author, kind, or timestamp".to_owned(),
        ));
    }
    event
        .verify_id()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
}

fn map_build_error(error: EventBuildError) -> WriteIntentError {
    match error {
        EventBuildError::TooManyTags { actual, maximum } => WriteIntentError::TooLarge {
            bytes: actual,
            maximum,
        },
        EventBuildError::TooLarge { bytes, maximum } => {
            WriteIntentError::TooLarge { bytes, maximum }
        }
        EventBuildError::Encoding(reason) => WriteIntentError::Encoding(reason),
    }
}

fn codec_refusal(reason: &str) -> WriteIntentError {
    WriteIntentError::Encoding(reason.to_owned())
}

#[cfg(test)]
mod tests;
