//! Pure public NIP-51 bookmark-list semantic edits.
//!
//! The compile-fail examples are external privacy checks for protocol nouns.
//!
//! ```compile_fail
//! use fava_bookmarks::BookmarkList;
//! ```
//!
//! ```compile_fail
//! use fava_bookmarks::Target;
//! ```
//!
//! ```compile_fail
//! use fava_bookmarks::Change;
//! ```
//!
//! ```compile_fail
//! use fava_bookmarks::BookmarkMaterializer;
//! ```

use std::sync::Arc;

use fava_state::EventCoordinate;
use fava_write::{
    Event, EventBuilder, EventId, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent, WriteIntentError,
};

const BOOKMARK_KIND: u16 = 10_003;
const FORMAT: u32 = 1;
const CODEC_VERSION: u8 = 1;
const ADD: u8 = 1;
const REMOVE: u8 = 2;
const EVENT: u8 = 1;
const COORDINATE: u8 = 2;
const MAX_IDENTIFIER_BYTES: usize = 4_096;

mod bounds;

use bounds::MAX_TAGS;

/// Produce a bounded edit that adds one public event bookmark.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the target cannot fit the
/// private versioned codec.
pub fn bookmark_event(
    actor: PublicKey,
    target: EventId,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, Target::Event(target), Operation::Add)
}

/// Produce a bounded edit that removes one public event bookmark.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the target cannot fit the
/// private versioned codec.
pub fn unbookmark_event(
    actor: PublicKey,
    target: EventId,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(actor, Target::Event(target), Operation::Remove)
}

/// Produce a bounded edit that adds one public replaceable-event coordinate.
///
/// # Errors
///
/// Returns an existing write-intent refusal for an ordinary event coordinate,
/// an invalid replaceable coordinate, or an oversized identifier.
pub fn bookmark_coordinate(
    actor: PublicKey,
    target: EventCoordinate,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    validate_target_coordinate(&target)?;
    edit(actor, Target::Coordinate(target), Operation::Add)
}

/// Produce a bounded edit that removes one public replaceable-event coordinate.
///
/// # Errors
///
/// Returns an existing write-intent refusal for an ordinary event coordinate,
/// an invalid replaceable coordinate, or an oversized identifier.
pub fn unbookmark_coordinate(
    actor: PublicKey,
    target: EventCoordinate,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    validate_target_coordinate(&target)?;
    edit(actor, Target::Coordinate(target), Operation::Remove)
}

/// Select the pure public-bookmark materializer for application assembly.
#[must_use]
pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
    Arc::new(BookmarkMaterializer)
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum Target {
    Event(EventId),
    Coordinate(EventCoordinate),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Change {
    operation: Operation,
    target: Target,
}

fn edit(
    actor: PublicKey,
    target: Target,
    operation: Operation,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    let coordinate = EventCoordinate::Replaceable {
        author: actor,
        kind: bookmark_kind(),
        identifier: None,
    };
    let change = encode(&Change {
        operation,
        target: target.clone(),
    })?;
    let inverse = encode(&Change {
        operation: operation.inverse(),
        target,
    })?;
    ReplaceableEventEdit::new(actor, coordinate, FORMAT, change, inverse)
}

fn encode(change: &Change) -> Result<Vec<u8>, WriteIntentError> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[CODEC_VERSION, change.operation.code()]);
    match &change.target {
        Target::Event(id) => {
            bytes.push(EVENT);
            bytes.extend_from_slice(id.as_bytes());
        }
        Target::Coordinate(coordinate) => {
            bytes.push(COORDINATE);
            encode_coordinate(&mut bytes, coordinate)?;
        }
    }
    Ok(bytes)
}

fn encode_coordinate(
    bytes: &mut Vec<u8>,
    coordinate: &EventCoordinate,
) -> Result<(), WriteIntentError> {
    validate_target_coordinate(coordinate)?;
    let EventCoordinate::Replaceable {
        author,
        kind,
        identifier,
    } = coordinate
    else {
        return Err(invalid_coordinate());
    };
    let identifier = identifier.as_deref();
    let identifier_bytes = identifier.unwrap_or_default().as_bytes();
    let length = u16::try_from(identifier_bytes.len()).map_err(|_| WriteIntentError::TooLarge {
        bytes: identifier_bytes.len(),
        maximum: MAX_IDENTIFIER_BYTES,
    })?;
    bytes.extend_from_slice(&kind.as_u16().to_be_bytes());
    bytes.push(u8::from(identifier.is_some()));
    bytes.extend_from_slice(author.as_bytes());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(identifier_bytes);
    Ok(())
}

