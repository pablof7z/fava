use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    Kind, MAX_EVENT_BYTES, PublicKey, WriteIntent, WriteIntentError, WritePayload, WriteRouting,
};

/// A protocol-owned change to one exact replaceable event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventEdit {
    kind: Kind,
    identifier: Option<String>,
    change: Vec<u8>,
}

impl EventEdit {
    /// Construct one persistable opaque change to a replaceable coordinate.
    ///
    /// # Arguments
    ///
    /// * `kind` - the replaceable or addressable kind the change targets
    /// * `identifier` - the addressable `d` tag value; `None` for a plain
    ///   replaceable kind, `Some` (including empty) for an addressable kind
    /// * `change` - the opaque protocol-owned change bytes
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] when kind and identifier do not form a
    /// replaceable coordinate or a retained value exceeds its bound.
    ///
    /// # Examples
    ///
    /// ```
    /// # use fava_write::{Kind, EventEdit};
    /// let edit = EventEdit::new(
    ///     Kind::from_u16(30_023),
    ///     Some("article".to_owned()),
    ///     b"opaque change".to_vec(),
    /// )
    /// .expect("addressable kind with identifier is a valid coordinate");
    /// ```
    pub fn new(
        kind: Kind,
        identifier: Option<String>,
        change: Vec<u8>,
    ) -> Result<Self, WriteIntentError> {
        validate_coordinate(kind, identifier.as_deref())?;
        validate_bytes(&change)?;
        Ok(Self {
            kind,
            identifier,
            change,
        })
    }

    /// Replaceable kind affected by this edit.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// Addressable identifier affected by this edit, when present.
    #[must_use]
    pub fn identifier(&self) -> Option<&str> {
        self.identifier.as_deref()
    }

    /// Opaque protocol-owned change bytes.
    #[must_use]
    pub fn change(&self) -> &[u8] {
        &self.change
    }
}

impl WriteIntent {
    /// Validate one semantic edit, resolved author, and route before custody.
    ///
    /// # Arguments
    ///
    /// * `edit` - the already-validated protocol-owned change
    /// * `author` - the author resolved for this edit, exactly once
    /// * `routing` - the relay-routing mode to publish it under
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for malformed, oversized, or unroutable
    /// edit structure. Provider selection remains outside this neutral owner.
    pub fn edit_as(
        edit: EventEdit,
        author: PublicKey,
        routing: WriteRouting,
    ) -> Result<Self, WriteIntentError> {
        routing.validate()?;
        Ok(Self {
            payload: WritePayload::Edit { edit, author },
            routing,
            access: fava_relay::RelayAccess::Public,
        })
    }
}

fn validate_coordinate(kind: Kind, identifier: Option<&str>) -> Result<(), WriteIntentError> {
    match identifier {
        None if kind.is_replaceable() => Ok(()),
        Some(_) if kind.is_addressable() => Ok(()),
        None => Err(WriteIntentError::InvalidEvent(
            "replaceable-event edit kind requires an addressable identifier".to_owned(),
        )),
        Some(_) => Err(WriteIntentError::InvalidEvent(
            "replaceable-event edit identifier requires an addressable kind".to_owned(),
        )),
    }
}

fn validate_bytes(value: &[u8]) -> Result<(), WriteIntentError> {
    if value.len() > MAX_EVENT_BYTES {
        Err(WriteIntentError::TooLarge {
            bytes: value.len(),
            maximum: MAX_EVENT_BYTES,
        })
    } else {
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EncodedEdit {
    kind: Kind,
    #[serde(deserialize_with = "deserialize_identifier")]
    identifier: Option<String>,
    change: Vec<u8>,
}

fn deserialize_identifier<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

impl Serialize for EventEdit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        EncodedEdit {
            kind: self.kind,
            identifier: self.identifier.clone(),
            change: self.change.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EventEdit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedEdit::deserialize(deserializer)?;
        Self::new(encoded.kind, encoded.identifier, encoded.change)
            .map_err(serde::de::Error::custom)
    }
}
