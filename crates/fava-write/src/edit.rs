use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    Kind, MAX_EVENT_BYTES, PublicKey, WriteIntent, WriteIntentError, WritePayload, WriteRouting,
    validate_routing,
};

const MAX_IDENTIFIER_BYTES: usize = 4_096;

/// A bounded protocol-owned change to one exact replaceable event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceableEventEdit {
    kind: Kind,
    identifier: Option<String>,
    change: Vec<u8>,
}

impl ReplaceableEventEdit {
    /// Construct one persistable opaque change to a replaceable coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] when kind and identifier do not form a
    /// replaceable coordinate or a retained value exceeds its bound.
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

    pub(crate) fn validate_for_intent(&self) -> Result<(), WriteIntentError> {
        validate_coordinate(self.kind, self.identifier())?;
        validate_bytes(&self.change)
    }
}

impl WriteIntent {
    /// Validate one semantic edit, resolved author, and route before custody.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for malformed, oversized, or unroutable
    /// edit structure. Provider selection remains outside this neutral owner.
    pub fn edit_as(
        edit: ReplaceableEventEdit,
        author: PublicKey,
        routing: WriteRouting,
    ) -> Result<Self, WriteIntentError> {
        edit.validate_for_intent()?;
        validate_routing(&routing)?;
        Ok(Self {
            payload: WritePayload::Edit { edit, author },
            routing,
        })
    }
}

fn validate_coordinate(kind: Kind, identifier: Option<&str>) -> Result<(), WriteIntentError> {
    if let Some(identifier) = identifier
        && identifier.len() > MAX_IDENTIFIER_BYTES
    {
        return Err(WriteIntentError::TooLarge {
            bytes: identifier.len(),
            maximum: MAX_IDENTIFIER_BYTES,
        });
    }
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

impl Serialize for ReplaceableEventEdit {
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

impl<'de> Deserialize<'de> for ReplaceableEventEdit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedEdit::deserialize(deserializer)?;
        Self::new(encoded.kind, encoded.identifier, encoded.change)
            .map_err(serde::de::Error::custom)
    }
}
