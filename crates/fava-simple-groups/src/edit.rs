//! Kind-10009 Simple Group List edits: encode, decode, and materialize.
//!
//! The five saved-list operations (save/remove/rename a group, save/remove a
//! bare relay) are packed into the private binary format read and written by
//! [`encode`]/[`decode_edit`], then folded onto the previous kind-10009 tag
//! set by [`apply`]. Fava installs the materializer that reaches that logic.

use std::sync::Arc;

use fava_write::{
    EventBuilder, EventValue, Kind, PublicKey, ReplaceableEventEdit, ReplaceableEventMaterializer,
    Tag, Timestamp, UnsignedEvent, WriteIntentError,
};
use nostr::types::RelayUrl;

use crate::SimpleGroup;

const SAVED_KIND: u16 = 10_009;
const SAVE_SIMPLE_GROUP: u8 = 1;
const REMOVE_SIMPLE_GROUP: u8 = 2;
const RENAME_SIMPLE_GROUP: u8 = 3;
const SAVE_RELAY: u8 = 4;
const REMOVE_RELAY: u8 = 5;

/// Produce a kind-10009 edit ensuring one entry for every selected group relay.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, save_simple_group};
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("photos", vec![relay])?;
///
/// let with_name = save_simple_group(&group, Some("Photography"))?;
/// let unnamed = save_simple_group(&group, None)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the private edit or neutral edit value is refused.
pub fn save_simple_group(
    group: &SimpleGroup,
    display_name: Option<&str>,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(
        SAVE_SIMPLE_GROUP,
        group.id(),
        &group.relays().collect::<Vec<_>>(),
        display_name,
    )
}

/// Produce a kind-10009 edit removing every selected id/relay entry.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, remove_saved_simple_group};
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("photos", vec![relay])?;
///
/// let edit = remove_saved_simple_group(&group)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the private edit or neutral edit value is refused.
pub fn remove_saved_simple_group(
    group: &SimpleGroup,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(
        REMOVE_SIMPLE_GROUP,
        group.id(),
        &group.relays().collect::<Vec<_>>(),
        None,
    )
}

/// Produce a kind-10009 edit naming every selected id/relay entry.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::{SimpleGroup, rename_saved_simple_group};
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let group = SimpleGroup::new("photos", vec![relay])?;
///
/// let edit = rename_saved_simple_group(&group, "Photography")?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the private edit or neutral edit value is refused.
pub fn rename_saved_simple_group(
    group: &SimpleGroup,
    display_name: &str,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(
        RENAME_SIMPLE_GROUP,
        group.id(),
        &group.relays().collect::<Vec<_>>(),
        Some(display_name),
    )
}

/// Produce a kind-10009 edit ensuring one exact inert `r` relay entry.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::save_relay;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let edit = save_relay(relay)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the private edit or neutral edit value is refused.
pub fn save_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(SAVE_RELAY, "", &[relay], None)
}

/// Produce a kind-10009 edit removing every exact inert `r` relay entry.
///
/// # Examples
///
/// ```
/// # use fava_simple_groups::remove_saved_relay;
/// # use nostr::types::RelayUrl;
/// let relay = RelayUrl::parse("wss://relay.example")?;
/// let edit = remove_saved_relay(relay)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns [`WriteIntentError`] when the private edit or neutral edit value is refused.
pub fn remove_saved_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, WriteIntentError> {
    edit(REMOVE_RELAY, "", &[relay], None)
}

#[must_use]
pub(crate) fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
    Arc::new(SavedGroupListMaterializer)
}

/// Wraps one encoded changeset as a non-addressable kind-10009 edit.
fn edit(
    operation: u8,
    id: &str,
    relays: &[RelayUrl],
    display_name: Option<&str>,
) -> Result<ReplaceableEventEdit, WriteIntentError> {
    ReplaceableEventEdit::new(
        saved_kind(),
        None,
        encode(operation, id, relays, display_name)?,
    )
}

