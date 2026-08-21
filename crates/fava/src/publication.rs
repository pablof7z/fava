//! Application-facing synchronous publication vocabulary.

use std::fmt;

use fava_publication::{Publication, PublicationError};
use fava_write::{
    Event, Receipt, ReceiptId, ReplaceableEventEdit, UnsignedEvent, WriteId, WriteIntent,
    WriteIntentError, WriteRouting,
};
use thiserror::Error;

/// One accepted publication obligation as seen by an application.
#[derive(Clone)]
pub struct Write {
    write_id: WriteId,
    receipt_id: ReceiptId,
    publication: Publication,
}

impl Write {
    /// Stable identity of this accepted publication obligation.
    #[must_use]
    pub const fn write_id(&self) -> WriteId {
        self.write_id
    }

    /// Stable reattachable identity of this publication receipt.
    #[must_use]
    pub const fn receipt_id(&self) -> ReceiptId {
        self.receipt_id
    }

    /// Read the current complete receipt from its durable owner.
    ///
    /// # Errors
    ///
    /// Returns [`PublishError`] when storage fails or the receipt disappeared.
    pub fn receipt(&self) -> Result<Receipt, PublishError> {
        self.publication
            .receipt(self.receipt_id)?
            .ok_or_else(|| PublicationError::ReceiptMissing(self.receipt_id).into())
    }
}

impl fmt::Debug for Write {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Write")
            .field("write_id", &self.write_id)
            .field("receipt_id", &self.receipt_id)
            .finish_non_exhaustive()
    }
}

/// Refusal at the application publication door.
#[derive(Debug, Error)]
pub enum PublishError {
    /// An authorless edit has no selected current account or explicit signer scope.
    #[error("replaceable edit publication requires an author selection")]
    MissingAuthor,
    /// Payload validation refused before durable custody.
    #[error(transparent)]
    Intent(#[from] WriteIntentError),
    /// The neutral publication owner refused or failed.
    #[error(transparent)]
    Publication(#[from] PublicationError),
}

pub(crate) trait PublishPayload {
    fn into_intent(
        self,
        author: Option<fava_write::PublicKey>,
    ) -> Result<WriteIntent, PublishError>;
}

impl PublishPayload for UnsignedEvent {
    fn into_intent(
        self,
        _author: Option<fava_write::PublicKey>,
    ) -> Result<WriteIntent, PublishError> {
        Ok(WriteIntent::event(self, WriteRouting::Automatic)?)
    }
}

impl PublishPayload for Event {
    fn into_intent(
        self,
        _author: Option<fava_write::PublicKey>,
    ) -> Result<WriteIntent, PublishError> {
        Ok(WriteIntent::presigned(self, WriteRouting::Automatic)?)
    }
}

impl PublishPayload for ReplaceableEventEdit {
    fn into_intent(
        self,
        author: Option<fava_write::PublicKey>,
    ) -> Result<WriteIntent, PublishError> {
        let author = author.ok_or(PublishError::MissingAuthor)?;
        Ok(WriteIntent::edit_as(self, author, WriteRouting::Automatic)?)
    }
}

impl PublishPayload for WriteIntent {
    fn into_intent(
        self,
        _author: Option<fava_write::PublicKey>,
    ) -> Result<WriteIntent, PublishError> {
        Ok(self)
    }
}

pub(crate) fn publish<P>(
    publication: Option<&Publication>,
    payload: P,
) -> Result<Write, PublishError>
where
    P: PublishPayload,
{
    let intent = payload.into_intent(None)?;
    let publication = publication.ok_or(PublicationError::NotConfigured)?;
    let accepted = publication.accept(intent)?;
    Ok(Write {
        write_id: accepted.write_id,
        receipt_id: accepted.receipt_id,
        publication: publication.clone(),
    })
}
