use std::collections::BTreeSet;
use std::sync::Arc;

use fava::{
    EventBuilder, EventValue, Kind, PublicKey, Query, EventEdit,
    EditApplier, Timestamp, UnsignedEvent, WriteIntentError,
};

const KIND: Kind = Kind::Custom(15_001);
const INSERT: u8 = 1;
const REMOVE: u8 = 2;
const MAX_ITEM_BYTES: usize = 256;
const MAX_SOURCE_TAGS: usize = 64;
const MAX_SOURCE_TAG_VALUES: usize = 16;
const MAX_SOURCE_BYTES: usize = 4_096;
const CONTENT_PREFIX: &str = "external-set-v1\n";

/// Return the unrelated non-addressable replaceable kind used by this proof.
#[must_use]
pub const fn external_kind() -> Kind {
    KIND
}

/// Build the capability-owned author-and-kind query fragment.
#[must_use]
pub fn external_query(actor: PublicKey) -> Query {
    Query::events()
        .authors([actor])
        .expect("one external capability author is bounded")
        .kinds([KIND])
        .expect("one external capability kind is bounded")
}

/// Validate one public event value against the capability's typed semantics.
///
/// # Errors
///
/// Returns an existing write-intent refusal for the wrong kind, malformed
/// state, or any content/tag bound violation.
pub fn validate_external_event(event: &EventValue) -> Result<(), WriteIntentError> {
    decode_external_event(event).map(|_| ())
}

/// Decode bounded capability state and preserved opaque content.
///
/// # Errors
///
/// Returns an existing write-intent refusal for the wrong kind, malformed
/// state, or any content/tag bound violation.
pub fn decode_external_event(
    event: &EventValue,
) -> Result<(BTreeSet<String>, String), WriteIntentError> {
    if event.kind() != KIND {
        return Err(WriteIntentError::InvalidEvent(
            "external capability event has the wrong kind".to_owned(),
        ));
    }
    let content = match event {
        EventValue::Unsigned(event) => {
            validate_bounds(
                &event.content,
                event.tags.len(),
                event.tags.iter().map(|tag| value_shape(tag.as_slice())),
            )?;
            &event.content
        }
        EventValue::Signed(event) => {
            validate_bounds(
                &event.content,
                event.tags.len(),
                event.tags.iter().map(|tag| value_shape(tag.as_slice())),
            )?;
            &event.content
        }
    };
    decode_content(content)
}

/// Construct one bounded insertion edit using public Fava values.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the item is malformed or
/// exceeds the capability's private bound.
pub fn insert(item: &str) -> Result<EventEdit, WriteIntentError> {
    edit(item, INSERT)
}

/// Construct one bounded removal edit using public Fava values.
///
/// # Errors
///
/// Returns an existing write-intent refusal when the item is malformed or
/// exceeds the capability's private bound.
pub fn remove(item: &str) -> Result<EventEdit, WriteIntentError> {
    edit(item, REMOVE)
}

/// Return the capability provider behind the public neutral contract.
#[must_use]
pub fn selected_applier() -> Arc<dyn EditApplier> {
    Arc::new(ExternalSetApplier)
}

fn edit(item: &str, operation: u8) -> Result<EventEdit, WriteIntentError> {
    let change = encode_action(operation, item)?;
    EventEdit::new(KIND, None, change)
}

fn encode_action(operation: u8, item: &str) -> Result<Vec<u8>, WriteIntentError> {
    validate_item(item)?;
    let length = u16::try_from(item.len()).map_err(|_| item_refusal())?;
    let mut encoded = Vec::with_capacity(item.len() + 3);
    encoded.push(operation);
    encoded.extend_from_slice(&length.to_be_bytes());
    encoded.extend_from_slice(item.as_bytes());
    Ok(encoded)
}

fn decode_action(bytes: &[u8]) -> Result<(u8, &str), WriteIntentError> {
    let [operation, high, low, item @ ..] = bytes else {
        return Err(edit_refusal());
    };
    if !matches!(*operation, INSERT | REMOVE)
        || usize::from(u16::from_be_bytes([*high, *low])) != item.len()
    {
        return Err(edit_refusal());
    }
    let item = std::str::from_utf8(item).map_err(|_| edit_refusal())?;
    validate_item(item)?;
    Ok((*operation, item))
}

fn validate_change(edit: &EventEdit) -> Result<(u8, String), WriteIntentError> {
    let (operation, item) = decode_action(edit.change())?;
    Ok((operation, item.to_owned()))
}

fn validate_item(item: &str) -> Result<(), WriteIntentError> {
    if item.is_empty()
        || item.len() > MAX_ITEM_BYTES
        || !item
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        Err(item_refusal())
    } else {
        Ok(())
    }
}

