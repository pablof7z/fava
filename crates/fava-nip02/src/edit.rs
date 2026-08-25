//! Bounded NIP-02 change encoding and lossless materialization.

use std::fmt;
use std::sync::Arc;

use fava_state::RelayUrl;
use fava_write::{
    EventBuilder, EventValue, Kind, PublicKey, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Tag, Timestamp, UnsignedEvent, WriteIntentError,
};

const ADD: u8 = 1;
const REMOVE: u8 = 2;
const ADD_WITH_METADATA: u8 = 3;
const CODEC_LEN: usize = 33;
const MAX_EDIT_BYTES: usize = 131_072;
const MAX_TARGET_TEXT_BYTES: usize = 69;

use crate::bounds;

/// Produce one bounded edit that follows `target` in a kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
#[allow(clippy::needless_pass_by_value)]
pub fn follow(target: impl fmt::Display) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(parse_target(&target)?, Operation::Add)
}

/// Produce one bounded edit that removes `target` from a kind-3 list.
///
/// # Errors
///
/// Returns an existing write-intent refusal if the private codec cannot encode
/// the target within the neutral edit bound.
#[allow(clippy::needless_pass_by_value)]
pub fn unfollow(target: impl fmt::Display) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(parse_target(&target)?, Operation::Remove)
}

/// Produce one bounded edit that follows `target` with optional NIP-02 metadata.
///
/// Existing matching rows remain byte-for-byte authoritative; supplied metadata
/// is used only when materialization appends a missing target.
///
/// # Errors
///
/// Returns a typed refusal when the target or private metadata encoding is
/// invalid or exceeds its neutral edit bound.
#[allow(clippy::needless_pass_by_value)]
pub fn follow_with(
    target: impl fmt::Display,
    relay: Option<RelayUrl>,
    petname: Option<&str>,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    let target = parse_target(&target)?;
    if relay.is_none() && petname.is_none() {
        return edit(target, Operation::Add);
    }
    metadata_encoded_len(relay.as_ref(), petname)?;
    edit(
        target,
        Operation::AddWithMetadata {
            relay,
            petname: petname.map(str::to_owned),
        },
    )
}

/// Select the pure NIP-02 materializer for application assembly.
#[must_use]
pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
    Arc::new(Nip02Materializer)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Operation {
    Add,
    AddWithMetadata {
        relay: Option<RelayUrl>,
        petname: Option<String>,
    },
    Remove,
}

