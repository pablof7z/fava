use std::sync::Arc;

use fava_state::RelayUrl;
use fava_write::{
    Event, EventBuildError, EventBuilder, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Tag, Timestamp, UnsignedEvent, WriteIntentError,
};

use crate::records::MAX_RECORD_VALUE_BYTES;
use crate::{Group, GroupError, SimpleGroups};

const SAVED_KIND: u16 = 10_009;
const SAVE_GROUP: u8 = 1;
const REMOVE_GROUP: u8 = 2;
const RENAME_GROUP: u8 = 3;
const SAVE_RELAY: u8 = 4;
const REMOVE_RELAY: u8 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Change {
    SaveGroup {
        id: String,
        hosts: Vec<RelayUrl>,
        name: Option<String>,
    },
    RemoveGroup {
        id: String,
        hosts: Vec<RelayUrl>,
    },
    RenameGroup {
        id: String,
        hosts: Vec<RelayUrl>,
        name: String,
    },
    SaveRelay(RelayUrl),
    RemoveRelay(RelayUrl),
}

impl SimpleGroups {
    /// Produce one bounded edit that saves every selected group host.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the change cannot fit the bounded private codec.
    pub fn save_group(
        group: &Group,
        name: Option<&str>,
    ) -> Result<ReplaceableEventEdit, GroupError> {
        edit(&Change::SaveGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
            name: name.map(str::to_owned),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that removes every selected group host row.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the change cannot fit the bounded private codec.
    pub fn remove_group(group: &Group) -> Result<ReplaceableEventEdit, GroupError> {
        edit(&Change::RemoveGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that renames every selected group host row.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the name or change exceeds its declared bound.
    pub fn rename_saved_group(
        group: &Group,
        name: &str,
    ) -> Result<ReplaceableEventEdit, GroupError> {
        edit(&Change::RenameGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
            name: name.to_owned(),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that saves an inert relay-in-use row.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the relay cannot fit the bounded private codec.
    pub fn save_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, GroupError> {
        edit(&Change::SaveRelay(relay)).map_err(Into::into)
    }

    /// Produce one bounded edit that removes an exact relay-in-use row.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the relay cannot fit the bounded private codec.
    pub fn remove_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, GroupError> {
        edit(&Change::RemoveRelay(relay)).map_err(Into::into)
    }

    /// Select the pure kind-10009 saved-list materializer.
    #[must_use]
    pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
        Arc::new(SavedListMaterializer)
    }
}

fn edit(change: &Change) -> Result<ReplaceableEventEdit, WriteIntentError> {
    ReplaceableEventEdit::new(saved_kind(), None, encode(change)?)
}

fn encode(change: &Change) -> Result<Vec<u8>, WriteIntentError> {
    let mut bytes = Vec::new();
    match change {
        Change::SaveGroup { id, hosts, name } => {
            bytes.push(SAVE_GROUP);
            encode_group(&mut bytes, id, hosts)?;
            encode_optional(&mut bytes, name.as_deref())?;
        }
        Change::RemoveGroup { id, hosts } => {
            bytes.push(REMOVE_GROUP);
            encode_group(&mut bytes, id, hosts)?;
        }
        Change::RenameGroup { id, hosts, name } => {
            bytes.push(RENAME_GROUP);
            encode_group(&mut bytes, id, hosts)?;
            encode_text(&mut bytes, name)?;
        }
        Change::SaveRelay(relay) => {
            bytes.push(SAVE_RELAY);
            encode_text(&mut bytes, relay.as_str())?;
        }
        Change::RemoveRelay(relay) => {
            bytes.push(REMOVE_RELAY);
            encode_text(&mut bytes, relay.as_str())?;
        }
    }
    Ok(bytes)
}

fn encode_group(bytes: &mut Vec<u8>, id: &str, hosts: &[RelayUrl]) -> Result<(), WriteIntentError> {
    encode_text(bytes, id)?;
    let count = u16::try_from(hosts.len()).map_err(|_| codec_refusal("too many group hosts"))?;
    bytes.extend_from_slice(&count.to_be_bytes());
    for host in hosts {
        encode_text(bytes, host.as_str())?;
    }
    Ok(())
}

fn encode_optional(bytes: &mut Vec<u8>, value: Option<&str>) -> Result<(), WriteIntentError> {
    if let Some(value) = value {
        bytes.push(1);
        encode_text(bytes, value)
    } else {
        bytes.push(0);
        Ok(())
    }
}

fn encode_text(bytes: &mut Vec<u8>, value: &str) -> Result<(), WriteIntentError> {
    if value.len() > MAX_RECORD_VALUE_BYTES {
        return Err(WriteIntentError::TooLarge {
            bytes: value.len(),
            maximum: MAX_RECORD_VALUE_BYTES,
        });
    }
    let length = u32::try_from(value.len()).map_err(|_| codec_refusal("edit text is too long"))?;
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
    Ok(())
}

fn decode_edit(edit: &ReplaceableEventEdit) -> Result<Change, WriteIntentError> {
    if edit.kind() != saved_kind() || edit.identifier().is_some() {
        return Err(codec_refusal(
            "saved-list edit requires a non-addressable kind-10009 coordinate",
        ));
    }
    let mut input = Input::new(edit.change());
    let operation = input.byte()?;
    let change = match operation {
        SAVE_GROUP => {
            let (id, hosts) = input.group()?;
            let name = input.optional_text()?;
            Change::SaveGroup { id, hosts, name }
        }
        REMOVE_GROUP => {
            let (id, hosts) = input.group()?;
            Change::RemoveGroup { id, hosts }
        }
        RENAME_GROUP => {
            let (id, hosts) = input.group()?;
            let name = input.text()?;
            Change::RenameGroup { id, hosts, name }
        }
        SAVE_RELAY => Change::SaveRelay(input.relay()?),
        REMOVE_RELAY => Change::RemoveRelay(input.relay()?),
        _ => return Err(codec_refusal("unknown saved-list edit operation")),
    };
    if input.bytes.is_empty() {
        Ok(change)
    } else {
        Err(codec_refusal("trailing saved-list edit bytes"))
    }
}

struct Input<'a> {
    bytes: &'a [u8],
}

impl<'a> Input<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn byte(&mut self) -> Result<u8, WriteIntentError> {
        let Some((&value, remaining)) = self.bytes.split_first() else {
            return Err(codec_refusal("truncated saved-list edit"));
        };
        self.bytes = remaining;
        Ok(value)
    }

    fn text(&mut self) -> Result<String, WriteIntentError> {
        let length_bytes: [u8; 4] = self
            .bytes
            .get(..4)
            .ok_or_else(|| codec_refusal("truncated saved-list text length"))?
            .try_into()
            .map_err(|_| codec_refusal("invalid saved-list text length"))?;
        let length = usize::try_from(u32::from_be_bytes(length_bytes))
            .map_err(|_| codec_refusal("saved-list text length overflow"))?;
        if length > MAX_RECORD_VALUE_BYTES {
            return Err(WriteIntentError::TooLarge {
                bytes: length,
                maximum: MAX_RECORD_VALUE_BYTES,
            });
        }
        let end = 4usize
            .checked_add(length)
            .ok_or_else(|| codec_refusal("saved-list text length overflow"))?;
        let value = self
            .bytes
            .get(4..end)
            .ok_or_else(|| codec_refusal("truncated saved-list text"))?;
        self.bytes = &self.bytes[end..];
        std::str::from_utf8(value)
            .map(str::to_owned)
            .map_err(|_| codec_refusal("saved-list text is not UTF-8"))
    }

    fn optional_text(&mut self) -> Result<Option<String>, WriteIntentError> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.text().map(Some),
            _ => Err(codec_refusal("invalid saved-list optional text flag")),
        }
    }

