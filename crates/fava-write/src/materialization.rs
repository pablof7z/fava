use crate::{Event, Kind, ReplaceableEventEdit, Timestamp, UnsignedEvent, WriteIntentError};
use serde::{Deserialize, Serialize};

/// Exact identity of one immutable event materialization generation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MaterializationId(u64);

impl MaterializationId {
    /// Construct an id allocated by a write store.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Pure protocol-provider contract for one replaceable-event edit format.
pub trait ReplaceableEventMaterializer: Send + Sync {
    /// Exact non-addressable replaceable kind owned by this provider.
    fn kind(&self) -> Kind;

    /// Whether this provider owns the edit's coordinate and durable format.
    fn supports(&self, edit: &ReplaceableEventEdit) -> bool;

    /// Apply the edit to qualified source or protocol-defined empty state.
    ///
    /// The caller supplies the exact timestamp. Implementations return only an
    /// unsigned event and receive no custody, signer, routing, publication,
    /// delivery, cache, or receipt authority.
    ///
    /// # Errors
    ///
    /// Returns an existing typed write refusal when the opaque format or
    /// resulting event cannot be materialized.
    fn materialize(
        &self,
        edit: &ReplaceableEventEdit,
        source: Option<&Event>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError>;
}