fn item_refusal() -> WriteIntentError {
    WriteIntentError::InvalidEvent(
        "external capability item must be 1..=256 lowercase ASCII bytes".to_owned(),
    )
}

fn edit_refusal() -> WriteIntentError {
    WriteIntentError::InvalidEvent("external capability edit encoding is malformed".to_owned())
}

struct ExternalSetApplier;

impl EditApplier for ExternalSetApplier {
    fn kind(&self) -> Kind {
        KIND
    }

    fn supports(&self, edit: &EventEdit) -> bool {
        edit.kind() == KIND && edit.identifier().is_none() && validate_change(edit).is_ok()
    }

    fn apply(
        &self,
        edit: &EventEdit,
        author: PublicKey,
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        if !self.supports(edit) {
            return Err(edit_refusal());
        }
        if let Some(source) = source
            && (source.author() != author || source.kind() != KIND)
        {
            return Err(WriteIntentError::InvalidEvent(
                "external capability source has the wrong coordinate".to_owned(),
            ));
        }
        let (mut items, preserved) = decode_source(source)?;
        let (operation, item) = validate_change(edit)?;
        match operation {
            INSERT => {
                items.insert(item);
            }
            REMOVE => {
                items.remove(&item);
            }
            _ => unreachable!("validated operation"),
        }
        let state = items.into_iter().collect::<Vec<_>>().join(",");
        let content = format!("{CONTENT_PREFIX}{state}\n{preserved}");
        let (tag_count, tag_shapes) = source.map_or((0, Vec::new()), |source| {
            (
                source.tags().len(),
                source
                    .tags()
                    .iter()
                    .map(|tag| value_shape(tag.as_slice()))
                    .collect::<Vec<_>>(),
            )
        });
        validate_bounds(&content, tag_count, tag_shapes)?;
        let mut builder = EventBuilder::new(author, KIND)
            .created_at(created_at)
            .content(content);
        if let Some(source) = source {
            for tag in source.tags().iter().cloned() {
                builder = builder.tag(tag);
            }
        }
        builder.build().map_err(WriteIntentError::from)
    }
}

fn decode_source(
    source: Option<&EventValue>,
) -> Result<(BTreeSet<String>, String), WriteIntentError> {
    let Some(source) = source else {
        return Ok((BTreeSet::new(), String::new()));
    };
    let content = match source {
        EventValue::Unsigned(event) => event.content.as_str(),
        EventValue::Signed(event) => event.content.as_str(),
    };
    validate_bounds(
        content,
        source.tags().len(),
        source.tags().iter().map(|tag| value_shape(tag.as_slice())),
    )?;
    decode_content(content)
}

fn value_shape(values: &[String]) -> (usize, usize) {
    (
        values.len(),
        values
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .unwrap_or(usize::MAX),
    )
}

fn validate_bounds(
    content: &str,
    tag_count: usize,
    tag_shapes: impl IntoIterator<Item = (usize, usize)>,
) -> Result<(), WriteIntentError> {
    if tag_count > MAX_SOURCE_TAGS {
        return Err(WriteIntentError::InvalidEvent(format!(
            "external capability source tag count exceeds bound: {tag_count} > {MAX_SOURCE_TAGS}"
        )));
    }
    let mut bytes = content.len();
    for (values, tag_bytes) in tag_shapes {
        if values > MAX_SOURCE_TAG_VALUES {
            return Err(WriteIntentError::InvalidEvent(format!(
                "external capability source nested values exceed bound: {values} > {MAX_SOURCE_TAG_VALUES}"
            )));
        }
        bytes = bytes.saturating_add(tag_bytes);
    }
    if bytes > MAX_SOURCE_BYTES {
        Err(WriteIntentError::TooLarge {
            bytes,
            maximum: MAX_SOURCE_BYTES,
        })
    } else {
        Ok(())
    }
}

fn decode_content(content: &str) -> Result<(BTreeSet<String>, String), WriteIntentError> {
    let Some(encoded) = content.strip_prefix(CONTENT_PREFIX) else {
        return Ok((BTreeSet::new(), content.to_owned()));
    };
    let (state, preserved) = encoded.split_once('\n').ok_or_else(edit_refusal)?;
    let mut items = BTreeSet::new();
    if !state.is_empty() {
        for item in state.split(',') {
            validate_item(item)?;
            if !items.insert(item.to_owned()) {
                return Err(WriteIntentError::InvalidEvent(
                    "external capability source contains duplicate state".to_owned(),
                ));
            }
        }
    }
    Ok((items, preserved.to_owned()))
}
