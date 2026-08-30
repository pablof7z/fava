use crate::{
    EventValue, Kind, PublicKey, EventEdit, Timestamp, UnsignedEvent, WriteIntentError,
};
use serde::{Deserialize, Serialize};
use std::num::{NonZeroU64, TryFromIntError};
use std::sync::Arc;

/// Exact identity of one immutable event revision generation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RevisionId(NonZeroU64);

impl RevisionId {
    /// First revision generation of an accepted write.
    pub const FIRST: Self = Self(NonZeroU64::MIN);

    /// Reconstruct a nonzero generation value.
    ///
    /// Construction does not make the generation current. Only the owning
    /// write store can commit a generation transition.
    #[must_use]
    pub const fn from_nonzero(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Return the provider-independent numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }

    /// Return the next generation, or `None` at numeric exhaustion.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.get().checked_add(1) {
            Some(value) => match NonZeroU64::new(value) {
                Some(value) => Some(Self(value)),
                None => None,
            },
            None => None,
        }
    }
}

impl TryFrom<u64> for RevisionId {
    type Error = TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::try_from(value).map(Self)
    }
}

/// Pure protocol-provider contract for one replaceable-event change encoding.
pub trait EditApplier: Send + Sync {
    /// Exact replaceable or addressable event kind owned by this provider.
    fn kind(&self) -> Kind;

    /// Whether this provider owns the edit's coordinate and change encoding.
    fn supports(&self, edit: &EventEdit) -> bool;

    /// Apply the edit to qualified signed or unsigned source, or protocol-defined empty state.
    ///
    /// The caller supplies the accepted write's exact author and timestamp.
    /// Implementations return only an unsigned event and receive no custody,
    /// signer, routing, publication, delivery, cache, or receipt authority.
    ///
    /// # Arguments
    ///
    /// * `edit` - the opaque protocol-owned change to apply
    /// * `author` - the accepted write's exact author
    /// * `source` - the current qualified event for this coordinate, or
    ///   `None` when the coordinate is protocol-defined empty state
    /// * `created_at` - the accepted write's exact timestamp
    ///
    /// # Errors
    ///
    /// Returns an existing typed write refusal when the opaque change or
    /// resulting event cannot be applied.
    fn apply(
        &self,
        edit: &EventEdit,
        author: PublicKey,
        source: Option<&EventValue>,
        created_at: Timestamp,
    ) -> Result<UnsignedEvent, WriteIntentError>;
}

/// Neutral acceptor for an edit applier, owned by the crate that owns the
/// edit-applier contract so that a protocol crate's enabling call does not
/// require it to depend on the facade that assembles handlers.
///
/// # Examples
///
/// ```
/// # use std::sync::Arc;
/// # use fava_write::{EditApplier, EditApplierSink};
/// #[derive(Default)]
/// struct Sink(Vec<Arc<dyn EditApplier>>);
///
/// impl EditApplierSink for Sink {
///     fn accept(mut self, applier: Arc<dyn EditApplier>) -> Self {
///         self.0.push(applier);
///         self
///     }
/// }
/// ```
pub trait EditApplierSink {
    /// Register the applier and return the sink for further configuration.
    ///
    /// # Arguments
    ///
    /// * `applier` - the edit applier to register
    #[must_use]
    fn accept(self, applier: Arc<dyn EditApplier>) -> Self;
}
