//! Pure NIP-02 follow-list semantic edits.

use std::sync::Arc;

use fava_state::EventCoordinate;
use fava_write::{
    Event, EventBuildError, EventBuilder, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent, WriteIntentError,
};

const FORMAT: u32 = 1;
const CODEC_VERSION: u8 = 1;
const ADD: u8 = 1;
const REMOVE: u8 = 2;
const CODEC_LEN: usize = 34;
const MAX_TAGS: usize = 2_000;
const MAX_EVENT_BYTES: usize = 131_072;

/// Produce one bounded edit that follows `target` in the actor's kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
pub fn follow(
    actor: PublicKey,
    target: PublicKey,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, target, Operation::Add)
}

/// Produce one bounded edit that removes `target` from the actor's kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
pub fn unfollow(
    actor: PublicKey,
    target: PublicKey,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, target, Operation::Remove)
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

    const fn inverse(self) -> Self {
        match self {
            Self::Add => Self::Remove,
            Self::Remove => Self::Add,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Change {
    operation: Operation,
    target: PublicKey,
}

fn edit(
    actor: PublicKey,
    target: PublicKey,
    operation: Operation,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    let coordinate = EventCoordinate::Replaceable {
        author: actor,
        kind: Kind::ContactList,
        identifier: None,
    };
    ReplaceableEventEdit::new(
        actor,
        coordinate,
        FORMAT,
        encode(Change { operation, target }),
        encode(Change {
            operation: operation.inverse(),
            target,
        }),
    )
}

fn encode(change: Change) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(CODEC_LEN);
    bytes.push(CODEC_VERSION);
    bytes.push(change.operation.code());
    bytes.extend_from_slice(change.target.as_bytes());
    bytes
}

fn decode(bytes: &[u8]) -> Result<Change, WriteIntentError> {
    if bytes.len() != CODEC_LEN || bytes[0] != CODEC_VERSION {
        return Err(codec_refusal("unsupported or malformed NIP-02 edit"));
    }
    let operation = match bytes[1] {
        ADD => Operation::Add,
        REMOVE => Operation::Remove,
        _ => return Err(codec_refusal("unknown NIP-02 edit operation")),
    };
    let target = PublicKey::from_slice(&bytes[2..])
        .map_err(|_| codec_refusal("invalid NIP-02 target public key"))?;
    Ok(Change { operation, target })
}

fn decode_edit(edit: &ReplaceableEventEdit) -> Result<Change, WriteIntentError> {
    validate_coordinate(edit)?;
    if edit.format() != FORMAT {
        return Err(codec_refusal("unsupported NIP-02 edit format"));
    }
    let change = decode(edit.change())?;
    let inverse = decode(edit.inverse_change())?;
    if inverse.target != change.target || inverse.operation != change.operation.inverse() {
        return Err(codec_refusal("NIP-02 edit inverse does not match change"));
    }
    Ok(change)
}

fn validate_coordinate(edit: &ReplaceableEventEdit) -> Result<(), WriteIntentError> {
    match edit.coordinate() {
        EventCoordinate::Replaceable {
            author,
            kind: Kind::ContactList,
            identifier: None,
        } if *author == edit.actor() => Ok(()),
        _ => Err(WriteIntentError::InvalidEvent(
            "NIP-02 edit requires its actor's kind-3 coordinate".to_owned(),
        )),
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
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        let change = decode_edit(edit)?;
        let (content, source_tags) = qualified_source(edit, source, created_at)?;
        let tags = apply(source_tags, change)?;
        let event = build(edit.actor(), content, tags, created_at)?;
        validate_output(edit, &event, created_at)?;
        Ok(event)
    }
}

fn qualified_source<'a>(
    edit: &ReplaceableEventEdit,
    source: Option<&'a Event>,
    created_at: Timestamp,
) -> Result<(&'a str, &'a [Tag]), WriteIntentError> {
    let Some(source) = source else {
        return Ok(("", &[]));
    };
    source
        .verify()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
    if source.pubkey != edit.actor() || source.kind != Kind::ContactList {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 source actor or kind does not match edit coordinate".to_owned(),
        ));
    }
    if created_at <= source.created_at {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 materialization timestamp must succeed its source".to_owned(),
        ));
    }
    validate_source_bound(source)?;
    Ok((&source.content, source.tags.as_slice()))
}

fn validate_source_bound(source: &Event) -> Result<(), WriteIntentError> {
    if source.tags.len() > MAX_TAGS {
        return Err(WriteIntentError::TooLarge {
            bytes: source.tags.len(),
            maximum: MAX_TAGS,
        });
    }
    let mut bytes = source.content.len();
    for value in source.tags.iter().flat_map(Tag::as_slice) {
        bytes = bytes
            .checked_add(value.len())
            .ok_or_else(|| codec_refusal("NIP-02 source byte count overflow"))?;
        if bytes > MAX_EVENT_BYTES {
            return Err(WriteIntentError::TooLarge {
                bytes,
                maximum: MAX_EVENT_BYTES,
            });
        }
    }
    Ok(())
}

fn apply(source: &[Tag], change: Change) -> Result<Vec<Tag>, WriteIntentError> {
    let additional = usize::from(change.operation == Operation::Add);
    let capacity = source
        .len()
        .checked_add(additional)
        .ok_or_else(|| codec_refusal("NIP-02 output tag count overflow"))?;
    if capacity > MAX_TAGS + 1 {
        return Err(WriteIntentError::TooLarge {
            bytes: capacity,
            maximum: MAX_TAGS,
        });
    }
    let mut found = false;
    let mut tags = Vec::with_capacity(capacity.min(MAX_TAGS));
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
    if tags.len() > MAX_TAGS {
        return Err(WriteIntentError::TooLarge {
            bytes: tags.len(),
            maximum: MAX_TAGS,
        });
    }
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
    edit: &ReplaceableEventEdit,
    event: &UnsignedEvent,
    created_at: Timestamp,
) -> Result<(), WriteIntentError> {
    if event.pubkey != edit.actor()
        || event.kind != Kind::ContactList
        || event.created_at != created_at
    {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 materializer produced the wrong actor, kind, or timestamp".to_owned(),
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