    fn relay(&mut self) -> Result<RelayUrl, WriteIntentError> {
        RelayUrl::parse(&self.text()?).map_err(|_| codec_refusal("invalid saved-list relay"))
    }

    fn group(&mut self) -> Result<(String, Vec<RelayUrl>), WriteIntentError> {
        let id = self.text()?;
        if id.is_empty() {
            return Err(codec_refusal("saved-list group id is empty"));
        }
        let count_bytes: [u8; 2] = self
            .bytes
            .get(..2)
            .ok_or_else(|| codec_refusal("truncated saved-list host count"))?
            .try_into()
            .map_err(|_| codec_refusal("invalid saved-list host count"))?;
        self.bytes = &self.bytes[2..];
        let count = usize::from(u16::from_be_bytes(count_bytes));
        if count == 0 || count > crate::bounds::MAX_GROUP_HOST_INPUT_ITEMS {
            return Err(codec_refusal("invalid saved-list host count"));
        }
        let mut hosts = Vec::with_capacity(count);
        for _ in 0..count {
            hosts.push(self.relay()?);
        }
        Ok((id, hosts))
    }
}

struct SavedListMaterializer;

impl ReplaceableEventMaterializer for SavedListMaterializer {
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
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        let change = decode_edit(edit)?;
        let (content, source_tags) = qualified_source(author, source, created_at)?;
        let tags = apply(source_tags, &change)?;
        let event =
            EventBuilder::from_parts(author, saved_kind(), created_at, tags, content.to_owned())
                .build()
                .map_err(map_build_error)?;
        event
            .verify_id()
            .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
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
    crate::records::validate_structure(source).map_err(|error| group_refusal(&error))?;
    source
        .verify()
        .map_err(|error| WriteIntentError::InvalidEvent(error.to_string()))?;
    if source.pubkey != author || source.kind != saved_kind() {
        return Err(WriteIntentError::InvalidEvent(
            "saved-list source author or kind does not match accepted write".to_owned(),
        ));
    }
    if created_at <= source.created_at {
        return Err(WriteIntentError::InvalidEvent(
            "saved-list materialization timestamp must succeed its source".to_owned(),
        ));
    }
    Ok((&source.content, source.tags.as_slice()))
}