impl Operation {
    const fn code(&self) -> u8 {
        match self {
            Self::Add => ADD,
            Self::AddWithMetadata { .. } => ADD_WITH_METADATA,
            Self::Remove => REMOVE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Change {
    operation: Operation,
    target: PublicKey,
}

fn edit(target: PublicKey, operation: Operation) -> Result<ReplaceableEventEdit, WriteIntentError> {
    let change = encode(&Change { operation, target })?;
    ReplaceableEventEdit::new(Kind::ContactList, None, change)
}

fn encode(change: &Change) -> Result<Vec<u8>, WriteIntentError> {
    let (relay, petname) = if let Operation::AddWithMetadata { relay, petname } = &change.operation
    {
        (relay.as_ref(), petname.as_deref())
    } else {
        let mut bytes = Vec::with_capacity(CODEC_LEN);
        bytes.push(change.operation.code());
        bytes.extend_from_slice(change.target.as_bytes());
        return Ok(bytes);
    };
    let encoded_len = metadata_encoded_len(relay, petname)?;
    let relay = relay.map_or("", RelayUrl::as_str);
    let relay_len = u32::try_from(relay.len()).map_err(|_| edit_too_large(relay.len()))?;
    let petname_len = petname
        .map(str::len)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| edit_too_large(petname.map_or(0, str::len)))?;
    let mut bytes = Vec::with_capacity(encoded_len);
    bytes.push(change.operation.code());
    bytes.extend_from_slice(change.target.as_bytes());
    bytes.extend_from_slice(&relay_len.to_be_bytes());
    bytes.extend_from_slice(relay.as_bytes());
    match (petname, petname_len) {
        (Some(petname), Some(petname_len)) => {
            bytes.push(1);
            bytes.extend_from_slice(&petname_len.to_be_bytes());
            bytes.extend_from_slice(petname.as_bytes());
        }
        (None, None) => bytes.push(0),
        _ => unreachable!("petname length follows petname presence"),
    }
    Ok(bytes)
}

fn metadata_encoded_len(
    relay: Option<&RelayUrl>,
    petname: Option<&str>,
) -> Result<usize, WriteIntentError> {
    let relay_len = relay.map_or(0, |relay| relay.as_str().len());
    let petname_len = petname.map_or(0, str::len);
    let encoded_len = CODEC_LEN
        .checked_add(4)
        .and_then(|len| len.checked_add(relay_len))
        .and_then(|len| len.checked_add(1))
        .and_then(|len| len.checked_add(usize::from(petname.is_some()) * 4))
        .and_then(|len| len.checked_add(petname_len))
        .ok_or_else(|| codec_refusal("NIP-02 edit size overflow"))?;
    if encoded_len > MAX_EDIT_BYTES {
        return Err(edit_too_large(encoded_len));
    }
    Ok(encoded_len)
}

fn decode(bytes: &[u8]) -> Result<Change, WriteIntentError> {
    if bytes.len() < CODEC_LEN {
        return Err(codec_refusal("malformed NIP-02 edit"));
    }
    let target = PublicKey::from_slice(&bytes[1..CODEC_LEN])
        .map_err(|_| codec_refusal("invalid NIP-02 target public key"))?;
    let operation = match (bytes[0], bytes.len()) {
        (ADD, CODEC_LEN) => Operation::Add,
        (REMOVE, CODEC_LEN) => Operation::Remove,
        (ADD_WITH_METADATA, _) => decode_metadata(&bytes[CODEC_LEN..])?,
        _ => return Err(codec_refusal("unknown NIP-02 edit operation")),
    };
    Ok(Change { operation, target })
}

fn decode_metadata(bytes: &[u8]) -> Result<Operation, WriteIntentError> {
    let (relay_bytes, remaining) = take_length_prefixed(bytes)?;
    let relay_text = std::str::from_utf8(relay_bytes)
        .map_err(|_| codec_refusal("invalid NIP-02 relay encoding"))?;
    let relay = if relay_text.is_empty() {
        None
    } else {
        Some(RelayUrl::parse(relay_text).map_err(|_| codec_refusal("invalid NIP-02 relay hint"))?)
    };
    let Some((&presence, remaining)) = remaining.split_first() else {
        return Err(codec_refusal("malformed NIP-02 metadata edit"));
    };
    let petname = match presence {
        0 if remaining.is_empty() => None,
        1 => {
            let (petname, trailing) = take_length_prefixed(remaining)?;
            if !trailing.is_empty() {
                return Err(codec_refusal("trailing NIP-02 metadata edit bytes"));
            }
            Some(
                std::str::from_utf8(petname)
                    .map_err(|_| codec_refusal("invalid NIP-02 petname encoding"))?
                    .to_owned(),
            )
        }
        _ => return Err(codec_refusal("invalid NIP-02 petname presence")),
    };
    Ok(Operation::AddWithMetadata { relay, petname })
}

fn take_length_prefixed(bytes: &[u8]) -> Result<(&[u8], &[u8]), WriteIntentError> {
    let prefix: [u8; 4] = bytes
        .get(..4)
        .ok_or_else(|| codec_refusal("truncated NIP-02 metadata length"))?
        .try_into()
        .map_err(|_| codec_refusal("invalid NIP-02 metadata length"))?;
    let length = usize::try_from(u32::from_be_bytes(prefix))
        .map_err(|_| codec_refusal("NIP-02 metadata length overflow"))?;
    let end = 4usize
        .checked_add(length)
        .ok_or_else(|| codec_refusal("NIP-02 metadata length overflow"))?;
    let value = bytes
        .get(4..end)
        .ok_or_else(|| codec_refusal("truncated NIP-02 metadata value"))?;
    Ok((value, &bytes[end..]))
}

fn parse_target(target: &impl fmt::Display) -> Result<PublicKey, WriteIntentError> {
    let mut text = BoundedTargetText::new();
    if fmt::write(&mut text, format_args!("{target}")).is_err() {
        if text.exceeded {
            return Err(WriteIntentError::TooLarge {
                bytes: text.attempted,
                maximum: MAX_TARGET_TEXT_BYTES,
            });
        }
        return Err(WriteIntentError::InvalidEvent(
            "invalid NIP-02 target public key".to_owned(),
        ));
    }
    PublicKey::parse(&text.value)
        .map_err(|_| WriteIntentError::InvalidEvent("invalid NIP-02 target public key".to_owned()))
}

struct BoundedTargetText {
    value: String,
    attempted: usize,
    exceeded: bool,
}

impl BoundedTargetText {
    fn new() -> Self {
        Self {
            value: String::with_capacity(MAX_TARGET_TEXT_BYTES),
            attempted: 0,
            exceeded: false,
        }
    }
}

impl fmt::Write for BoundedTargetText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.attempted = self.value.len().saturating_add(value.len());
        if self.attempted > MAX_TARGET_TEXT_BYTES {
            self.exceeded = true;
            return Err(fmt::Error);
        }
        self.value.write_str(value)
    }
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
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        let change = decode_edit(edit)?;
        let (content, source_tags) = qualified_source(author, source, created_at)?;
        let tags = apply(source_tags, &change)?;
        let event = build(author, content, tags, created_at)?;
        validate_output(author, &event, created_at)?;
        Ok(event)
    }
}