/// Serializes one changeset operation to the private byte format `decode_edit` reads back.
fn encode(
    operation: u8,
    id: &str,
    relays: &[RelayUrl],
    display_name: Option<&str>,
) -> Result<Vec<u8>, WriteIntentError> {
    let mut bytes = Vec::new();
    match operation {
        SAVE_SIMPLE_GROUP => {
            bytes.push(SAVE_SIMPLE_GROUP);
            encode_simple_group(&mut bytes, id, relays)?;
            encode_optional(&mut bytes, display_name)?;
        }
        REMOVE_SIMPLE_GROUP => {
            bytes.push(REMOVE_SIMPLE_GROUP);
            encode_simple_group(&mut bytes, id, relays)?;
        }
        RENAME_SIMPLE_GROUP => {
            bytes.push(RENAME_SIMPLE_GROUP);
            encode_simple_group(&mut bytes, id, relays)?;
            encode_text(
                &mut bytes,
                display_name.ok_or_else(|| codec_refusal("missing saved-list display name"))?,
            )?;
        }
        SAVE_RELAY | REMOVE_RELAY => {
            bytes.push(operation);
            let relay = relays
                .first()
                .ok_or_else(|| codec_refusal("missing saved-list relay"))?;
            encode_text(&mut bytes, relay.as_str())?;
        }
        _ => return Err(codec_refusal("unknown saved-list edit operation")),
    }
    Ok(bytes)
}

/// Appends the id, relay count, and each relay in the changeset byte format.
fn encode_simple_group(
    bytes: &mut Vec<u8>,
    id: &str,
    relays: &[RelayUrl],
) -> Result<(), WriteIntentError> {
    encode_text(bytes, id)?;
    let count =
        u16::try_from(relays.len()).map_err(|_| codec_refusal("too many simple group relays"))?;
    bytes.extend_from_slice(&count.to_be_bytes());
    for relay in relays {
        encode_text(bytes, relay.as_str())?;
    }
    Ok(())
}

/// Appends a presence byte, then the value's encoded text if present.
fn encode_optional(bytes: &mut Vec<u8>, value: Option<&str>) -> Result<(), WriteIntentError> {
    if let Some(value) = value {
        bytes.push(1);
        encode_text(bytes, value)
    } else {
        bytes.push(0);
        Ok(())
    }
}

/// Appends a `u32`-length-prefixed UTF-8 string, erroring past `u32::MAX` bytes.
fn encode_text(bytes: &mut Vec<u8>, value: &str) -> Result<(), WriteIntentError> {
    let length = u32::try_from(value.len()).map_err(|_| codec_refusal("edit text is too long"))?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

/// Reverses `encode`, requiring an exact non-addressable kind-10009 coordinate and no trailing bytes.
fn decode_edit(
    edit: &ReplaceableEventEdit,
) -> Result<(u8, String, Vec<RelayUrl>, Option<String>), WriteIntentError> {
    if edit.kind() != saved_kind() || edit.identifier().is_some() {
        return Err(codec_refusal(
            "saved-list edit requires a non-addressable kind-10009 coordinate",
        ));
    }
    let mut input = edit.change();
    let operation = take_byte(&mut input)?;
    let (id, relays, display_name) = match operation {
        SAVE_SIMPLE_GROUP => {
            let (id, relays) = take_simple_group(&mut input)?;
            let display_name = take_optional_text(&mut input)?;
            (id, relays, display_name)
        }
        REMOVE_SIMPLE_GROUP => {
            let (id, relays) = take_simple_group(&mut input)?;
            (id, relays, None)
        }
        RENAME_SIMPLE_GROUP => {
            let (id, relays) = take_simple_group(&mut input)?;
            let display_name = take_text(&mut input)?;
            (id, relays, Some(display_name))
        }
        SAVE_RELAY | REMOVE_RELAY => (String::new(), vec![take_relay(&mut input)?], None),
        _ => return Err(codec_refusal("unknown saved-list edit operation")),
    };
    if input.is_empty() {
        Ok((operation, id, relays, display_name))
    } else {
        Err(codec_refusal("trailing saved-list edit bytes"))
    }
}

/// Pops the first byte, or errors on empty input.
fn take_byte(input: &mut &[u8]) -> Result<u8, WriteIntentError> {
    let Some((&value, remaining)) = input.split_first() else {
        return Err(codec_refusal("truncated saved-list edit"));
    };
    *input = remaining;
    Ok(value)
}

/// Reads a length-prefixed UTF-8 string written by `encode_text`.
fn take_text(input: &mut &[u8]) -> Result<String, WriteIntentError> {
    let length_bytes: [u8; 4] = input
        .get(..4)
        .ok_or_else(|| codec_refusal("truncated saved-list text length"))?
        .try_into()
        .map_err(|_| codec_refusal("invalid saved-list text length"))?;
    let length = usize::try_from(u32::from_be_bytes(length_bytes))
        .map_err(|_| codec_refusal("saved-list text length overflow"))?;
    let end = 4usize
        .checked_add(length)
        .ok_or_else(|| codec_refusal("saved-list text length overflow"))?;
    let value = input
        .get(4..end)
        .ok_or_else(|| codec_refusal("truncated saved-list text"))?;
    *input = &input[end..];
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| codec_refusal("saved-list text is not UTF-8"))
}

