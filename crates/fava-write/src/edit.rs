use fava_state::EventCoordinate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    EventId, Kind, MAX_EVENT_BYTES, PublicKey, WriteIntent, WriteIntentError, WritePayload,
    WriteRouting, validate_routing,
};

/// A bounded protocol-owned change to one exact replaceable event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplaceableEventEdit {
    actor: PublicKey,
    coordinate: EventCoordinate,
    format: u32,
    change: Vec<u8>,
    inverse: Vec<u8>,
}

impl ReplaceableEventEdit {
    /// Construct one persistable opaque edit and its inverse.
    ///
    /// Coordinate and actor consistency is checked when a [`crate::WriteIntent`]
    /// is created, so protocol code can construct a value whose refusal is
    /// observable at the pre-custody boundary.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError::TooLarge`] when either opaque value exceeds
    /// the ordinary write-intent byte bound.
    pub fn new(
        actor: PublicKey,
        coordinate: EventCoordinate,
        format: u32,
        change: Vec<u8>,
        inverse: Vec<u8>,
    ) -> Result<Self, WriteIntentError> {
        validate_bytes(&change)?;
        validate_bytes(&inverse)?;
        change
            .len()
            .checked_add(inverse.len())
            .ok_or_else(|| WriteIntentError::Encoding("edit byte count overflow".to_owned()))?;
        Ok(Self {
            actor,
            coordinate,
            format,
            change,
            inverse,
        })
    }

    /// Actor fixed before any event body exists.
    #[must_use]
    pub const fn actor(&self) -> PublicKey {
        self.actor
    }

    /// Exact replaceable-event coordinate affected by this edit.
    #[must_use]
    pub const fn coordinate(&self) -> &EventCoordinate {
        &self.coordinate
    }

    /// Protocol-owned durable format version.
    #[must_use]
    pub const fn format(&self) -> u32 {
        self.format
    }

    /// Opaque protocol-owned change bytes.
    #[must_use]
    pub fn change(&self) -> &[u8] {
        &self.change
    }

    /// Opaque protocol-owned inverse bytes.
    #[must_use]
    pub fn inverse_change(&self) -> &[u8] {
        &self.inverse
    }

    /// Return the bounded inverse through the same neutral value.
    #[must_use]
    pub fn inverse(&self) -> Self {
        Self {
            actor: self.actor,
            coordinate: self.coordinate.clone(),
            format: self.format,
            change: self.inverse.clone(),
            inverse: self.change.clone(),
        }
    }

    pub(crate) fn validate_for_intent(&self) -> Result<(), WriteIntentError> {
        validate_bytes(&self.change)?;
        validate_bytes(&self.inverse)?;
        self.change
            .len()
            .checked_add(self.inverse.len())
            .ok_or_else(|| WriteIntentError::Encoding("edit byte count overflow".to_owned()))?;
        match &self.coordinate {
            EventCoordinate::Replaceable {
                author,
                kind,
                identifier: None,
            } if *author == self.actor && kind.is_replaceable() => Ok(()),
            EventCoordinate::Replaceable {
                identifier: Some(_),
                ..
            } => Err(WriteIntentError::InvalidEvent(
                "addressable replaceable-event edits are not supported".to_owned(),
            )),
            EventCoordinate::Replaceable { .. } => Err(WriteIntentError::InvalidEvent(
                "replaceable-event edit actor and coordinate must match".to_owned(),
            )),
            EventCoordinate::Event(_) => Err(WriteIntentError::InvalidEvent(
                "replaceable-event edit requires a replaceable coordinate".to_owned(),
            )),
        }
    }
}

impl WriteIntent {
    /// Validate one semantic replaceable-event edit and its route before custody.
    ///
    /// # Errors
    ///
    /// Returns [`WriteIntentError`] for malformed, oversized, addressable, or
    /// unroutable edit structure. Provider selection is intentionally outside
    /// this neutral value owner.
    pub fn edit(
        edit: ReplaceableEventEdit,
        routing: WriteRouting,
    ) -> Result<Self, WriteIntentError> {
        edit.validate_for_intent()?;
        validate_routing(&routing)?;
        Ok(Self {
            payload: WritePayload::Edit(edit),
            routing,
        })
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
    actor: PublicKey,
    coordinate: EncodedCoordinate,
    format: u32,
    change: Vec<u8>,
    inverse: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
enum EncodedCoordinate {
    Event(EventId),
    Replaceable {
        author: PublicKey,
        kind: Kind,
        identifier: Option<String>,
    },
}

impl From<&EventCoordinate> for EncodedCoordinate {
    fn from(value: &EventCoordinate) -> Self {
        match value {
            EventCoordinate::Event(id) => Self::Event(*id),
            EventCoordinate::Replaceable {
                author,
                kind,
                identifier,
            } => Self::Replaceable {
                author: *author,
                kind: *kind,
                identifier: identifier.clone(),
            },
        }
    }
}

impl From<EncodedCoordinate> for EventCoordinate {
    fn from(value: EncodedCoordinate) -> Self {
        match value {
            EncodedCoordinate::Event(id) => Self::Event(id),
            EncodedCoordinate::Replaceable {
                author,
                kind,
                identifier,
            } => Self::Replaceable {
                author,
                kind,
                identifier,
            },
        }
    }
}

impl Serialize for ReplaceableEventEdit {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        EncodedEdit {
            actor: self.actor,
            coordinate: EncodedCoordinate::from(&self.coordinate),
            format: self.format,
            change: self.change.clone(),
            inverse: self.inverse.clone(),
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
        Self::new(
            encoded.actor,
            encoded.coordinate.into(),
            encoded.format,
            encoded.change,
            encoded.inverse,
        )
        .map_err(serde::de::Error::custom)
    }
}