fn decode(bytes: &[u8]) -> Result<Change, WriteIntentError> {
    if bytes.len() < 3 || bytes[0] != CODEC_VERSION {
        return Err(codec_refusal("unsupported or malformed bookmark edit"));
    }
    let operation = match bytes[1] {
        ADD => Operation::Add,
        REMOVE => Operation::Remove,
        _ => return Err(codec_refusal("unknown bookmark edit operation")),
    };
    let target = match bytes[2] {
        EVENT if bytes.len() == 35 => Target::Event(
            EventId::from_slice(&bytes[3..])
                .map_err(|_| codec_refusal("invalid bookmark event id"))?,
        ),
        COORDINATE => Target::Coordinate(decode_coordinate(&bytes[3..])?),
        EVENT => return Err(codec_refusal("malformed bookmark event target")),
        _ => return Err(codec_refusal("unknown bookmark target kind")),
    };
    Ok(Change { operation, target })
}

fn decode_coordinate(bytes: &[u8]) -> Result<EventCoordinate, WriteIntentError> {
    if bytes.len() < 37 {
        return Err(codec_refusal("malformed bookmark coordinate"));
    }
    let kind = Kind::from_u16(u16::from_be_bytes([bytes[0], bytes[1]]));
    let has_identifier = match bytes[2] {
        0 => false,
        1 => true,
        _ => return Err(codec_refusal("invalid bookmark coordinate identifier flag")),
    };
    let author = PublicKey::from_slice(&bytes[3..35])
        .map_err(|_| codec_refusal("invalid bookmark coordinate author"))?;
    let length = usize::from(u16::from_be_bytes([bytes[35], bytes[36]]));
    if length > MAX_IDENTIFIER_BYTES || bytes.len() != 37 + length {
        return Err(codec_refusal(
            "invalid bookmark coordinate identifier length",
        ));
    }
    let identifier = std::str::from_utf8(&bytes[37..])
        .map_err(|_| codec_refusal("bookmark coordinate identifier is not UTF-8"))?;
    let coordinate = EventCoordinate::Replaceable {
        author,
        kind,
        identifier: has_identifier.then(|| identifier.to_owned()),
    };
    validate_target_coordinate(&coordinate)?;
    Ok(coordinate)
}

fn decode_edit(edit: &ReplaceableEventEdit) -> Result<Change, WriteIntentError> {
    validate_edit_coordinate(edit)?;
    if edit.format() != FORMAT {
        return Err(codec_refusal("unsupported bookmark edit format"));
    }
    let change = decode(edit.change())?;
    let inverse = decode(edit.inverse_change())?;
    if inverse.target != change.target || inverse.operation != change.operation.inverse() {
        return Err(codec_refusal("bookmark edit inverse does not match change"));
    }
    Ok(change)
}

fn validate_edit_coordinate(edit: &ReplaceableEventEdit) -> Result<(), WriteIntentError> {
    match edit.coordinate() {
        EventCoordinate::Replaceable {
            author,
            kind,
            identifier: None,
        } if *author == edit.actor() && *kind == bookmark_kind() => Ok(()),
        _ => Err(WriteIntentError::InvalidEvent(
            "bookmark edit requires its actor's kind-10003 coordinate".to_owned(),
        )),
    }
}

fn validate_target_coordinate(coordinate: &EventCoordinate) -> Result<(), WriteIntentError> {
    let EventCoordinate::Replaceable {
        kind, identifier, ..
    } = coordinate
    else {
        return Err(invalid_coordinate());
    };
    if identifier
        .as_ref()
        .is_some_and(|value| value.len() > MAX_IDENTIFIER_BYTES)
    {
        return Err(WriteIntentError::TooLarge {
            bytes: identifier.as_ref().map_or(0, String::len),
            maximum: MAX_IDENTIFIER_BYTES,
        });
    }
    let valid = match identifier {
        None => kind.is_replaceable(),
        Some(_) => kind.is_addressable(),
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_coordinate())
    }
}

struct BookmarkMaterializer;