fn apply(source: &[Tag], change: &Change) -> Result<Vec<Tag>, WriteIntentError> {
    match change {
        Change::SaveGroup { id, hosts, name } => {
            apply_group(source, id, hosts, GroupOperation::Save(name.as_deref()))
        }
        Change::RemoveGroup { id, hosts } => apply_group(source, id, hosts, GroupOperation::Remove),
        Change::RenameGroup { id, hosts, name } => {
            apply_group(source, id, hosts, GroupOperation::Rename(name))
        }
        Change::SaveRelay(relay) => apply_relay(source, relay, true),
        Change::RemoveRelay(relay) => apply_relay(source, relay, false),
    }
}

#[derive(Clone, Copy)]
enum GroupOperation<'a> {
    Save(Option<&'a str>),
    Remove,
    Rename(&'a str),
}

fn apply_group(
    source: &[Tag],
    id: &str,
    hosts: &[RelayUrl],
    operation: GroupOperation<'_>,
) -> Result<Vec<Tag>, WriteIntentError> {
    let selected = hosts
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut found = std::collections::BTreeSet::new();
    let mut tags = Vec::with_capacity(source.len().saturating_add(hosts.len()));
    for tag in source {
        let Some(host) = group_target(tag, id).filter(|host| selected.contains(host)) else {
            tags.push(tag.clone());
            continue;
        };
        if !found.insert(host) {
            continue;
        }
        match operation {
            GroupOperation::Save(_) => tags.push(tag.clone()),
            GroupOperation::Remove => {}
            GroupOperation::Rename(name) => tags.push(renamed_row(tag, name)?),
        }
    }
    match operation {
        GroupOperation::Save(name) => {
            for host in hosts.iter().filter(|host| !found.contains(*host)) {
                tags.push(group_row(id, host, name)?);
            }
        }
        GroupOperation::Rename(name) => {
            for host in hosts.iter().filter(|host| !found.contains(*host)) {
                tags.push(group_row(id, host, Some(name))?);
            }
        }
        GroupOperation::Remove => {}
    }
    Ok(tags)
}

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

fn group_target(tag: &Tag, id: &str) -> Option<RelayUrl> {
    let values = tag.as_slice();
    ((values.len() == 3 || values.len() == 4)
        && values.first().map(String::as_str) == Some("group")
        && values.get(1).map(String::as_str) == Some(id))
    .then(|| values.get(2).and_then(|host| RelayUrl::parse(host).ok()))
    .flatten()
}

fn relay_target(tag: &Tag) -> Option<RelayUrl> {
    let values = tag.as_slice();
    (values.len() == 2 && values.first().map(String::as_str) == Some("r"))
        .then(|| values.get(1).and_then(|relay| RelayUrl::parse(relay).ok()))
        .flatten()
}

fn renamed_row(tag: &Tag, name: &str) -> Result<Tag, WriteIntentError> {
    let mut values = tag.as_slice()[..3].to_vec();
    values.push(name.to_owned());
    Tag::parse(values).map_err(|error| codec_refusal(&error.to_string()))
}

fn group_row(id: &str, host: &RelayUrl, name: Option<&str>) -> Result<Tag, WriteIntentError> {
    let mut values = vec!["group".to_owned(), id.to_owned(), host.as_str().to_owned()];
    if let Some(name) = name {
        values.push(name.to_owned());
    }
    Tag::parse(values).map_err(|error| codec_refusal(&error.to_string()))
}

fn saved_kind() -> Kind {
    Kind::from_u16(SAVED_KIND)
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

fn group_refusal(error: &GroupError) -> WriteIntentError {
    WriteIntentError::InvalidEvent(error.to_string())
}
