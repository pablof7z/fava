use fava_write::UnsignedEvent;

use crate::{SimpleGroup, SimpleGroupError};

impl SimpleGroup {
    /// Prepare one ordinary author-bearing kind-9002 metadata-management event.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the draft has another kind or invalid simple group context.
    pub fn edit_metadata(&self, payload: UnsignedEvent) -> Result<UnsignedEvent, SimpleGroupError> {
        self.prepare_management(payload, 9_002)
    }

    /// Prepare one ordinary author-bearing kind-9010 pin-management event.
    ///
    /// # Errors
    ///
    /// Returns [`SimpleGroupError`] when the draft has another kind or invalid simple group context.
    pub fn set_pins(&self, payload: UnsignedEvent) -> Result<UnsignedEvent, SimpleGroupError> {
        self.prepare_management(payload, 9_010)
    }

    fn prepare_management(
        &self,
        payload: UnsignedEvent,
        expected_kind: u16,
    ) -> Result<UnsignedEvent, SimpleGroupError> {
        if payload.kind.as_u16() != expected_kind {
            return Err(SimpleGroupError::Event(format!(
                "simple group management event kind is {}, expected {expected_kind}",
                payload.kind.as_u16()
            )));
        }
        self.prepare(payload)
    }
}