/// Reads the presence byte and value written by `encode_optional`.
fn take_optional_text(input: &mut &[u8]) -> Result<Option<String>, WriteIntentError> {
    match take_byte(input)? {
        0 => Ok(None),
        1 => take_text(input).map(Some),
        _ => Err(codec_refusal("invalid saved-list optional text flag")),
    }
}

/// Reads text and parses it as a relay URL.
fn take_relay(input: &mut &[u8]) -> Result<RelayUrl, WriteIntentError> {
    RelayUrl::parse(&take_text(input)?).map_err(|_| codec_refusal("invalid saved-list relay"))
}

/// Reads an id and its relay set, rejecting a zero count and duplicate relays.
fn take_simple_group(input: &mut &[u8]) -> Result<(String, Vec<RelayUrl>), WriteIntentError> {
    let id = take_text(input)?;
    let count_bytes: [u8; 2] = input
        .get(..2)
        .ok_or_else(|| codec_refusal("truncated saved-list relay count"))?
        .try_into()
        .map_err(|_| codec_refusal("invalid saved-list relay count"))?;
    *input = &input[2..];
    let count = usize::from(u16::from_be_bytes(count_bytes));
    if count == 0 {
        return Err(codec_refusal("invalid saved-list relay count"));
    }
    let mut relays = Vec::with_capacity(count);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..count {
        let relay = take_relay(input)?;
        if !seen.insert(relay.clone()) {
            return Err(codec_refusal("duplicate saved-list simple-group relay"));
        }
        relays.push(relay);
    }
    Ok((id, relays))
}

/// The kind-10009 [`ReplaceableEventMaterializer`] used by Fava.
///
/// Turns a decoded saved-list edit back into tags by replaying it against the
/// prior kind-10009 event, so a save/remove/rename only ever touches the
/// entries it names.
struct SavedGroupListMaterializer;

impl ReplaceableEventMaterializer for SavedGroupListMaterializer {
    fn kind(&self) -> Kind {
        saved_kind()
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
        let (operation, id, relays, display_name) = decode_edit(edit)?;
        let (content, source_tags) = qualified_source(author, source, created_at)?;
        let tags = apply(
            source_tags,
            operation,
            &id,
            &relays,
            display_name.as_deref(),
        )?;
        let event =
            EventBuilder::from_parts(author, saved_kind(), created_at, tags, content.to_owned())
                .build()
                .map_err(WriteIntentError::from)?;
        event
            .verify_id()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
        Ok(event)
    }
}

/// Verifies and unpacks the event this materialization supersedes.
///
/// Requires the same author and kind as `source`, and a strictly later
/// `created_at`; returns empty content and tags when there is no prior event.
fn qualified_source(
    author: PublicKey,
    source: Option<&EventValue>,
    created_at: Timestamp,
) -> Result<(&str, &[Tag]), WriteIntentError> {
    let Some(source) = source else {
        return Ok(("", &[]));
    };
    match source {
        EventValue::Signed(event) => event
            .verify()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?,
        EventValue::Unsigned(event) => event
            .verify_id()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?,
    }
    if source.author() != author || source.kind() != saved_kind() {
        return Err(WriteIntentError::InvalidEvent(
            "saved-list source author or kind does not match accepted write".to_owned(),
        ));
    }
    if created_at <= source.created_at() {
        return Err(WriteIntentError::InvalidEvent(
            "saved-list materialization timestamp must succeed its source".to_owned(),
        ));
    }
    let content = match source {
        EventValue::Unsigned(event) => event.content.as_str(),
        EventValue::Signed(event) => event.content.as_str(),
    };
    Ok((content, source.tags()))
}

/// Dispatches one changeset operation to `apply_simple_group` or `apply_relay`.
fn apply(
    source: &[Tag],
    operation: u8,
    id: &str,
    relays: &[RelayUrl],
    display_name: Option<&str>,
) -> Result<Vec<Tag>, WriteIntentError> {
    match operation {
        SAVE_SIMPLE_GROUP | REMOVE_SIMPLE_GROUP | RENAME_SIMPLE_GROUP => {
            apply_simple_group(source, id, relays, operation, display_name)
        }
        SAVE_RELAY => apply_relay(
            source,
            relays
                .first()
                .ok_or_else(|| codec_refusal("missing saved-list relay"))?,
            true,
        ),
        REMOVE_RELAY => apply_relay(
            source,
            relays
                .first()
                .ok_or_else(|| codec_refusal("missing saved-list relay"))?,
            false,
        ),
        _ => Err(codec_refusal("unknown saved-list edit operation")),
    }
}

