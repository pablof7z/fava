use fava_write::UnsignedEvent;

use crate::{Group, GroupError};

impl Group {
    /// Prepare one ordinary author-bearing kind-9002 metadata-management event.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the draft has another kind or invalid group context.
    pub fn edit_metadata(&self, payload: UnsignedEvent) -> Result<UnsignedEvent, GroupError> {
        self.prepare_management(payload, 9_002)
    }

    /// Prepare one ordinary author-bearing kind-9010 pin-management event.
    ///
    /// # Errors
    ///
    /// Returns [`GroupError`] when the draft has another kind or invalid group context.
    pub fn set_pins(&self, payload: UnsignedEvent) -> Result<UnsignedEvent, GroupError> {
        self.prepare_management(payload, 9_010)
    }

    fn prepare_management(
        &self,
        payload: UnsignedEvent,
        expected_kind: u16,
    ) -> Result<UnsignedEvent, GroupError> {
        if payload.kind.as_u16() != expected_kind {
            return Err(GroupError::Event(format!(
                "group management event kind is {}, expected {expected_kind}",
                payload.kind.as_u16()
            )));
        }
        self.prepare(payload)
    }
}