fn qualified_source(
    author: PublicKey,
    source: Option<&EventValue>,
    created_at: Timestamp,
) -> Result<(&str, &[Tag]), WriteIntentError> {
    let Some(source) = source else {
        return Ok(("", &[]));
    };
    bounds::validate_value_source(source)?;
    match source {
        EventValue::Signed(event) => event
            .verify()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?,
        EventValue::Unsigned(event) => event
            .verify_id()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?,
    }
    if source.author() != author || source.kind() != Kind::ContactList {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 source author or kind does not match accepted write".to_owned(),
        ));
    }
    if created_at <= source.created_at() {
        return Err(WriteIntentError::InvalidEvent(
            "NIP-02 materialization timestamp must succeed its source".to_owned(),
        ));
    }
    let content = match source {
        EventValue::Unsigned(event) => event.content.as_str(),
        EventValue::Signed(event) => event.content.as_str(),
    };
    Ok((content, source.tags()))
}

fn apply(source: &[Tag], change: &Change) -> Result<Vec<Tag>, WriteIntentError> {
    let matching = source
        .iter()
        .filter(|tag| tag_targets(tag, change.target))
        .count();
    let retained = usize::from(matches!(
        change.operation,
        Operation::Add | Operation::AddWithMetadata { .. }
    ));
    let capacity = source
        .len()
        .checked_sub(matching)
        .and_then(|without_matches| without_matches.checked_add(retained))
        .ok_or_else(|| codec_refusal("NIP-02 output tag count overflow"))?;
    let mut found = false;
    let mut tags = Vec::with_capacity(capacity);
    for tag in source {
        if tag_targets(tag, change.target) {
            if !matches!(change.operation, Operation::Remove) && !found {
                tags.push(tag.clone());
                found = true;
            }
        } else {
            tags.push(tag.clone());
        }
    }
    if !matches!(change.operation, Operation::Remove) && !found {
        tags.push(canonical_row(change)?);
    }
    debug_assert_eq!(tags.len(), capacity);
    Ok(tags)
}

fn canonical_row(change: &Change) -> Result<Tag, WriteIntentError> {
    let mut values = vec!["p".to_owned(), change.target.to_hex()];
    if let Operation::AddWithMetadata { relay, petname } = &change.operation {
        if relay.is_some() || petname.is_some() {
            values.push(relay.as_ref().map_or("", RelayUrl::as_str).to_owned());
        }
        if let Some(petname) = petname {
            values.push(petname.clone());
        }
    }
    Tag::parse(values).map_err(|_| codec_refusal("invalid NIP-02 contact row"))
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
    author: PublicKey,
    content: &str,
    tags: Vec<Tag>,
    created_at: Timestamp,
) -> Result<UnsignedEvent, WriteIntentError> {
    let mut builder = EventBuilder::new(author, Kind::ContactList)
        .created_at(created_at)
        .content(content);
    for tag in tags {
        builder = builder.tag(tag);
    }
    builder.build().map_err(WriteIntentError::from)
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

fn codec_refusal(reason: &str) -> WriteIntentError {
    WriteIntentError::Encoding(reason.to_owned())
}

fn edit_too_large(bytes: usize) -> WriteIntentError {
    WriteIntentError::TooLarge {
        bytes,
        maximum: MAX_EDIT_BYTES,
    }
}
