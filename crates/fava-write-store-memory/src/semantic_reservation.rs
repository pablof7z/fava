//! Coordinate-bound bounded reservations for volatile semantic admission.

use fava_write::{PublicKey, ReplaceableEventEdit};
use fava_write_store::WriteStoreError;

use super::MemoryWriteStore;
use super::state::{capacity_reached, edit_coordinate};

impl MemoryWriteStore {
    pub(super) fn reserve_active_slot(
        &self,
        edit: &ReplaceableEventEdit,
        author: PublicKey,
    ) -> Result<u64, WriteStoreError> {
        let mut state = self.lock_state()?;
        let coordinate = edit_coordinate(edit, author);
        if state
            .reservations
            .values()
            .any(|reserved| reserved == &coordinate)
        {
            return Err(WriteStoreError::Refused(
                "replaceable coordinate already has an active reservation".to_owned(),
            ));
        }
        if !state.coordinates.contains_key(&coordinate)
            && capacity_reached(&state, self.capacity.get())
        {
            return Err(WriteStoreError::Refused(format!(
                "bounded write-store capacity {} reached",
                self.capacity
            )));
        }
        let reservation = state.next_reservation;
        state.next_reservation = reservation
            .checked_add(1)
            .ok_or_else(|| WriteStoreError::Refused("active reservation exhausted".to_owned()))?;
        state.reservations.insert(reservation, coordinate);
        Ok(reservation)
    }

    pub(super) fn release_active_slot(&self, reservation: u64) -> Result<(), WriteStoreError> {
        let mut state = self.lock_state()?;
        if state.reservations.remove(&reservation).is_some() {
            Ok(())
        } else {
            Err(WriteStoreError::Refused(
                "active reservation is not current".to_owned(),
            ))
        }
    }
}