/// Replays a save/remove/rename against the prior tag set, keeping at most one `group` entry per relay.
fn apply_simple_group(
    source: &[Tag],
    id: &str,
    relays: &[RelayUrl],
    operation: u8,
    display_name: Option<&str>,
) -> Result<Vec<Tag>, WriteIntentError> {
    let selected = relays
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut found = std::collections::BTreeSet::new();
    let mut tags = Vec::with_capacity(source.len().saturating_add(relays.len()));
    for tag in source {
        let Some(relay) = simple_group_target(tag, id).filter(|relay| selected.contains(relay))
        else {
            tags.push(tag.clone());
            continue;
        };
        if !found.insert(relay) {
            continue;
        }
        match operation {
            SAVE_SIMPLE_GROUP => tags.push(tag.clone()),
            REMOVE_SIMPLE_GROUP => {}
            RENAME_SIMPLE_GROUP => tags.push(renamed_tag(
                tag,
                display_name.ok_or_else(|| codec_refusal("missing saved-list display name"))?,
            )?),
            _ => return Err(codec_refusal("invalid saved-list simple-group operation")),
        }
    }
    match operation {
        SAVE_SIMPLE_GROUP => {
            for relay in relays {
                if found.insert(relay.clone()) {
                    tags.push(simple_group_tag(id, relay, display_name)?);
                }
            }
        }
        RENAME_SIMPLE_GROUP => {
            let display_name =
                display_name.ok_or_else(|| codec_refusal("missing saved-list display name"))?;
            for relay in relays {
                if found.insert(relay.clone()) {
                    tags.push(simple_group_tag(id, relay, Some(display_name))?);
                }
            }
        }
        REMOVE_SIMPLE_GROUP => {}
        _ => return Err(codec_refusal("invalid saved-list simple-group operation")),
    }
    Ok(tags)
}

/// Replays a save/remove against the prior `r` tags for one relay.
fn apply_relay(source: &[Tag], relay: &RelayUrl, save: bool) -> Result<Vec<Tag>, WriteIntentError> {
    let mut found = false;
    let mut tags = Vec::with_capacity(source.len().saturating_add(usize::from(save)));
    for tag in source {
        if relay_target(tag).as_ref() == Some(relay) {
            if save && !found {
                tags.push(tag.clone());
                found = true;
            }
        } else {
            tags.push(tag.clone());
        }
    }
    if save && !found {
        tags.push(
            Tag::parse(["r", relay.as_str()]).map_err(|error| codec_refusal(&error.to_string()))?,
        );
    }
    Ok(tags)
}

/// Extracts the relay from a `group` tag matching `id`, if well-formed.
fn simple_group_target(tag: &Tag, id: &str) -> Option<RelayUrl> {
    let values = tag.as_slice();
    (values.len() >= 3
        && values.first().map(String::as_str) == Some("group")
        && values.get(1).map(String::as_str) == Some(id))
    .then(|| values.get(2).and_then(|host| RelayUrl::parse(host).ok()))
    .flatten()
}

/// Extracts the relay from an `r` tag, if well-formed.
fn relay_target(tag: &Tag) -> Option<RelayUrl> {
    let values = tag.as_slice();
    (values.len() >= 2 && values.first().map(String::as_str) == Some("r"))
        .then(|| values.get(1).and_then(|relay| RelayUrl::parse(relay).ok()))
        .flatten()
}

/// Returns `tag` with its display-name value (index 3) set to `name`.
fn renamed_tag(tag: &Tag, name: &str) -> Result<Tag, WriteIntentError> {
    let mut values = tag.as_slice().to_vec();
    if values.len() == 3 {
        values.push(name.to_owned());
    } else {
        name.clone_into(&mut values[3]);
    }
    Tag::parse(values).map_err(|error| codec_refusal(&error.to_string()))
}

/// Builds a `group` tag for `id` and `host`, with an optional display name.
fn simple_group_tag(
    id: &str,
    host: &RelayUrl,
    name: Option<&str>,
) -> Result<Tag, WriteIntentError> {
    let mut values = vec!["group".to_owned(), id.to_owned(), host.as_str().to_owned()];
    if let Some(name) = name {
        values.push(name.to_owned());
    }
    Tag::parse(values).map_err(|error| codec_refusal(&error.to_string()))
}

fn saved_kind() -> Kind {
    Kind::from_u16(SAVED_KIND)
}

fn codec_refusal(reason: &str) -> WriteIntentError {
    WriteIntentError::Encoding(reason.to_owned())
}