impl ReplaceableEventMaterializer for BookmarkMaterializer {
    fn kind(&self) -> Kind {
        bookmark_kind()
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
        let tags = apply(source_tags, &change)?;
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
    bounds::validate_source(source)?;
    source
        .verify()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
    if source.pubkey != edit.actor() || source.kind != bookmark_kind() {
        return Err(WriteIntentError::InvalidEvent(
            "bookmark source actor or kind does not match edit coordinate".to_owned(),
        ));
    }
    if created_at <= source.created_at {
        return Err(WriteIntentError::InvalidEvent(
            "bookmark materialization timestamp must succeed its source".to_owned(),
        ));
    }
    Ok((&source.content, source.tags.as_slice()))
}

fn apply(source: &[Tag], change: &Change) -> Result<Vec<Tag>, WriteIntentError> {
    let mut matching = 0usize;
    let mut retained = None;
    for tag in source.iter().filter(|tag| tag_targets(tag, &change.target)) {
        matching = matching
            .checked_add(1)
            .ok_or_else(|| codec_refusal("bookmark target count overflow"))?;
        retained = Some(match retained {
            None => tag,
            Some(current) => canonical_tag(current, tag),
        });
    }
    let capacity = source
        .len()
        .checked_sub(matching)
        .and_then(|without_matches| {
            without_matches.checked_add(usize::from(change.operation == Operation::Add))
        })
        .ok_or_else(|| codec_refusal("bookmark output tag count overflow"))?;
    if capacity > MAX_TAGS {
        return Err(WriteIntentError::TooLarge {
            bytes: capacity,
            maximum: MAX_TAGS,
        });
    }
    let mut found = false;
    let mut tags = Vec::with_capacity(capacity);
    for tag in source {
        if tag_targets(tag, &change.target) {
            if change.operation == Operation::Add && !found {
                tags.push(retained.expect("matching target was retained").clone());
                found = true;
            }
        } else {
            tags.push(tag.clone());
        }
    }
    if change.operation == Operation::Add && !found {
        tags.push(target_tag(&change.target)?);
    }
    debug_assert_eq!(tags.len(), capacity);
    Ok(tags)
}

fn canonical_tag<'a>(left: &'a Tag, right: &'a Tag) -> &'a Tag {
    match left.as_slice().len().cmp(&right.as_slice().len()) {
        std::cmp::Ordering::Greater => left,
        std::cmp::Ordering::Equal if left <= right => left,
        std::cmp::Ordering::Less | std::cmp::Ordering::Equal => right,
    }
}

fn tag_targets(tag: &Tag, target: &Target) -> bool {
    let values = tag.as_slice();
    match target {
        Target::Event(id) => {
            values.first().map(String::as_str) == Some("e")
                && values
                    .get(1)
                    .and_then(|value| EventId::from_hex(value).ok())
                    == Some(*id)
        }
        Target::Coordinate(coordinate) => {
            values.first().map(String::as_str) == Some("a")
                && values.get(1) == Some(&coordinate_text(coordinate))
        }
    }
}

fn target_tag(target: &Target) -> Result<Tag, WriteIntentError> {
    match target {
        Target::Event(id) => Ok(Tag::event(*id)),
        Target::Coordinate(coordinate) => Tag::parse(["a", &coordinate_text(coordinate)])
            .map_err(|error| codec_refusal(&error.to_string())),
    }
}

fn coordinate_text(coordinate: &EventCoordinate) -> String {
    match coordinate {
        EventCoordinate::Replaceable {
            author,
            kind,
            identifier,
        } => format!(
            "{}:{}:{}",
            kind.as_u16(),
            author.to_hex(),
            identifier.as_deref().unwrap_or_default()
        ),
        EventCoordinate::Event(_) => String::new(),
    }
}

fn build(
    actor: PublicKey,
    content: &str,
    tags: Vec<Tag>,
    created_at: Timestamp,
) -> Result<UnsignedEvent, WriteIntentError> {
    let mut builder = EventBuilder::new(actor, bookmark_kind())
        .created_at(created_at)
        .content(content);
    for tag in tags {
        builder = builder.tag(tag);
    }
    builder.build().map_err(bounds::map_build_error)
}

fn validate_output(
    edit: &ReplaceableEventEdit,
    event: &UnsignedEvent,
    created_at: Timestamp,
) -> Result<(), WriteIntentError> {
    if event.pubkey != edit.actor()
        || event.kind != bookmark_kind()
        || event.created_at != created_at
    {
        return Err(WriteIntentError::InvalidEvent(
            "bookmark materializer produced the wrong actor, kind, or timestamp".to_owned(),
        ));
    }
    event
        .verify_id()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))
}

fn bookmark_kind() -> Kind {
    Kind::from_u16(BOOKMARK_KIND)
}

fn invalid_coordinate() -> WriteIntentError {
    WriteIntentError::InvalidEvent(
        "bookmark target requires a valid replaceable coordinate".to_owned(),
    )
}

fn codec_refusal(reason: &str) -> WriteIntentError {
    WriteIntentError::Encoding(reason.to_owned())
}

#[cfg(test)]
mod tests;
