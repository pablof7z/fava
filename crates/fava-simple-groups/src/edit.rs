use std::sync::Arc;

use fava_state::RelayUrl;
use fava_write::{
    Event, EventBuildError, EventBuilder, Kind, PublicKey, ReplaceableEventEdit,
    ReplaceableEventMaterializer, Timestamp, UnsignedEvent, WriteIntentError,
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
    pub fn save_group(
        group: &Group,
        name: Option<&str>,
    ) -> Result<ReplaceableEventEdit, GroupError> {
        edit(Change::SaveGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
            name: name.map(str::to_owned),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that removes every selected group host row.
    pub fn remove_group(group: &Group) -> Result<ReplaceableEventEdit, GroupError> {
        edit(Change::RemoveGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that renames every selected group host row.
    pub fn rename_saved_group(
        group: &Group,
        name: &str,
    ) -> Result<ReplaceableEventEdit, GroupError> {
        edit(Change::RenameGroup {
            id: group.id().to_owned(),
            hosts: group.hosts().collect(),
            name: name.to_owned(),
        })
        .map_err(Into::into)
    }

    /// Produce one bounded edit that saves an inert relay-in-use row.
    pub fn save_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, GroupError> {
        edit(Change::SaveRelay(relay)).map_err(Into::into)
    }

    /// Produce one bounded edit that removes an exact relay-in-use row.
    pub fn remove_relay(relay: RelayUrl) -> Result<ReplaceableEventEdit, GroupError> {
        edit(Change::RemoveRelay(relay)).map_err(Into::into)
    }

    /// Select the pure kind-10009 saved-list materializer.
    #[must_use]
    pub fn materializer() -> Arc<dyn ReplaceableEventMaterializer> {
        Arc::new(SavedListMaterializer)
    }
}

fn edit(change: Change) -> Result<ReplaceableEventEdit, WriteIntentError> {
    ReplaceableEventEdit::new(saved_kind(), None, encode(&change)?)
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
    match value {
        Some(value) => {
            bytes.push(1);
            encode_text(bytes, value)
        }
        None => {
            bytes.push(0);
            Ok(())
        }
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
        _source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError> {
        let _ = decode_edit(edit)?;
        EventBuilder::new(author, saved_kind())
            .created_at(created_at)
            .build()
            .map_err(map_build_error)
    }
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
